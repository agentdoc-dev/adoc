use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de, ser::SerializeStruct};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub severity: Severity,
    pub message: String,
    pub span: Option<SourceSpan>,
    pub object_id: Option<String>,
    pub help: Option<String>,
}

/// Semantic identifier for a diagnostic.
///
/// Per ADR-0005, this is part of the public surface as of v0.x — promoted from
/// `pub(crate)` so external consumers (the CLI today, future LSP/web hosts
/// tomorrow) can pattern-match on it instead of comparing strings. The wire
/// format remains the dotted code string (`parse.raw_html`,
/// `io.unreadable_file`, `io.unreadable_directory`, etc.); the manual
/// `Serialize` impl below preserves byte-identical JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticCode {
    ParseRawHtml,
    ParseUnsafeLink,
    ParseUnclosedFence,
    ParseMalformedPageAnnotation,
    ParseNestedTypedBlock,
    ParseMalformedField,
    ParseMalformedOpenFence,
    SchemaUnknownKind,
    SchemaMissingField,
    SchemaDuplicateField,
    SchemaInvalidStatus,
    ClaimVerifiedMissingEvidence,
    ClaimStatusCasing,
    IdDuplicate,
    IdInvalid,
    RefBroken,
    IoUnreadableFile,
    IoUnreadableDirectory,
    IoUnsupportedSourceExtension,
    IoArtifactMissing,
    IoArtifactUnreadable,
    IoArtifactMalformed,
    SchemaUnsupportedVersion,
    IdDuplicateInArtifact,
    RetrievalObjectNotFound,
}

impl DiagnosticCode {
    pub fn as_str(self) -> &'static str {
        match self {
            DiagnosticCode::ParseRawHtml => "parse.raw_html",
            DiagnosticCode::ParseUnsafeLink => "parse.unsafe_link",
            DiagnosticCode::ParseUnclosedFence => "parse.unclosed_fence",
            DiagnosticCode::ParseMalformedPageAnnotation => "parse.malformed_page_annotation",
            DiagnosticCode::ParseNestedTypedBlock => "parse.nested_typed_block",
            DiagnosticCode::ParseMalformedField => "parse.malformed_field",
            DiagnosticCode::ParseMalformedOpenFence => "parse.malformed_open_fence",
            DiagnosticCode::SchemaUnknownKind => "schema.unknown_kind",
            DiagnosticCode::SchemaMissingField => "schema.missing_field",
            DiagnosticCode::SchemaDuplicateField => "schema.duplicate_field",
            DiagnosticCode::SchemaInvalidStatus => "schema.invalid_status",
            DiagnosticCode::ClaimVerifiedMissingEvidence => "claim.verified_missing_evidence",
            DiagnosticCode::ClaimStatusCasing => "claim.status_casing",
            DiagnosticCode::IdDuplicate => "id.duplicate",
            DiagnosticCode::IdInvalid => "id.invalid",
            DiagnosticCode::RefBroken => "ref.broken",
            DiagnosticCode::IoUnreadableFile => "io.unreadable_file",
            DiagnosticCode::IoUnreadableDirectory => "io.unreadable_directory",
            DiagnosticCode::IoUnsupportedSourceExtension => "io.unsupported_source_extension",
            DiagnosticCode::IoArtifactMissing => "io.artifact_missing",
            DiagnosticCode::IoArtifactUnreadable => "io.artifact_unreadable",
            DiagnosticCode::IoArtifactMalformed => "io.artifact_malformed",
            DiagnosticCode::SchemaUnsupportedVersion => "schema.unsupported_version",
            DiagnosticCode::IdDuplicateInArtifact => "id.duplicate_in_artifact",
            DiagnosticCode::RetrievalObjectNotFound => "retrieval.object_not_found",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value {
            "parse.raw_html" => Some(DiagnosticCode::ParseRawHtml),
            "parse.unsafe_link" => Some(DiagnosticCode::ParseUnsafeLink),
            "parse.unclosed_fence" => Some(DiagnosticCode::ParseUnclosedFence),
            "parse.malformed_page_annotation" => Some(DiagnosticCode::ParseMalformedPageAnnotation),
            "parse.nested_typed_block" => Some(DiagnosticCode::ParseNestedTypedBlock),
            "parse.malformed_field" => Some(DiagnosticCode::ParseMalformedField),
            "parse.malformed_open_fence" => Some(DiagnosticCode::ParseMalformedOpenFence),
            "schema.unknown_kind" => Some(DiagnosticCode::SchemaUnknownKind),
            "schema.missing_field" => Some(DiagnosticCode::SchemaMissingField),
            "schema.duplicate_field" => Some(DiagnosticCode::SchemaDuplicateField),
            "schema.invalid_status" => Some(DiagnosticCode::SchemaInvalidStatus),
            "claim.verified_missing_evidence" => Some(DiagnosticCode::ClaimVerifiedMissingEvidence),
            "claim.status_casing" => Some(DiagnosticCode::ClaimStatusCasing),
            "id.duplicate" => Some(DiagnosticCode::IdDuplicate),
            "id.invalid" => Some(DiagnosticCode::IdInvalid),
            "ref.broken" => Some(DiagnosticCode::RefBroken),
            "io.unreadable_file" => Some(DiagnosticCode::IoUnreadableFile),
            "io.unreadable_directory" => Some(DiagnosticCode::IoUnreadableDirectory),
            "io.unsupported_source_extension" => Some(DiagnosticCode::IoUnsupportedSourceExtension),
            "io.artifact_missing" => Some(DiagnosticCode::IoArtifactMissing),
            "io.artifact_unreadable" => Some(DiagnosticCode::IoArtifactUnreadable),
            "io.artifact_malformed" => Some(DiagnosticCode::IoArtifactMalformed),
            "schema.unsupported_version" => Some(DiagnosticCode::SchemaUnsupportedVersion),
            "id.duplicate_in_artifact" => Some(DiagnosticCode::IdDuplicateInArtifact),
            "retrieval.object_not_found" => Some(DiagnosticCode::RetrievalObjectNotFound),
            _ => None,
        }
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for DiagnosticCode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for DiagnosticCode {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        DiagnosticCode::from_str(&value)
            .ok_or_else(|| de::Error::unknown_variant(&value, DIAGNOSTIC_CODE_VARIANTS))
    }
}

const DIAGNOSTIC_CODE_VARIANTS: &[&str] = &[
    "parse.raw_html",
    "parse.unsafe_link",
    "parse.unclosed_fence",
    "parse.malformed_page_annotation",
    "parse.nested_typed_block",
    "parse.malformed_field",
    "parse.malformed_open_fence",
    "schema.unknown_kind",
    "schema.missing_field",
    "schema.duplicate_field",
    "schema.invalid_status",
    "claim.verified_missing_evidence",
    "claim.status_casing",
    "id.duplicate",
    "id.invalid",
    "ref.broken",
    "io.unreadable_file",
    "io.unreadable_directory",
    "io.unsupported_source_extension",
    "io.artifact_missing",
    "io.artifact_unreadable",
    "io.artifact_malformed",
    "schema.unsupported_version",
    "id.duplicate_in_artifact",
    "retrieval.object_not_found",
];

impl Diagnostic {
    pub(crate) fn error(code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: Severity::Error,
            message: message.into(),
            span: None,
            object_id: None,
            help: None,
        }
    }

