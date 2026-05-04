use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::source::SourceFile;

/// Adapter trait for the input side of the compiler.
///
/// `compile_workspace` defers all filesystem walking and reading to a
/// [`SourceProvider`]. The default adapter is [`FsSourceProvider`]; tests can
/// substitute [`InMemorySourceProvider`] to exercise the orchestration logic
/// without touching disk.
pub(crate) trait SourceProvider {
    fn load_sources(&self) -> Vec<Result<SourceFile, SourceLoadError>>;
}

/// Reported by a [`SourceProvider`] when a single source cannot be loaded.
///
/// `compile_workspace` translates each error into an `io.unreadable_file`
/// diagnostic; the original I/O message is preserved verbatim so the CLI
/// surface stays unchanged.
#[derive(Debug, Clone)]
pub(crate) struct SourceLoadError {
    pub path: PathBuf,
    pub message: String,
}

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
        let mut results = Vec::new();
        for source_path in source_paths(&self.root) {
            match fs::read_to_string(&source_path.path) {
                Ok(text) => results.push(Ok(SourceFile::new_with_identity_path(
                    source_path.path,
                    text,
                    source_path.identity_path,
                ))),
                Err(error) => results.push(Err(SourceLoadError {
                    path: source_path.path,
                    message: error.to_string(),
                })),
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
        } else if path
            .extension()
            .is_some_and(|extension| extension == "adoc")
        {
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

/// In-memory adapter for unit tests. Yields the supplied results verbatim.
#[cfg(test)]
#[derive(Debug, Default, Clone)]
pub(crate) struct InMemorySourceProvider {
    results: Vec<Result<SourceFile, SourceLoadError>>,
}

#[cfg(test)]
impl InMemorySourceProvider {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn with_source(mut self, source: SourceFile) -> Self {
        self.results.push(Ok(source));
        self
    }

    pub(crate) fn with_error(mut self, path: PathBuf, message: impl Into<String>) -> Self {
        self.results.push(Err(SourceLoadError {
            path,
            message: message.into(),
        }));
        self
    }
}

#[cfg(test)]
impl SourceProvider for InMemorySourceProvider {
    fn load_sources(&self) -> Vec<Result<SourceFile, SourceLoadError>> {
        self.results.clone()
    }
}
