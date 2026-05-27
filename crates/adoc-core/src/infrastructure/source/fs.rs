use std::path::{Path, PathBuf};
use std::{fs, io};

use crate::domain::ports::source_provider::{SourceLoadError, SourceProvider};
use crate::domain::source::SourceFile;

/// Reads `.adoc` files from a directory tree (or a single file) on disk.
///
/// Iteration order is the lexicographic ordering of paths so that compilation
/// is deterministic across platforms.
///
/// Callers that need identity-rebased paths (the V3 review pipeline, which
/// compares two snapshots of the same source tree at different filesystem
/// roots) wrap this provider via the trait-level
/// [`SourceProvider::with_identity_prefix`] decorator — there is no
/// inherent rebase constructor on `FsSourceProvider`.
#[derive(Debug, Clone)]
pub(crate) struct FsSourceProvider {
    root: PathBuf,
}

impl FsSourceProvider {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

impl SourceProvider for FsSourceProvider {
    fn load_sources(&self) -> Vec<Result<SourceFile, SourceLoadError>> {
        if self.root.is_file() && !is_source_path(&self.root) {
            return vec![Err(SourceLoadError::unsupported_source_extension(
                self.root.clone(),
            ))];
        }

        let mut results = Vec::new();
        for source_path in source_paths(&self.root) {
            match source_path {
                Ok(source_path) => match fs::read_to_string(&source_path.path) {
                    Ok(text) => {
                        results.push(Ok(SourceFile::new_with_identity_path(
                            source_path.path,
                            text,
                            source_path.identity_path,
                        )));
                    }
                    Err(error) => results.push(Err(SourceLoadError::unreadable(
                        source_path.path,
                        error.to_string(),
                    ))),
                },
                Err(error) => results.push(Err(error)),
            }
        }
        results
    }
}

#[derive(Debug, Clone)]
struct SourcePath {
    path: PathBuf,
    identity_path: PathBuf,
}

fn source_paths(root: &Path) -> Vec<Result<SourcePath, SourceLoadError>> {
    if root.is_file() {
        return vec![Ok(SourcePath {
            path: root.to_path_buf(),
            identity_path: root
                .file_name()
                .map(PathBuf::from)
                .unwrap_or_else(|| root.into()),
        })];
    }

    if !root.exists() {
        return vec![Ok(SourcePath {
            path: root.to_path_buf(),
            identity_path: root
                .file_name()
                .map(PathBuf::from)
                .unwrap_or_else(|| root.into()),
        })];
    }

    let mut paths = Vec::new();
    collect_source_files(root, root, &mut paths);
    paths.sort_by(|left, right| source_path_result_path(left).cmp(source_path_result_path(right)));
    paths
}

fn collect_source_files(
    root: &Path,
    directory: &Path,
    paths: &mut Vec<Result<SourcePath, SourceLoadError>>,
) {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            paths.push(source_path_for_unreadable_directory(root, directory, error));
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_source_files(root, &path, paths);
        } else if is_source_path(&path) {
            let identity_path = path
                .strip_prefix(root)
                .map(PathBuf::from)
                .unwrap_or_else(|_| path.clone());
            paths.push(Ok(SourcePath {
                path,
                identity_path,
            }));
        }
    }
}

fn source_path_for_unreadable_directory(
    _root: &Path,
    directory: &Path,
    error: io::Error,
) -> Result<SourcePath, SourceLoadError> {
    Err(SourceLoadError::unreadable_directory(
        directory.to_path_buf(),
        error.to_string(),
    ))
}

fn source_path_result_path(result: &Result<SourcePath, SourceLoadError>) -> &Path {
    match result {
        Ok(source_path) => &source_path.path,
        Err(load_error) => &load_error.path,
    }
}

fn is_source_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension == "adoc" || extension == "md")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    #[cfg(unix)]
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::domain::ports::source_provider::SourceLoadErrorKind;

    #[test]
    fn source_paths_preserves_unreadable_directory_as_load_error() {
        let root = Path::new("/workspace");
        let blocked = root.join("blocked");
        let error = io::Error::new(io::ErrorKind::PermissionDenied, "permission denied");

        let result = source_path_for_unreadable_directory(root, &blocked, error);

        let load_error = result.expect_err("unreadable directory should be a load error");
        assert_eq!(load_error.path, blocked);
        assert_eq!(load_error.kind, SourceLoadErrorKind::UnreadableDirectory);
        assert!(load_error.message.contains("permission denied"));
    }

    #[cfg(unix)]
    #[test]
    fn fs_source_provider_reports_read_dir_failure_as_unreadable_directory() {
        let temp_root = TempRoot::new("adoc-unreadable-directory");
        let root = temp_root.path();
        fs::write(root.join("readable.adoc"), "# Readable\n").expect("write readable source");
        let blocked = root.join("blocked");
        fs::create_dir(&blocked).expect("create blocked directory");
        fs::set_permissions(&blocked, fs::Permissions::from_mode(0o000))
            .expect("make blocked directory unreadable");

        let results = FsSourceProvider::new(root.to_path_buf()).load_sources();

        fs::set_permissions(&blocked, fs::Permissions::from_mode(0o700))
            .expect("restore blocked directory permissions before assertions");

        let load_error = results
            .iter()
            .find_map(|result| match result {
                Err(error) if error.kind == SourceLoadErrorKind::UnreadableDirectory => Some(error),
                _ => None,
            })
            .expect("provider should preserve read_dir failure as unreadable directory");

        assert_eq!(load_error.path, blocked);
        assert!(
            load_error
                .message
                .to_ascii_lowercase()
                .contains("permission denied"),
            "expected real permission error, got: {}",
            load_error.message
        );
        assert!(
            !load_error
                .message
                .to_ascii_lowercase()
                .contains("is a directory"),
            "directory traversal failure must not be reported through read_to_string: {}",
            load_error.message
        );
    }

    #[cfg(unix)]
    struct TempRoot {
        path: PathBuf,
    }

    #[cfg(unix)]
    impl TempRoot {
        fn new(label: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time is after unix epoch")
                .as_nanos();
            let path =
                std::env::temp_dir().join(format!("{}-{}-{}", label, std::process::id(), unique));
            fs::create_dir(&path).expect("create temp root");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    #[cfg(unix)]
    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