    pub(crate) fn warning(code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: Severity::Warning,
            message: message.into(),
            span: None,
            object_id: None,
            help: None,
        }
    }

    pub fn with_span(mut self, span: SourceSpan) -> Self {
        self.span = Some(span);
        self
    }

    pub(crate) fn with_object_id(mut self, object_id: impl Into<String>) -> Self {
        self.object_id = Some(object_id.into());
        self
    }

    pub(crate) fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl fmt::Display for Severity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => formatter.write_str("error"),
            Severity::Warning => formatter.write_str("warning"),
            Severity::Info => formatter.write_str("info"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SourceSpan {
    pub file: PathBuf,
    pub start: SourcePosition,
    pub end: SourcePosition,
}

impl SourceSpan {
    pub(crate) fn render_location(&self) -> String {
        format!(
            "{}:{}:{}",
            self.file.display(),
            self.start.line,
            self.start.column
        )
    }
}

impl Serialize for SourceSpan {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("SourceSpan", 3)?;
        state.serialize_field("file", &self.file.display().to_string())?;
        state.serialize_field("start", &self.start)?;
        state.serialize_field("end", &self.end)?;
        state.end()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourcePosition {
    pub line: u32,
    pub column: u32,
    pub offset: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span_with_file(file: PathBuf) -> SourceSpan {
        SourceSpan {
            file,
            start: SourcePosition {
                line: 1,
                column: 1,
                offset: 0,
            },
            end: SourcePosition {
                line: 1,
                column: 5,
                offset: 4,
            },
        }
    }

    #[test]
    fn source_span_serializes_file_as_display_string() {
        let value =
            serde_json::to_value(span_with_file(PathBuf::from("docs/sample.adoc"))).unwrap();

        assert_eq!(value["file"], "docs/sample.adoc");
        assert_eq!(value["start"]["line"], 1);
        assert_eq!(value["end"]["column"], 5);
    }

    #[test]
    fn source_span_renders_start_location() {
        let span = span_with_file(PathBuf::from("docs/sample.adoc"));

        assert_eq!(span.render_location(), "docs/sample.adoc:1:1");
    }

    #[test]
    fn with_help_sets_help_field() {
        let diagnostic =
            Diagnostic::error(DiagnosticCode::IdInvalid, "bad id").with_help("fix your id");
        assert_eq!(diagnostic.help.as_deref(), Some("fix your id"));
    }

    #[cfg(unix)]
    #[test]
    fn source_span_serializes_non_utf8_file_as_display_string() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let file = PathBuf::from(OsString::from_vec(vec![b'd', b'o', b'c', b's', b'/', 0xff]));
        let value = serde_json::to_value(span_with_file(file)).unwrap();

        assert!(
            value["file"].is_string(),
            "display-based serialization must not fail for non-UTF-8 paths"
        );
    }
}
