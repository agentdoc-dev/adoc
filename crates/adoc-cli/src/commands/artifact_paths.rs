use std::fs;
use std::path::{Path, PathBuf};

/// Remove a stale output so a failed run never leaves a previous artifact
/// behind for a consumer to mistake for this run's result.
pub(crate) fn remove_stale(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "could not remove stale output {}: {error}",
            path.display()
        )),
    }
}

/// Reject aliased input/output paths (relative spellings, symlinks) before
/// an output write could clobber an input.
pub(crate) fn ensure_distinct_paths(paths: &[&Path]) -> Result<(), String> {
    let identities = paths
        .iter()
        .map(|path| path_identity(path))
        .collect::<Vec<_>>();
    if identities
        .iter()
        .enumerate()
        .any(|(index, path)| identities[..index].contains(path))
    {
        return Err("input and output artifact paths must be distinct".to_string());
    }
    Ok(())
}

fn path_identity(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| {
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::canonicalize(parent)
            .map(|parent| {
                path.file_name()
                    .map_or(parent.clone(), |name| parent.join(name))
            })
            .unwrap_or_else(|_| path.to_path_buf())
    })
}
