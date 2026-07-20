//! Adapter trait for reading cited evidence files at check time
//! (V8.5.1, ADR-0048).
//!
//! The Evidence Anchor pass re-hashes files that `source` objects cite via
//! `path`. All filesystem access goes through this port so the pass stays a
//! pure function of `(workspace, reader)`; the fs adapter lives in
//! `infrastructure/source/evidence_fs.rs` and tests substitute in-memory
//! fakes.

use crate::domain::value_objects::rel_path::RelPath;

pub(crate) trait EvidenceFileReader {
    fn read(&self, path: &RelPath) -> EvidenceFileRead;
}

/// Outcome of one evidence-file read. `Missing` and `Unreadable` are
/// distinct on purpose: a deleted target and a permission failure need
/// different remediation messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EvidenceFileRead {
    Found(Vec<u8>),
    Missing,
    Unreadable(String),
}
