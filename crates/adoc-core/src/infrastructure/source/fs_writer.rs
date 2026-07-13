//! Filesystem implementation of the patch-apply write port (V6.4, ADR-0036).
//!
//! Atomicity per file: temp file in the same directory (`create_new`), write,
//! fsync, re-hash the target immediately before rename (TOCTOU guard, which
//! also refuses a target that became a symlink since containment resolved
//! it), then rename over the target and best-effort fsync the parent
//! directory so the rename itself is durable. The target is never touched on
//! any error path and never reverted after the rename. Containment mirrors `adoc-local`'s
//! `ProjectRootPathPolicy` (which lives downstream and cannot be reused here):
//! `..` components are rejected and the resolved path must stay under the
//! sandbox root.

use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::domain::hashing::sha256_prefixed;
use crate::domain::ports::workspace_writer::{WorkspaceWriteError, WorkspaceWriter};

pub(crate) struct FsWorkspaceWriter {
    sandbox_root: PathBuf,
}

impl FsWorkspaceWriter {
    pub(crate) fn new(sandbox_root: impl Into<PathBuf>) -> Self {
        let root = sandbox_root.into();
        let sandbox_root = std::fs::canonicalize(&root).unwrap_or_else(|_| normalize_path(&root));
        Self { sandbox_root }
    }

    fn contained(&self, path: &Path) -> Result<PathBuf, WorkspaceWriteError> {
        if path
            .components()
            .any(|component| component == Component::ParentDir)
        {
            return Err(WorkspaceWriteError::OutsideSandbox {
                path: path.to_path_buf(),
            });
        }
        let candidate = if path.is_absolute() {
            normalize_path(path)
        } else {
            normalize_path(&self.sandbox_root.join(path))
        };
        let resolved = resolve_through_nearest_existing_ancestor(&candidate);
        if resolved.starts_with(&self.sandbox_root) {
            Ok(resolved)
        } else {
            Err(WorkspaceWriteError::OutsideSandbox { path: candidate })
        }
    }
}

impl WorkspaceWriter for FsWorkspaceWriter {
    fn read_to_string(&self, path: &Path) -> Result<String, WorkspaceWriteError> {
        let resolved = self.contained(path)?;
        std::fs::read_to_string(&resolved).map_err(|error| WorkspaceWriteError::Io {
            path: resolved,
            message: error.to_string(),
        })
    }

    fn write_atomic(
        &self,
        path: &Path,
        contents: &str,
        expected_current_hash: &str,
    ) -> Result<(), WorkspaceWriteError> {
        let resolved = self.contained(path)?;
        let io_error = |path: &Path, error: std::io::Error| WorkspaceWriteError::Io {
            path: path.to_path_buf(),
            message: error.to_string(),
        };

        let directory = resolved.parent().ok_or_else(|| WorkspaceWriteError::Io {
            path: resolved.clone(),
            message: "target path has no parent directory".to_string(),
        })?;
        let file_name = resolved
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| WorkspaceWriteError::Io {
                path: resolved.clone(),
                message: "target path has no file name".to_string(),
            })?;
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.subsec_nanos())
            .unwrap_or(0);
        let temp_path = directory.join(format!(".{file_name}.{}.{nanos}.tmp", std::process::id()));

        let mut temp_file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .map_err(|error| io_error(&temp_path, error))?;

        let write_result = temp_file
            .write_all(contents.as_bytes())
            .and_then(|()| temp_file.sync_all());
        if let Err(error) = write_result {
            let _ = std::fs::remove_file(&temp_path);
            return Err(io_error(&temp_path, error));
        }
        drop(temp_file);

        // TOCTOU guard: the on-disk file must still match the bytes the edit
        // plan was computed against, immediately before the rename.
        let current = match std::fs::read(&resolved) {
            Ok(bytes) => bytes,
            Err(error) => {
                let _ = std::fs::remove_file(&temp_path);
                return Err(io_error(&resolved, error));
            }
        };
        if sha256_prefixed(&current) != expected_current_hash {
            let _ = std::fs::remove_file(&temp_path);
            return Err(WorkspaceWriteError::ConcurrentModification { path: resolved });
        }

        // `resolved` is canonical, so its leaf was not a symlink when the
        // containment check ran; one swapped in since would make the hash
        // check read through the link while the rename replaces the link
        // itself, orphaning the linked file. Refuse when observed. This
        // guards a race window no deterministic test can open; the residual
        // instant before `rename(2)` is accepted (see the port docs).
        match std::fs::symlink_metadata(&resolved) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                let _ = std::fs::remove_file(&temp_path);
                return Err(WorkspaceWriteError::ConcurrentModification { path: resolved });
            }
            Ok(_) => {}
            Err(error) => {
                let _ = std::fs::remove_file(&temp_path);
                return Err(io_error(&resolved, error));
            }
        }

        std::fs::rename(&temp_path, &resolved).map_err(|error| {
            let _ = std::fs::remove_file(&temp_path);
            io_error(&resolved, error)
        })?;

        // Persist the rename's dirent update, not just the temp file's
        // bytes. Best-effort: the write already succeeded, and reporting a
        // directory-sync failure as an apply failure would lie about the
        // file's contents (opening a directory also fails on non-Unix).
        let _ = std::fs::File::open(directory).and_then(|dir| dir.sync_all());
        Ok(())
    }
}

