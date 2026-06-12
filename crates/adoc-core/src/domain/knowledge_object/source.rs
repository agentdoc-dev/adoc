//! `source` Knowledge Object aggregate (V5.7, ADR-0027).
//!
//! A reusable evidence pointer representing a single external artefact — a
//! source file, test, URL, commit, or any other [`EvidenceKind`] entry.
//! Required fields: `id`, `kind`, exactly one of `path` or `url`, `body`.
//! Optional fields pass through to `OptionalFields` (`owner`, `symbol`,
//! `commit`, `last_seen_at`, `hash`, …).

#[cfg(test)]
use std::collections::BTreeMap;

use crate::domain::ast::ParsedTypedBlock;
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, SourceSpan};
use crate::domain::identity::{OBJECT_ID_GRAMMAR_HELP, ObjectId, ObjectIdError};
use crate::domain::knowledge_object::Relations;
use crate::domain::value_objects::evidence_kind::{EvidenceKind, EvidenceKindError};
use crate::domain::value_objects::rel_path::{RelPath, RelPathError};
use crate::domain::value_objects::url::{Url, UrlError};
use crate::domain::values::{Body, OptionalFields};

const KIND_FIELD: &str = "kind";
const PATH_FIELD: &str = "path";
const URL_FIELD: &str = "url";

const SOURCE_MISSING_KIND_HELP: &str = "Add a `kind` field to the source: one of source_code, test, commit, pull_request, issue, \
     design_doc, human_review, external_url, api_schema, runtime_metric, incident, \
     support_ticket, audit_record, policy_reference, dataset, experiment.";
const SOURCE_INVALID_KIND_HELP: &str = "Use a valid evidence kind: one of source_code, test, commit, pull_request, issue, \
     design_doc, human_review, external_url, api_schema, runtime_metric, incident, \
     support_ticket, audit_record, policy_reference, dataset, experiment.";
const SOURCE_MISSING_PATH_OR_URL_HELP: &str =
    "Add either a `path` (repo-relative) or `url` (absolute URL) field to the source object.";
const SOURCE_CONFLICTING_PATH_AND_URL_HELP: &str =
    "Provide only one of `path` or `url` on a source object, not both.";
const SOURCE_INVALID_PATH_HELP: &str = "Use a repo-relative path (e.g. `src/main.rs`); avoid leading `/`, `..` segments, \
     backslashes, and Windows drive letters.";
const SOURCE_INVALID_URL_HELP: &str =
    "Use a well-formed absolute URL with an allowed scheme (http, https, or mailto).";
const SOURCE_KIND_TARGET_MISMATCH_HELP: &str = "The evidence kind restricts target to path-only or url-only. \
     Adjust the `kind`, `path`, or `url` field accordingly.";
const SOURCE_MISSING_BODY_HELP: &str =
    "Sources require non-empty body text describing what this evidence artefact contains.";

/// The evidence target carried by a `source` Knowledge Object: either a
/// repo-relative path or an absolute URL, but never both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SourceTarget {
    Path(RelPath),
    Url(Url),
}

/// A reusable evidence pointer (PRD §13.15, V5.7, ADR-0027).
///
/// Required fields: `id`, `kind`, one of `path` or `url`, `body`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Source {
    id: ObjectId,
    kind: EvidenceKind,
    target: SourceTarget,
    body: Body,
    fields: OptionalFields,
    relations: Relations,
    span: SourceSpan,
}

/// Why a `source` failed to build from parsed input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SourceError {
    InvalidId(ObjectIdError),
    MissingKind,
    InvalidKind(String),
    MissingBody,
    ConflictingPathAndUrl,
    MissingPathOrUrl,
    InvalidPath(RelPathError),
    InvalidUrl(UrlError),
    /// The kind's target requirement excludes the provided target type.
    /// Carries `(kind_str, disallowed_target_description)`.
    KindTargetMismatch(String, String),
}

