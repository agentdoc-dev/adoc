use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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

/// Replace an artifact atomically so an interrupted write can leave only a
/// disposable sibling temp file, never a truncated final artifact.
pub(crate) fn write_atomic(path: &Path, contents: &[u8]) -> Result<(), String> {
    let name = path
        .file_name()
        .ok_or_else(|| format!("output path {} has no file name", path.display()))?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.subsec_nanos())
        .unwrap_or(0);
    let temp = path.with_file_name(format!(
        ".{}.{}.{nonce}.tmp",
        name.to_string_lossy(),
        std::process::id()
    ));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|error| format!("could not write {}: {error}", temp.display()))?;
    if let Err(error) = file.write_all(contents).and_then(|()| file.sync_all()) {
        let _ = fs::remove_file(&temp);
        return Err(format!("could not write {}: {error}", temp.display()));
    }
    drop(file);
    fs::rename(&temp, path).map_err(|error| {
        let _ = fs::remove_file(&temp);
        format!("could not write {}: {error}", path.display())
    })?;
    if let Some(directory) = path.parent() {
        let _ = fs::File::open(directory).and_then(|directory| directory.sync_all());
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_replaces_the_artifact_without_leaving_a_temp_file() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock is after epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "adoc-artifact-write-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&directory).expect("creates test directory");
        let path = directory.join("proposal.json");
        fs::write(&path, b"old").expect("writes old artifact");

        write_atomic(&path, b"new\n").expect("replaces artifact");

        assert_eq!(fs::read(&path).expect("reads artifact"), b"new\n");
        assert_eq!(
            fs::read_dir(&directory).expect("lists directory").count(),
            1
        );
        fs::remove_dir_all(directory).expect("cleans test directory");
    }
}