fn resolve_through_nearest_existing_ancestor(path: &Path) -> PathBuf {
    let mut ancestor = path.to_path_buf();
    let mut missing_suffix = Vec::new();

    while !ancestor.exists() {
        let Some(name) = ancestor.file_name().map(|name| name.to_os_string()) else {
            break;
        };
        missing_suffix.push(name);
        if !ancestor.pop() {
            break;
        }
    }

    let mut resolved = std::fs::canonicalize(&ancestor).unwrap_or(ancestor);
    for segment in missing_suffix.iter().rev() {
        resolved.push(segment);
    }
    resolved
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(Path::new("/")),
            Component::CurDir => {}
            Component::ParentDir => normalized.push(".."),
            Component::Normal(segment) => normalized.push(segment),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "adoc-fs-writer-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn write_atomic_replaces_contents_when_hash_matches() {
        let root = temp_dir("happy");
        let target = root.join("doc.adoc");
        std::fs::write(&target, "before").expect("seed");
        let writer = FsWorkspaceWriter::new(&root);

        writer
            .write_atomic(&target, "after", &sha256_prefixed(b"before"))
            .expect("writes");

        assert_eq!(std::fs::read_to_string(&target).expect("read"), "after");
        // No temp files left behind.
        let leftovers: Vec<_> = std::fs::read_dir(&root)
            .expect("read dir")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "temp file must be cleaned up");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn write_atomic_refuses_on_concurrent_modification_and_writes_nothing() {
        let root = temp_dir("toctou");
        let target = root.join("doc.adoc");
        std::fs::write(&target, "moved on").expect("seed");
        let writer = FsWorkspaceWriter::new(&root);

        let error = writer
            .write_atomic(&target, "after", &sha256_prefixed(b"planned-against"))
            .expect_err("must refuse");

        assert!(matches!(
            error,
            WorkspaceWriteError::ConcurrentModification { .. }
        ));
        assert_eq!(std::fs::read_to_string(&target).expect("read"), "moved on");
        std::fs::remove_dir_all(&root).ok();
    }

    /// A symlink present when `write_atomic` starts is canonicalized by the
    /// containment check: the write lands on the real file and the link
    /// survives, still pointing at it.
    #[test]
    #[cfg(unix)]
    fn write_atomic_writes_through_a_preexisting_symlink() {
        let root = temp_dir("symlink-through");
        let real = root.join("real.adoc");
        let link = root.join("link.adoc");
        std::fs::write(&real, "before").expect("seed");
        std::os::unix::fs::symlink(&real, &link).expect("symlink");
        let writer = FsWorkspaceWriter::new(&root);

        writer
            .write_atomic(&link, "after", &sha256_prefixed(b"before"))
            .expect("writes");

        assert_eq!(std::fs::read_to_string(&real).expect("read"), "after");
        assert!(
            std::fs::symlink_metadata(&link)
                .expect("stat link")
                .file_type()
                .is_symlink(),
            "the link itself must survive the write"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rejects_paths_outside_the_sandbox_root() {
        let root = temp_dir("sandbox");
        let writer = FsWorkspaceWriter::new(&root);

        let escape = Path::new("../escape.adoc");
        assert!(matches!(
            writer.read_to_string(escape),
            Err(WorkspaceWriteError::OutsideSandbox { .. })
        ));
        assert!(matches!(
            writer.write_atomic(Path::new("/etc/adoc-escape"), "x", "sha256:0"),
            Err(WorkspaceWriteError::OutsideSandbox { .. })
        ));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn read_to_string_resolves_relative_paths_under_the_root() {
        let root = temp_dir("relative");
        std::fs::create_dir_all(root.join("docs")).expect("mkdir");
        std::fs::write(root.join("docs/page.adoc"), "content").expect("seed");
        let writer = FsWorkspaceWriter::new(&root);

        assert_eq!(
            writer
                .read_to_string(Path::new("docs/page.adoc"))
                .expect("reads"),
            "content"
        );
        std::fs::remove_dir_all(&root).ok();
    }
}