impl Source {
    /// Build a `Source` from a parsed typed block, collecting all validation
    /// diagnostics. Returns `None` if any required field is absent or invalid.
    pub(crate) fn build_from_parsed(
        mut parsed: ParsedTypedBlock,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Self> {
        if super::reject_duplicate_fields(&parsed, "source", diagnostics) {
            return None;
        }

        // Parse id (needed for error messages throughout).
        let id = match ObjectId::new(&parsed.id_text) {
            Ok(id) => Some(id),
            Err(error) => {
                emit_error(&parsed, SourceError::InvalidId(error), diagnostics);
                None
            }
        };

        // Parse kind.
        let kind_raw = parsed.raw_fields.remove(KIND_FIELD);
        let kind = match EvidenceKind::try_new(kind_raw.as_deref().unwrap_or("")) {
            Ok(k) => Some(k),
            Err(EvidenceKindError::Missing) => {
                emit_error(&parsed, SourceError::MissingKind, diagnostics);
                None
            }
            Err(EvidenceKindError::Invalid(s)) => {
                emit_error(&parsed, SourceError::InvalidKind(s), diagnostics);
                None
            }
        };

        // Parse path (optional field — only an error if parsing fails, not if absent).
        let path_raw = parsed.raw_fields.remove(PATH_FIELD);
        let path_result: Option<Result<RelPath, RelPathError>> =
            path_raw.as_deref().map(RelPath::try_new);
        let path: Option<Option<RelPath>> = match path_result {
            None => None, // field absent
            Some(Ok(p)) => Some(Some(p)),
            Some(Err(e)) => {
                emit_error(&parsed, SourceError::InvalidPath(e), diagnostics);
                Some(None) // field present but invalid
            }
        };

        // Parse url (optional field — only an error if parsing fails, not if absent).
        let url_raw = parsed.raw_fields.remove(URL_FIELD);
        let url_result: Option<Result<Url, UrlError>> = url_raw.as_deref().map(Url::try_new);
        let url: Option<Option<Url>> = match url_result {
            None => None, // field absent
            Some(Ok(u)) => Some(Some(u)),
            Some(Err(e)) => {
                emit_error(&parsed, SourceError::InvalidUrl(e), diagnostics);
                Some(None) // field present but invalid
            }
        };

        // Resolve target: exactly one of path or url must be present and valid.
        let target: Option<SourceTarget> = match (&path, &url) {
            // Both present (regardless of validity) — conflict.
            (Some(_), Some(_)) => {
                emit_error(&parsed, SourceError::ConflictingPathAndUrl, diagnostics);
                None
            }
            // Neither present.
            (None, None) => {
                emit_error(&parsed, SourceError::MissingPathOrUrl, diagnostics);
                None
            }
            // Only path present and valid.
            (Some(Some(p)), None) => {
                // If we already have a kind, validate kind/target compatibility.
                if let Some(k) = kind {
                    if !k.allows_path() {
                        emit_error(
                            &parsed,
                            SourceError::KindTargetMismatch(
                                k.as_str().to_string(),
                                "path".to_string(),
                            ),
                            diagnostics,
                        );
                        None
                    } else {
                        Some(SourceTarget::Path(p.clone()))
                    }
                } else {
                    // kind failed; target is structurally present but we can't
                    // validate compatibility — produce None so the outer gate
                    // fires.
                    Some(SourceTarget::Path(p.clone()))
                }
            }
            // Only path present but invalid — already reported above.
            (Some(None), None) => None,
            // Only url present and valid.
            (None, Some(Some(u))) => {
                if let Some(k) = kind {
                    if !k.allows_url() {
                        emit_error(
                            &parsed,
                            SourceError::KindTargetMismatch(
                                k.as_str().to_string(),
                                "url".to_string(),
                            ),
                            diagnostics,
                        );
                        None
                    } else {
                        Some(SourceTarget::Url(u.clone()))
                    }
                } else {
                    Some(SourceTarget::Url(u.clone()))
                }
            }
            // Only url present but invalid — already reported above.
            (None, Some(None)) => None,
        };

        // Parse body.
        let body = match super::body_from_parsed(&parsed) {
            Some(b) => Some(b),
            None => {
                emit_error(&parsed, SourceError::MissingBody, diagnostics);
                None
            }
        };

        // All required fields must be present to produce a valid aggregate.
        if id.is_none() || kind.is_none() || target.is_none() || body.is_none() {
            return None;
        }

        let relations = super::extract_relations(&mut parsed, diagnostics);
        let optional_fields = std::mem::take(&mut parsed.raw_fields);

        Some(Self {
            id: id.expect("checked above"),
            kind: kind.expect("checked above"),
            target: target.expect("checked above"),
            body: body.expect("checked above"),
            fields: OptionalFields::from_map(optional_fields),
            relations,
            span: parsed.span.clone(),
        })
    }

    // ── Accessors ────────────────────────────────────────────────────────────

    pub(crate) fn id(&self) -> &ObjectId {
        &self.id
    }

    pub(crate) fn kind(&self) -> EvidenceKind {
        self.kind
    }

    pub(crate) fn body(&self) -> &Body {
        &self.body
    }

    pub(crate) fn body_mut(&mut self) -> &mut Body {
        &mut self.body
    }

    pub(crate) fn fields(&self) -> &OptionalFields {
        &self.fields
    }

    pub(crate) fn relations(&self) -> &Relations {
        &self.relations
    }

    pub(crate) fn span(&self) -> &SourceSpan {
        &self.span
    }

    /// Returns the repo-relative path if the target is a path.
    pub(crate) fn path(&self) -> Option<&RelPath> {
        match &self.target {
            SourceTarget::Path(p) => Some(p),
            SourceTarget::Url(_) => None,
        }
    }

    /// Returns the absolute URL if the target is a URL.
    pub(crate) fn url(&self) -> Option<&Url> {
        match &self.target {
            SourceTarget::Path(_) => None,
            SourceTarget::Url(u) => Some(u),
        }
    }

    /// Test-only constructor that bypasses the parsed-block pipeline.
    #[cfg(test)]
    pub(crate) fn try_new(
        id_text: &str,
        kind_text: &str,
        path_text: Option<&str>,
        url_text: Option<&str>,
        body_text: &str,
        optional_fields: BTreeMap<String, String>,
        span: SourceSpan,
    ) -> Result<Self, SourceError> {
        let id = ObjectId::new(id_text).map_err(SourceError::InvalidId)?;
        let kind = EvidenceKind::try_new(kind_text).map_err(|e| match e {
            EvidenceKindError::Missing => SourceError::MissingKind,
            EvidenceKindError::Invalid(s) => SourceError::InvalidKind(s),
        })?;
        let target = match (path_text, url_text) {
            (Some(_), Some(_)) => return Err(SourceError::ConflictingPathAndUrl),
            (None, None) => return Err(SourceError::MissingPathOrUrl),
            (Some(p), None) => {
                if !kind.allows_path() {
                    return Err(SourceError::KindTargetMismatch(
                        kind.as_str().to_string(),
                        "path".to_string(),
                    ));
                }
                SourceTarget::Path(RelPath::try_new(p).map_err(SourceError::InvalidPath)?)
            }
            (None, Some(u)) => {
                if !kind.allows_url() {
                    return Err(SourceError::KindTargetMismatch(
                        kind.as_str().to_string(),
                        "url".to_string(),
                    ));
                }
                SourceTarget::Url(Url::try_new(u).map_err(SourceError::InvalidUrl)?)
            }
        };
        let body = Body::from_plain_text(body_text).ok_or(SourceError::MissingBody)?;
        Ok(Self {
            id,
            kind,
            target,
            body,
            fields: OptionalFields::from_map(optional_fields),
            relations: Relations::empty(),
            span,
        })
    }
}

fn emit_error(parsed: &ParsedTypedBlock, error: SourceError, diagnostics: &mut Vec<Diagnostic>) {
    match error {
        SourceError::InvalidId(error) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::IdInvalid,
                format!("invalid source id `{}`: {error}", parsed.id_text),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(OBJECT_ID_GRAMMAR_HELP),
        ),
        SourceError::MissingKind => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaSourceMissingKind,
                format!(
                    "source `{}` is missing required field `kind`",
                    parsed.id_text
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(SOURCE_MISSING_KIND_HELP),
        ),
        SourceError::InvalidKind(kind) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaSourceInvalidKind,
                format!("source `{}` has invalid kind `{kind}`", parsed.id_text),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(SOURCE_INVALID_KIND_HELP),
        ),
        SourceError::MissingBody => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaMissingField,
                format!("source `{}` is missing required body", parsed.id_text),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(SOURCE_MISSING_BODY_HELP),
        ),
        SourceError::ConflictingPathAndUrl => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaSourceConflictingPathAndUrl,
                format!(
                    "source `{}` has both `path` and `url`; provide only one",
                    parsed.id_text
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(SOURCE_CONFLICTING_PATH_AND_URL_HELP),
        ),
        SourceError::MissingPathOrUrl => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaSourceMissingPathOrUrl,
                format!(
                    "source `{}` is missing required field: one of `path` or `url`",
                    parsed.id_text
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(SOURCE_MISSING_PATH_OR_URL_HELP),
        ),
        SourceError::InvalidPath(error) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaSourceInvalidPath,
                format!("source `{}` has invalid `path`: {error}", parsed.id_text),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(SOURCE_INVALID_PATH_HELP),
        ),
        SourceError::InvalidUrl(error) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaSourceInvalidUrl,
                format!("source `{}` has invalid `url`: {error}", parsed.id_text),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(SOURCE_INVALID_URL_HELP),
        ),
        SourceError::KindTargetMismatch(kind, target) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaSourceKindTargetMismatch,
                format!(
                    "source `{}` kind `{kind}` does not allow target `{target}`",
                    parsed.id_text
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(SOURCE_KIND_TARGET_MISMATCH_HELP),
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use super::*;
    use crate::domain::diagnostic::{DiagnosticCode, SourcePosition, SourceSpan};

    fn span() -> SourceSpan {
        SourceSpan {
            file: PathBuf::from("test.adoc"),
            start: SourcePosition {
                line: 1,
                column: 1,
                offset: 0,
            },
            end: SourcePosition {
                line: 1,
                column: 8,
                offset: 7,
            },
        }
    }

    fn parsed_source(fields: BTreeMap<String, String>, body_text: &str) -> ParsedTypedBlock {
        ParsedTypedBlock {
            kind_word: "source".to_string(),
            kind_word_span: span(),
            id_text: "billing.consume-use-case".to_string(),
            raw_fields: fields,
            raw_field_spans: BTreeMap::new(),
            duplicate_keys: Vec::new(),
            body_text: body_text.to_string(),
            body_inlines: ParsedTypedBlock::test_body_inlines_from_text(body_text),
            body_spans: Vec::new(),
            content_spans: Vec::new(),
            span: span(),
            close_fence_span: span(),
            body_separator_span: None,
        }
    }

    fn path_fields() -> BTreeMap<String, String> {
        BTreeMap::from([
            (KIND_FIELD.to_string(), "source_code".to_string()),
            (
                PATH_FIELD.to_string(),
                "src/features/credits/consume.ts".to_string(),
            ),
        ])
    }

    fn url_fields() -> BTreeMap<String, String> {
        BTreeMap::from([
            (KIND_FIELD.to_string(), "external_url".to_string()),
            (
                URL_FIELD.to_string(),
                "https://example.com/credits".to_string(),
            ),
        ])
    }

    const BODY: &str = "Source implementation for credit consumption.";

    // ── try_new tests ─────────────────────────────────────────────────────

    #[test]
    fn try_new_accepts_source_code_with_path() {
        let s = Source::try_new(
            "billing.consume-use-case",
            "source_code",
            Some("src/features/credits/consume.ts"),
            None,
            BODY,
            BTreeMap::new(),
            span(),
        )
        .expect("valid source_code + path");

        assert_eq!(s.id().as_str(), "billing.consume-use-case");
        assert_eq!(s.kind().as_str(), "source_code");
        assert_eq!(
            s.path().map(RelPath::as_str),
            Some("src/features/credits/consume.ts")
        );
        assert!(s.url().is_none());
        assert_eq!(s.body().to_source(), BODY);
    }

    #[test]
    fn try_new_accepts_external_url_with_url() {
        let s = Source::try_new(
            "billing.external-ref",
            "external_url",
            None,
            Some("https://example.com/credits"),
            BODY,
            BTreeMap::new(),
            span(),
        )
        .expect("valid external_url + url");

        assert_eq!(s.kind().as_str(), "external_url");
        assert!(s.path().is_none());
        assert_eq!(
            s.url().map(Url::as_str),
            Some("https://example.com/credits")
        );
    }

    #[test]
    fn try_new_rejects_both_path_and_url() {
        let err = Source::try_new(
            "billing.conflict",
            "commit",
            Some("src/main.rs"),
            Some("https://example.com"),
            BODY,
            BTreeMap::new(),
            span(),
        )
        .expect_err("both path and url must be rejected");
        assert_eq!(err, SourceError::ConflictingPathAndUrl);
    }

    #[test]
    fn try_new_rejects_neither_path_nor_url() {
        let err = Source::try_new(
            "billing.missing-target",
            "source_code",
            None,
            None,
            BODY,
            BTreeMap::new(),
            span(),
        )
        .expect_err("no target must be rejected");
        assert_eq!(err, SourceError::MissingPathOrUrl);
    }

    #[test]
    fn try_new_rejects_external_url_kind_with_path() {
        let err = Source::try_new(
            "billing.mismatch",
            "external_url",
            Some("src/main.rs"),
            None,
            BODY,
            BTreeMap::new(),
            span(),
        )
        .expect_err("external_url kind does not allow path");
        assert_eq!(
            err,
            SourceError::KindTargetMismatch("external_url".to_string(), "path".to_string())
        );
    }

    #[test]
    fn try_new_rejects_source_code_kind_with_url() {
        let err = Source::try_new(
            "billing.mismatch",
            "source_code",
            None,
            Some("https://example.com"),
            BODY,
            BTreeMap::new(),
            span(),
        )
        .expect_err("source_code kind does not allow url");
        assert_eq!(
            err,
            SourceError::KindTargetMismatch("source_code".to_string(), "url".to_string())
        );
    }

    #[test]
    fn try_new_rejects_invalid_kind() {
        let err = Source::try_new(
            "billing.bad-kind",
            "bogus_kind",
            Some("src/main.rs"),
            None,
            BODY,
            BTreeMap::new(),
            span(),
        )
        .expect_err("invalid kind");
        assert_eq!(err, SourceError::InvalidKind("bogus_kind".to_string()));
    }

    #[test]
    fn try_new_rejects_missing_body() {
        let err = Source::try_new(
            "billing.no-body",
            "source_code",
            Some("src/main.rs"),
            None,
            "   ",
            BTreeMap::new(),
            span(),
        )
        .expect_err("empty body");
        assert_eq!(err, SourceError::MissingBody);
    }

    // ── build_from_parsed — valid ─────────────────────────────────────────

    #[test]
    fn build_from_parsed_accepts_source_code_with_path() {
        let parsed = parsed_source(path_fields(), BODY);
        let mut diagnostics = Vec::new();

        let s = Source::build_from_parsed(parsed, &mut diagnostics).expect("valid source");

        assert!(diagnostics.is_empty(), "diagnostics: {diagnostics:?}");
        assert_eq!(s.id().as_str(), "billing.consume-use-case");
        assert_eq!(s.kind().as_str(), "source_code");
        assert_eq!(
            s.path().map(RelPath::as_str),
            Some("src/features/credits/consume.ts")
        );
        assert!(s.url().is_none());
        assert_eq!(s.body().to_source(), BODY);
    }

    #[test]
    fn build_from_parsed_accepts_external_url_with_url() {
        let parsed = parsed_source(url_fields(), BODY);
        let mut diagnostics = Vec::new();

        let s = Source::build_from_parsed(parsed, &mut diagnostics).expect("valid source");

        assert!(diagnostics.is_empty(), "diagnostics: {diagnostics:?}");
        assert_eq!(s.kind().as_str(), "external_url");
        assert!(s.path().is_none());
        assert_eq!(
            s.url().map(Url::as_str),
            Some("https://example.com/credits")
        );
    }

    // ── build_from_parsed — validation errors ─────────────────────────────

    #[test]
    fn build_from_parsed_reports_missing_kind() {
        let mut fields = path_fields();
        fields.remove(KIND_FIELD);
        let parsed = parsed_source(fields, BODY);
        let mut diagnostics = Vec::new();

        let result = Source::build_from_parsed(parsed, &mut diagnostics);

        assert!(result.is_none());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::SchemaSourceMissingKind),
            "expected MissingKind, got: {diagnostics:?}"
        );
    }

    #[test]
    fn build_from_parsed_reports_invalid_kind() {
        let mut fields = path_fields();
        fields.insert(KIND_FIELD.to_string(), "bogus".to_string());
        let parsed = parsed_source(fields, BODY);
        let mut diagnostics = Vec::new();

        let result = Source::build_from_parsed(parsed, &mut diagnostics);

        assert!(result.is_none());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::SchemaSourceInvalidKind),
            "expected InvalidKind, got: {diagnostics:?}"
        );
    }

    #[test]
    fn build_from_parsed_reports_conflicting_path_and_url() {
        let mut fields = path_fields();
        fields.insert(URL_FIELD.to_string(), "https://example.com".to_string());
        let parsed = parsed_source(fields, BODY);
        let mut diagnostics = Vec::new();

        let result = Source::build_from_parsed(parsed, &mut diagnostics);

        assert!(result.is_none());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::SchemaSourceConflictingPathAndUrl),
            "expected ConflictingPathAndUrl, got: {diagnostics:?}"
        );
    }

    #[test]
    fn build_from_parsed_reports_missing_path_or_url() {
        let mut fields = path_fields();
        fields.remove(PATH_FIELD);
        let parsed = parsed_source(fields, BODY);
        let mut diagnostics = Vec::new();

        let result = Source::build_from_parsed(parsed, &mut diagnostics);

        assert!(result.is_none());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::SchemaSourceMissingPathOrUrl),
            "expected MissingPathOrUrl, got: {diagnostics:?}"
        );
    }

    #[test]
    fn build_from_parsed_reports_kind_target_mismatch_url_kind_with_path() {
        let fields = BTreeMap::from([
            (KIND_FIELD.to_string(), "external_url".to_string()),
            (PATH_FIELD.to_string(), "src/main.rs".to_string()),
        ]);
        let parsed = parsed_source(fields, BODY);
        let mut diagnostics = Vec::new();

        let result = Source::build_from_parsed(parsed, &mut diagnostics);

        assert!(result.is_none());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::SchemaSourceKindTargetMismatch),
            "expected KindTargetMismatch, got: {diagnostics:?}"
        );
    }

    #[test]
    fn build_from_parsed_reports_kind_target_mismatch_path_kind_with_url() {
        let fields = BTreeMap::from([
            (KIND_FIELD.to_string(), "source_code".to_string()),
            (URL_FIELD.to_string(), "https://example.com".to_string()),
        ]);
        let parsed = parsed_source(fields, BODY);
        let mut diagnostics = Vec::new();

        let result = Source::build_from_parsed(parsed, &mut diagnostics);

        assert!(result.is_none());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::SchemaSourceKindTargetMismatch),
            "expected KindTargetMismatch, got: {diagnostics:?}"
        );
    }

    #[test]
    fn build_from_parsed_reports_invalid_id() {
        let mut parsed = parsed_source(path_fields(), BODY);
        parsed.id_text = "Invalid.ID".to_string();
        let mut diagnostics = Vec::new();

        let result = Source::build_from_parsed(parsed, &mut diagnostics);

        assert!(result.is_none());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::IdInvalid),
            "expected IdInvalid, got: {diagnostics:?}"
        );
    }

    #[test]
    fn build_from_parsed_reports_missing_body() {
        let parsed = parsed_source(path_fields(), "   ");
        let mut diagnostics = Vec::new();

        let result = Source::build_from_parsed(parsed, &mut diagnostics);

        assert!(result.is_none());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::SchemaMissingField),
            "expected SchemaMissingField for missing body, got: {diagnostics:?}"
        );
    }

    #[test]
    fn build_from_parsed_preserves_optional_fields() {
        let mut fields = path_fields();
        fields.insert("owner".to_string(), "backend-platform".to_string());
        fields.insert("symbol".to_string(), "consumeUseCase".to_string());
        fields.insert("commit".to_string(), "abc1234".to_string());
        fields.insert("last_seen_at".to_string(), "2026-05-01".to_string());
        fields.insert("hash".to_string(), "sha256:deadbeef".to_string());
        let parsed = parsed_source(fields, BODY);
        let mut diagnostics = Vec::new();

        let s = Source::build_from_parsed(parsed, &mut diagnostics).expect("valid source");

        assert!(diagnostics.is_empty(), "diagnostics: {diagnostics:?}");
        let keys: Vec<&str> = s.fields().iter().map(|(k, _)| k.as_str()).collect();
        // kind and path are stripped; owner/symbol/commit/last_seen_at/hash pass through
        assert!(
            !keys.contains(&KIND_FIELD),
            "kind must not be in optional fields"
        );
        assert!(
            !keys.contains(&PATH_FIELD),
            "path must not be in optional fields"
        );
        assert!(keys.contains(&"owner"));
        assert!(keys.contains(&"symbol"));
        assert!(keys.contains(&"commit"));
        assert!(keys.contains(&"last_seen_at"));
        assert!(keys.contains(&"hash"));
    }
}
