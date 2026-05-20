use std::path::{Component, Path, PathBuf};

use crate::LocalError;

pub trait PathPolicy: Clone + Send + Sync + 'static {
    fn resolve_read_path(&self, path: &Path) -> Result<PathBuf, LocalError>;
    fn resolve_write_path(&self, path: &Path) -> Result<PathBuf, LocalError>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct UnrestrictedPathPolicy;

impl PathPolicy for UnrestrictedPathPolicy {
    fn resolve_read_path(&self, path: &Path) -> Result<PathBuf, LocalError> {
        Ok(path.to_path_buf())
    }

    fn resolve_write_path(&self, path: &Path) -> Result<PathBuf, LocalError> {
        Ok(path.to_path_buf())
    }
}

#[derive(Debug, Clone)]
pub struct ProjectRootPathPolicy {
    project_root: PathBuf,
}

impl ProjectRootPathPolicy {
    pub fn new(project_root: impl AsRef<Path>) -> Result<Self, LocalError> {
        Ok(Self {
            project_root: absolute_canonical_root(project_root.as_ref())?,
        })
    }

    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    fn resolve_project_path(&self, path: &Path) -> Result<PathBuf, LocalError> {
        if has_parent_component(path) {
            return Err(LocalError::PathOutsideProject {
                path: path.to_path_buf(),
                project_root: self.project_root.clone(),
            });
        }

        let candidate = if path.is_absolute() {
            normalize_path(path)
        } else {
            normalize_path(&self.project_root.join(path))
        };
        let resolved = resolve_through_nearest_existing_ancestor(&candidate);
        if resolved.starts_with(&self.project_root) {
            Ok(resolved)
        } else {
            Err(LocalError::PathOutsideProject {
                path: candidate,
                project_root: self.project_root.clone(),
            })
        }
    }
}

impl PathPolicy for ProjectRootPathPolicy {
    fn resolve_read_path(&self, path: &Path) -> Result<PathBuf, LocalError> {
        self.resolve_project_path(path)
    }

    fn resolve_write_path(&self, path: &Path) -> Result<PathBuf, LocalError> {
        self.resolve_project_path(path)
    }
}

fn absolute_canonical_root(path: &Path) -> Result<PathBuf, LocalError> {
    if let Ok(canonical) = std::fs::canonicalize(path) {
        return Ok(canonical);
    }

    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|source| LocalError::CurrentDir { source })?
            .join(path)
    };
    Ok(normalize_path(&absolute))
}

fn has_parent_component(path: &Path) -> bool {
    path.components()
        .any(|component| component == Component::ParentDir)
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
