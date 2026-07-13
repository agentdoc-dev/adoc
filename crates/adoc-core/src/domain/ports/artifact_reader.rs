use std::path::Path;

use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArtifactReadErrorKind {
    Missing,
    Unreadable,
    Malformed,
    UnsupportedVersion,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ArtifactReadError {
    kind: ArtifactReadErrorKind,
    schema_version: Option<String>,
    diagnostics: Vec<Diagnostic>,
}

impl ArtifactReadError {
    pub(crate) fn new(kind: ArtifactReadErrorKind, diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            kind,
            schema_version: None,
            diagnostics,
        }
    }

    pub(crate) fn from_diagnostics(diagnostics: Vec<Diagnostic>) -> Self {
        let kind = if diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::IoArtifactMissing)
        {
            ArtifactReadErrorKind::Missing
        } else if diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::SchemaUnsupportedVersion)
        {
            ArtifactReadErrorKind::UnsupportedVersion
        } else if diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::IoArtifactMalformed)
        {
            ArtifactReadErrorKind::Malformed
        } else {
            ArtifactReadErrorKind::Unreadable
        };
        Self::new(kind, diagnostics)
    }

    pub(crate) fn with_kind(mut self, kind: ArtifactReadErrorKind) -> Self {
        self.kind = kind;
        self
    }

    pub(crate) fn with_schema_version(mut self, schema_version: Option<String>) -> Self {
        self.schema_version = schema_version;
        self
    }

    pub(crate) fn kind(&self) -> ArtifactReadErrorKind {
        self.kind
    }

    pub(crate) fn schema_version(&self) -> Option<&str> {
        self.schema_version.as_deref()
    }

    pub(crate) fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

/// Input port for structured artifacts consumed by read-side application code.
///
/// Each adapter declares its own `Output` shape so retrieval can depend on a
/// stable read boundary while `lib.rs` chooses the concrete artifact format.
pub(crate) trait ArtifactReader {
    type Output;

    fn read(&self, path: &Path) -> Result<Self::Output, ArtifactReadError>;
}
