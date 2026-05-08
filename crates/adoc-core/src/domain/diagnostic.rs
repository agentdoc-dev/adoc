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
    SearchInvalidFilter,
    EmbedModelLoadFailed,
    EmbedComputeFailed,
    EmbedUnexpectedDimension,
    BuildEmbeddingsCached,
    BuildEmbeddingsCacheIgnored,
    BuildEmbeddingsSkipped,
}

impl DiagnosticCode {
    fn all() -> &'static [Self] {
        &[
            DiagnosticCode::ParseRawHtml,
            DiagnosticCode::ParseUnsafeLink,
            DiagnosticCode::ParseUnclosedFence,
            DiagnosticCode::ParseMalformedPageAnnotation,
            DiagnosticCode::ParseNestedTypedBlock,
            DiagnosticCode::ParseMalformedField,
            DiagnosticCode::ParseMalformedOpenFence,
            DiagnosticCode::SchemaUnknownKind,
            DiagnosticCode::SchemaMissingField,
            DiagnosticCode::SchemaDuplicateField,
            DiagnosticCode::SchemaInvalidStatus,
            DiagnosticCode::ClaimVerifiedMissingEvidence,
            DiagnosticCode::ClaimStatusCasing,
            DiagnosticCode::IdDuplicate,
            DiagnosticCode::IdInvalid,
            DiagnosticCode::RefBroken,
            DiagnosticCode::IoUnreadableFile,
            DiagnosticCode::IoUnreadableDirectory,
            DiagnosticCode::IoUnsupportedSourceExtension,
            DiagnosticCode::IoArtifactMissing,
            DiagnosticCode::IoArtifactUnreadable,
            DiagnosticCode::IoArtifactMalformed,
            DiagnosticCode::SchemaUnsupportedVersion,
            DiagnosticCode::IdDuplicateInArtifact,
            DiagnosticCode::RetrievalObjectNotFound,
            DiagnosticCode::SearchInvalidFilter,
            DiagnosticCode::EmbedModelLoadFailed,
            DiagnosticCode::EmbedComputeFailed,
            DiagnosticCode::EmbedUnexpectedDimension,
            DiagnosticCode::BuildEmbeddingsCached,
            DiagnosticCode::BuildEmbeddingsCacheIgnored,
            DiagnosticCode::BuildEmbeddingsSkipped,
        ]
    }

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
            DiagnosticCode::SearchInvalidFilter => "search.invalid_filter",
            DiagnosticCode::EmbedModelLoadFailed => "embed.model_load_failed",
            DiagnosticCode::EmbedComputeFailed => "embed.compute_failed",
            DiagnosticCode::EmbedUnexpectedDimension => "embed.unexpected_dim",
            DiagnosticCode::BuildEmbeddingsCached => "build.embeddings_cached",
            DiagnosticCode::BuildEmbeddingsCacheIgnored => "build.embeddings_cache_ignored",
            DiagnosticCode::BuildEmbeddingsSkipped => "build.embeddings_skipped",
        }
    }

    pub fn default_help(self) -> &'static str {
        match self {
            DiagnosticCode::ParseRawHtml => {
                "Remove raw HTML or replace it with supported Markdown/ADoc syntax."
            }
            DiagnosticCode::ParseUnsafeLink => {
                "Use a safe link scheme such as https, http, mailto, or a relative path."
            }
            DiagnosticCode::ParseUnclosedFence => {
                "Close the typed block with a matching fence before the end of the file."
            }
            DiagnosticCode::ParseMalformedPageAnnotation => {
                "Use a page annotation in the form `@doc(object.id)` with a valid Object ID."
            }
            DiagnosticCode::ParseNestedTypedBlock => {
                "Move nested typed blocks out of the current block body or field value."
            }
            DiagnosticCode::ParseMalformedField => {
                "Write typed block fields as `key: value` lines before the block body."
            }
            DiagnosticCode::ParseMalformedOpenFence => {
                "Open typed blocks with `::kind object.id`, using a supported kind and valid Object ID."
            }
            DiagnosticCode::SchemaUnknownKind => {
                "Use a supported object kind or update the schema before compiling."
            }
            DiagnosticCode::SchemaMissingField => "Add the required field with a non-empty value.",
            DiagnosticCode::SchemaDuplicateField => {
                "Keep only one value for each field inside the object."
            }
            DiagnosticCode::SchemaInvalidStatus => {
                "Use one of the allowed status values for this object kind."
            }
            DiagnosticCode::ClaimVerifiedMissingEvidence => {
                "Add evidence entries before marking the claim as verified."
            }
            DiagnosticCode::ClaimStatusCasing => "Use the canonical lowercase claim status value.",
            DiagnosticCode::IdDuplicate => {
                "Give each object a unique ID across the compiled workspace."
            }
            DiagnosticCode::IdInvalid => {
                "Use a valid Object ID with lowercase segments separated by dots."
            }
            DiagnosticCode::RefBroken => {
                "Update the reference to an existing object ID or remove the reference."
            }
            DiagnosticCode::IoUnreadableFile => {
                "Check that the source path exists and can be read by the current user."
            }
            DiagnosticCode::IoUnreadableDirectory => {
                "Check that the source directory exists and can be read by the current user."
            }
            DiagnosticCode::IoUnsupportedSourceExtension => {
                "Use supported source files with the `.adoc` extension."
            }
            DiagnosticCode::IoArtifactMissing => {
                "Build docs.agent.json before loading the retrieval artifact."
            }
            DiagnosticCode::IoArtifactUnreadable => {
                "Check that docs.agent.json exists and can be read by the current user."
            }
            DiagnosticCode::IoArtifactMalformed => {
                "Rebuild docs.agent.json from valid source documents."
            }
            DiagnosticCode::SchemaUnsupportedVersion => {
                "Regenerate the artifact with a schema version supported by this binary."
            }
            DiagnosticCode::IdDuplicateInArtifact => {
                "Rebuild the artifact after removing duplicate object IDs from the source."
            }
            DiagnosticCode::RetrievalObjectNotFound => {
                "Use an object ID present in the loaded retrieval artifact."
            }
            DiagnosticCode::SearchInvalidFilter => {
                "Change or remove the filter so it matches at least one object field in the artifact."
            }
            DiagnosticCode::EmbedModelLoadFailed => {
                "Check network access for the first model download, verify the local model cache is readable, ensure the binary was built with the `embeddings` feature, or rerun `adoc build --no-embeddings`."
            }
            DiagnosticCode::EmbedComputeFailed => {
                "Retry `adoc build`; if the error repeats, rebuild with `--no-embeddings` while investigating the embedding provider."
            }
            DiagnosticCode::EmbedUnexpectedDimension => {
                "Use an embedding provider that returns exactly one vector per input and the configured vector dimension."
            }
            DiagnosticCode::BuildEmbeddingsCached => {
                "No action is required; this reports search artifact embedding cache reuse."
            }
            DiagnosticCode::BuildEmbeddingsCacheIgnored => {
                "No action is required; the search cache will be recomputed for the current embedding model."
            }
            DiagnosticCode::BuildEmbeddingsSkipped => {
                "Run `adoc build` without `--no-embeddings` to emit docs.search.json."
            }
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        Self::all()
            .iter()
            .copied()
            .find(|code| code.as_str() == value)
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
    "search.invalid_filter",
    "embed.model_load_failed",
    "embed.compute_failed",
    "embed.unexpected_dim",
    "build.embeddings_cached",
    "build.embeddings_cache_ignored",
    "build.embeddings_skipped",
];

