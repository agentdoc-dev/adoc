use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::ports::source_provider::{SourceLoadError, SourceProvider};
use crate::domain::source::SourceFile;

/// Reads `.adoc` files from a directory tree (or a single file) on disk.
///
/// Iteration order is the lexicographic ordering of paths so that compilation
/// is deterministic across platforms.
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
        if self.root.is_file() && !is_adoc_source_path(&self.root) {
            return vec![Err(SourceLoadError::unsupported_source_extension(
                self.root.clone(),
            ))];
        }

        let mut results = Vec::new();
        for source_path in source_paths(&self.root) {
            match fs::read_to_string(&source_path.path) {
                Ok(text) => results.push(Ok(SourceFile::new_with_identity_path(
                    source_path.path,
                    text,
                    source_path.identity_path,
                ))),
                Err(error) => results.push(Err(SourceLoadError::unreadable(
                    source_path.path,
                    error.to_string(),
                ))),
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

fn source_paths(root: &Path) -> Vec<SourcePath> {
    if root.is_file() {
        return vec![SourcePath {
            path: root.to_path_buf(),
            identity_path: root
                .file_name()
                .map(PathBuf::from)
                .unwrap_or_else(|| root.into()),
        }];
    }

    let mut paths = Vec::new();
    collect_adoc_files(root, root, &mut paths);
    paths.sort_by(|left, right| left.path.cmp(&right.path));
    paths
}

fn collect_adoc_files(root: &Path, directory: &Path, paths: &mut Vec<SourcePath>) {
    let Ok(entries) = fs::read_dir(directory) else {
        paths.push(SourcePath {
            path: directory.to_path_buf(),
            identity_path: directory
                .strip_prefix(root)
                .map(PathBuf::from)
                .unwrap_or_else(|_| directory.into()),
        });
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_adoc_files(root, &path, paths);
        } else if is_adoc_source_path(&path) {
            let identity_path = path
                .strip_prefix(root)
                .map(PathBuf::from)
                .unwrap_or_else(|_| path.clone());
            paths.push(SourcePath {
                path,
                identity_path,
            });
        }
    }
}

fn is_adoc_source_path(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension == "adoc")
}
