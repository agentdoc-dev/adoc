use std::path::Path;

use crate::domain::diagnostic::Diagnostic;

/// Input port for structured artifacts consumed by read-side application code.
///
/// Each adapter declares its own `Output` shape so retrieval can depend on a
/// stable read boundary while `lib.rs` chooses the concrete artifact format.
pub(crate) trait ArtifactReader {
    type Output;

    fn read(&self, path: &Path) -> Result<Self::Output, Vec<Diagnostic>>;
}