impl Diagnostic {
    /// Constructs the `explain.not_found` diagnostic emitted when the
    /// requested object id is absent from the loaded artifact.
    pub fn not_found(object_id: impl Into<String>) -> Self {
        let id = object_id.into();
        Self {
            code: DiagnosticCode::RetrievalObjectNotFound,
            severity: Severity::Error,
            message: format!("Object ID `{id}` was not found in the agent artifact."),
            span: None,
            object_id: Some(id),
            help: Some(
                "Run `adoc build` if the source was changed after the artifact was generated."
                    .to_string(),
            ),
        }
    }

    /// Constructs the `explain.resolver` diagnostic emitted when the record
    /// resolver encounters an infrastructure failure.
    ///
    /// The diagnostic code and message are derived from the specific
    /// [`crate::application::ports::record_resolver::ResolverError`] variant so
    /// that the inner string is used directly without duplicating any prefix
    /// that the error's `Display` impl already emits.
    pub fn resolver(err: &crate::application::ports::ResolverError) -> Self {
        use crate::application::ports::ResolverError;
        let (code, message) = match err {
            ResolverError::Io(inner) => (
                DiagnosticCode::IoArtifactUnreadable,
                format!("resolver error: {inner}"),
            ),
        };
        Self {
            code,
            severity: Severity::Error,
            message,
            span: None,
            object_id: None,
            help: None,
        }
    }

    pub(crate) fn error(code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: Severity::Error,
            message: message.into(),
            span: None,
            object_id: None,
            help: Some(code.default_help().to_string()),
        }
    }

    pub(crate) fn warning(code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: Severity::Warning,
            message: message.into(),
            span: None,
            object_id: None,
            help: Some(code.default_help().to_string()),
        }
    }

    pub(crate) fn info(code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: Severity::Info,
            message: message.into(),
            span: None,
            object_id: None,
            help: Some(code.default_help().to_string()),
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

    #[test]
    fn constructors_attach_default_help() {
        let error = Diagnostic::error(DiagnosticCode::ParseRawHtml, "raw html is not allowed");
        let warning = Diagnostic::warning(DiagnosticCode::ClaimStatusCasing, "status casing");

        assert_eq!(
            error.help.as_deref(),
            Some(DiagnosticCode::ParseRawHtml.default_help())
        );
        assert_eq!(
            warning.help.as_deref(),
            Some(DiagnosticCode::ClaimStatusCasing.default_help())
        );
    }

    #[test]
    fn default_help_is_complete_for_every_wire_code() {
        let codes = DiagnosticCode::all();
        let wire_codes: Vec<&str> = codes.iter().map(|code| code.as_str()).collect();

        assert_eq!(wire_codes, DIAGNOSTIC_CODE_VARIANTS);

        for code in codes {
            let help = code.default_help();
            assert!(
                !help.trim().is_empty(),
                "{} should have non-empty default help",
                code
            );
            assert_eq!(DiagnosticCode::from_str(code.as_str()), Some(*code));
        }
    }

    #[test]
    fn search_invalid_filter_code_roundtrips_through_wire_string() {
        let value = serde_json::to_value(DiagnosticCode::SearchInvalidFilter)
            .expect("diagnostic code serializes");

        assert_eq!(value, "search.invalid_filter");
        assert_eq!(
            serde_json::from_value::<DiagnosticCode>(value).expect("diagnostic code deserializes"),
            DiagnosticCode::SearchInvalidFilter
        );
        assert!(
            DiagnosticCode::SearchInvalidFilter
                .default_help()
                .contains("filter")
        );
    }

    #[test]
    fn resolver_diagnostic_uses_io_artifact_unreadable_and_does_not_double_print_prefix() {
        use crate::application::ports::ResolverError;
        let err = ResolverError::Io("disk gone".to_string());
        let diag = Diagnostic::resolver(&err);
        assert_eq!(diag.code, DiagnosticCode::IoArtifactUnreadable);
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "resolver error: disk gone");
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
