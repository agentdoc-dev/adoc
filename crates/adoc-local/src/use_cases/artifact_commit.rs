use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::LocalError;

static NEXT_AUXILIARY_ID: AtomicU64 = AtomicU64::new(0);

pub(super) struct ArtifactWrite {
    pub(super) path: PathBuf,
    pub(super) contents: Vec<u8>,
}

struct StagedArtifact {
    target: PathBuf,
    staged: PathBuf,
    backup: Option<PathBuf>,
    promoted: bool,
}

trait ArtifactFileSystem {
    fn create_dir_all(&self, path: &Path) -> io::Result<()>;
    fn write_new_synced(&self, path: &Path, contents: &[u8]) -> io::Result<()>;
    fn try_exists(&self, path: &Path) -> io::Result<bool>;
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;
    fn remove_file(&self, path: &Path) -> io::Result<()>;
}

struct RealArtifactFileSystem;

impl ArtifactFileSystem for RealArtifactFileSystem {
    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        fs::create_dir_all(path)
    }

    fn write_new_synced(&self, path: &Path, contents: &[u8]) -> io::Result<()> {
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        if let Err(error) = file.write_all(contents).and_then(|()| file.sync_all()) {
            drop(file);
            let _ = fs::remove_file(path);
            return Err(error);
        }
        Ok(())
    }

    fn try_exists(&self, path: &Path) -> io::Result<bool> {
        path.try_exists()
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        fs::rename(from, to)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)
    }
}

pub(super) fn commit_artifact_set(entries: Vec<ArtifactWrite>) -> Result<(), LocalError> {
    commit_artifact_set_with(&RealArtifactFileSystem, entries)
}

fn commit_artifact_set_with(
    file_system: &impl ArtifactFileSystem,
    entries: Vec<ArtifactWrite>,
) -> Result<(), LocalError> {
    reject_duplicate_targets(&entries)?;

    let mut staged = Vec::with_capacity(entries.len());
    for entry in entries {
        let parent = artifact_parent(&entry.path);
        if let Err(source) = file_system.create_dir_all(parent) {
            return Err(fail_with_rollback(
                file_system,
                &mut staged,
                "create directory",
                parent.to_path_buf(),
                source,
            ));
        }

        let staged_path = match unused_auxiliary_path(file_system, &entry.path, "stage") {
            Ok(path) => path,
            Err(source) => {
                return Err(fail_with_rollback(
                    file_system,
                    &mut staged,
                    "allocate staging file",
                    entry.path,
                    source,
                ));
            }
        };
        if let Err(source) = file_system.write_new_synced(&staged_path, &entry.contents) {
            return Err(fail_with_rollback(
                file_system,
                &mut staged,
                "stage",
                entry.path,
                source,
            ));
        }
        staged.push(StagedArtifact {
            target: entry.path,
            staged: staged_path,
            backup: None,
            promoted: false,
        });
    }

    for index in 0..staged.len() {
        let target = staged[index].target.clone();
        let target_exists = match file_system.try_exists(&target) {
            Ok(exists) => exists,
            Err(source) => {
                return Err(fail_with_rollback(
                    file_system,
                    &mut staged,
                    "inspect target",
                    target,
                    source,
                ));
            }
        };

        if target_exists {
            let backup = match unused_auxiliary_path(file_system, &target, "backup") {
                Ok(path) => path,
                Err(source) => {
                    return Err(fail_with_rollback(
                        file_system,
                        &mut staged,
                        "allocate backup",
                        target,
                        source,
                    ));
                }
            };
            if let Err(source) = file_system.rename(&target, &backup) {
                return Err(fail_with_rollback(
                    file_system,
                    &mut staged,
                    "backup",
                    target,
                    source,
                ));
            }
            staged[index].backup = Some(backup);
        }

        let stage = staged[index].staged.clone();
        if let Err(source) = file_system.rename(&stage, &target) {
            return Err(fail_with_rollback(
                file_system,
                &mut staged,
                "promote",
                target,
                source,
            ));
        }
        staged[index].promoted = true;
    }

    for artifact in staged {
        if let Some(backup) = artifact.backup
            && let Err(error) = remove_if_present(file_system, &backup)
        {
            tracing::warn!(path = %backup.display(), %error, "could not remove artifact backup");
        }
    }
    Ok(())
}

