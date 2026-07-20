//! Filesystem adapter for [`EvidenceFileReader`] (V8.5.1, ADR-0048).

use std::path::PathBuf;

use crate::domain::ports::evidence_file::{EvidenceFileRead, EvidenceFileReader};
use crate::domain::value_objects::rel_path::RelPath;

/// Reads anchored evidence files relative to the project's anchor root
/// (the discovered config directory, else the context start directory).
/// Containment is lexical only: `RelPath` rejects `..` segments and
/// absolute paths, but `std::fs::read` follows symlinks, so a symlinked
/// target resolves wherever the filesystem points. Accepted (ADR-0048):
/// this is a read-only pass over an author-controlled tree, and refusing
/// symlinks would break legitimate in-repo links.
pub(crate) struct FsEvidenceFileReader {
    root: PathBuf,
}

impl FsEvidenceFileReader {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

impl EvidenceFileReader for FsEvidenceFileReader {
    fn read(&self, path: &RelPath) -> EvidenceFileRead {
        let full = self.root.join(path.as_str());
        match std::fs::read(&full) {
            Ok(bytes) => EvidenceFileRead::Found(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => EvidenceFileRead::Missing,
            Err(error) => EvidenceFileRead::Unreadable(error.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_distinguishes_found_and_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("cited.rs"), b"fn main() {}").expect("write");
        let reader = FsEvidenceFileReader::new(dir.path().to_path_buf());

        let found = reader.read(&RelPath::try_new("cited.rs").expect("rel path"));
        assert_eq!(found, EvidenceFileRead::Found(b"fn main() {}".to_vec()));

        let missing = reader.read(&RelPath::try_new("absent.rs").expect("rel path"));
        assert_eq!(missing, EvidenceFileRead::Missing);
    }
}
