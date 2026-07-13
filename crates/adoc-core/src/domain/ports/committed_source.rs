use std::path::Path;

/// Reports whether a source can be removed with a committed version as its
/// recovery point. The concrete Git adapter owns repository inspection.
pub(crate) trait CommittedSourceProbe {
    fn is_committed_and_clean(&self, source: &Path) -> bool;
}