fn reject_duplicate_targets(entries: &[ArtifactWrite]) -> Result<(), LocalError> {
    let mut targets = BTreeSet::new();
    for entry in entries {
        if !targets.insert(&entry.path) {
            return Err(LocalError::ArtifactCommitFailed {
                phase: "validate",
                path: entry.path.clone(),
                rollback_failed: Vec::new(),
                source: io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "artifact output paths must be distinct",
                ),
            });
        }
    }
    Ok(())
}

fn fail_with_rollback(
    file_system: &impl ArtifactFileSystem,
    artifacts: &mut [StagedArtifact],
    phase: &'static str,
    path: PathBuf,
    source: io::Error,
) -> LocalError {
    LocalError::ArtifactCommitFailed {
        phase,
        path,
        rollback_failed: rollback(file_system, artifacts),
        source,
    }
}

fn rollback(
    file_system: &impl ArtifactFileSystem,
    artifacts: &mut [StagedArtifact],
) -> Vec<PathBuf> {
    let mut failures = Vec::new();
    for artifact in artifacts.iter_mut().rev() {
        if artifact.promoted {
            if let Err(error) = remove_if_present(file_system, &artifact.target) {
                tracing::error!(path = %artifact.target.display(), %error, "artifact rollback remove failed");
                failures.push(artifact.target.clone());
                continue;
            }
            artifact.promoted = false;
        }
        if let Some(backup) = artifact.backup.take()
            && let Err(error) = file_system.rename(&backup, &artifact.target)
        {
            tracing::error!(path = %artifact.target.display(), %error, "artifact rollback restore failed");
            failures.push(artifact.target.clone());
        }
        if let Err(error) = remove_if_present(file_system, &artifact.staged) {
            tracing::error!(path = %artifact.staged.display(), %error, "artifact staging cleanup failed");
            failures.push(artifact.staged.clone());
        }
    }
    failures
}

fn unused_auxiliary_path(
    file_system: &impl ArtifactFileSystem,
    target: &Path,
    role: &str,
) -> io::Result<PathBuf> {
    for _ in 0..100 {
        let id = NEXT_AUXILIARY_ID.fetch_add(1, Ordering::Relaxed);
        let mut name = OsString::from(".");
        name.push(target.file_name().unwrap_or_else(|| OsStr::new("artifact")));
        name.push(format!(".adoc-{role}-{}-{id}", std::process::id()));
        let candidate = artifact_parent(target).join(name);
        if !file_system.try_exists(&candidate)? {
            return Ok(candidate);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a unique artifact staging path",
    ))
}

fn artifact_parent(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn remove_if_present(file_system: &impl ArtifactFileSystem, path: &Path) -> io::Result<()> {
    match file_system.remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    struct FailingFileSystem {
        real: RealArtifactFileSystem,
        fail_write_at: Option<usize>,
        fail_rename_at: Option<usize>,
        writes: Cell<usize>,
        renames: Cell<usize>,
    }

    impl FailingFileSystem {
        fn at_write(operation: usize) -> Self {
            Self {
                real: RealArtifactFileSystem,
                fail_write_at: Some(operation),
                fail_rename_at: None,
                writes: Cell::new(0),
                renames: Cell::new(0),
            }
        }

        fn at_rename(operation: usize) -> Self {
            Self {
                real: RealArtifactFileSystem,
                fail_write_at: None,
                fail_rename_at: Some(operation),
                writes: Cell::new(0),
                renames: Cell::new(0),
            }
        }
    }

    impl ArtifactFileSystem for FailingFileSystem {
        fn create_dir_all(&self, path: &Path) -> io::Result<()> {
            self.real.create_dir_all(path)
        }

        fn write_new_synced(&self, path: &Path, contents: &[u8]) -> io::Result<()> {
            let operation = self.writes.get() + 1;
            self.writes.set(operation);
            if self.fail_write_at == Some(operation) {
                Err(io::Error::other("injected staging failure"))
            } else {
                self.real.write_new_synced(path, contents)
            }
        }

        fn try_exists(&self, path: &Path) -> io::Result<bool> {
            self.real.try_exists(path)
        }

        fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
            let operation = self.renames.get() + 1;
            self.renames.set(operation);
            if self.fail_rename_at == Some(operation) {
                Err(io::Error::other("injected rename failure"))
            } else {
                self.real.rename(from, to)
            }
        }

        fn remove_file(&self, path: &Path) -> io::Result<()> {
            self.real.remove_file(path)
        }
    }

    fn entries(root: &Path, prefix: &str) -> Vec<ArtifactWrite> {
        ["docs.html", "docs.graph.json", "docs.search.json"]
            .into_iter()
            .map(|name| ArtifactWrite {
                path: root.join(name),
                contents: format!("{prefix}:{name}").into_bytes(),
            })
            .collect()
    }

    fn seed(root: &Path, prefix: &str) {
        for entry in entries(root, prefix) {
            fs::write(entry.path, entry.contents).expect("seed artifact");
        }
    }

    fn assert_set(root: &Path, prefix: &str) {
        for entry in entries(root, prefix) {
            assert_eq!(fs::read(entry.path).expect("read artifact"), entry.contents);
        }
        let auxiliary = fs::read_dir(root)
            .expect("read output directory")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".adoc-"))
            .collect::<Vec<_>>();
        assert!(auxiliary.is_empty(), "auxiliary files leaked");
    }

    fn assert_no_artifacts(root: &Path) {
        for entry in entries(root, "unused") {
            assert!(!entry.path.exists(), "partial artifact set was published");
        }
        let auxiliary = fs::read_dir(root)
            .expect("read output directory")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".adoc-"))
            .collect::<Vec<_>>();
        assert!(auxiliary.is_empty(), "auxiliary files leaked");
    }

    #[test]
    fn staging_failure_preserves_the_previous_artifact_set() {
        let directory = tempfile::tempdir().expect("tempdir");
        seed(directory.path(), "old");

        let error = commit_artifact_set_with(
            &FailingFileSystem::at_write(2),
            entries(directory.path(), "new"),
        )
        .expect_err("second staged write fails");

        assert!(matches!(
            error,
            LocalError::ArtifactCommitFailed { phase: "stage", .. }
        ));
        assert_set(directory.path(), "old");
    }

    #[test]
    fn promotion_failure_rolls_back_every_artifact() {
        let directory = tempfile::tempdir().expect("tempdir");
        seed(directory.path(), "old");

        let error = commit_artifact_set_with(
            &FailingFileSystem::at_rename(4),
            entries(directory.path(), "new"),
        )
        .expect_err("graph promotion fails after html promotion");

        assert!(matches!(
            error,
            LocalError::ArtifactCommitFailed {
                phase: "promote",
                rollback_failed,
                ..
            } if rollback_failed.is_empty()
        ));
        assert_set(directory.path(), "old");
    }

    #[test]
    fn promotion_failure_removes_new_outputs_when_no_previous_set_exists() {
        let directory = tempfile::tempdir().expect("tempdir");

        commit_artifact_set_with(
            &FailingFileSystem::at_rename(2),
            entries(directory.path(), "new"),
        )
        .expect_err("graph promotion fails after html promotion");

        assert_no_artifacts(directory.path());
    }

    #[test]
    fn successful_commit_replaces_the_complete_set_and_cleans_backups() {
        let directory = tempfile::tempdir().expect("tempdir");
        seed(directory.path(), "old");

        commit_artifact_set(entries(directory.path(), "new")).expect("commit artifact set");

        assert_set(directory.path(), "new");
    }
}
