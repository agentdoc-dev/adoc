use std::collections::BTreeMap;

use crate::domain::ast::ParsedTypedBlock;
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, SourceSpan};
use crate::domain::identity::{OBJECT_ID_GRAMMAR_HELP, ObjectId, ObjectIdError};
use crate::domain::knowledge_object::Relations;
use crate::domain::value_objects::lang::{Lang, LangError};
use crate::domain::value_objects::lifecycle_status::{LifecycleStatus, LifecycleStatusError};
use crate::domain::value_objects::rel_path::RelPath;
use crate::domain::value_objects::sandbox::{SandboxName, SandboxNameError};
use crate::domain::values::{Body, NonEmpty, OptionalFields};

const STATUS_FIELD: &str = "status";
pub(crate) const LANG_FIELD: &str = "lang";
pub(crate) const FORMAT_FIELD: &str = "format";
pub(crate) const CHECKS_FIELD: &str = "checks";
pub(crate) const SANDBOX_FIELD: &str = "sandbox";

const EXAMPLE_MISSING_LANG_HELP: &str = "An example requires either `lang` or `format`.";
const EXAMPLE_INVALID_LANG_HELP: &str =
    "Valid `lang` is a lowercase token matching [a-z][a-z0-9_+-]* (e.g. `ts`, `python`, `c++`).";
const EXAMPLE_INVALID_SANDBOX_HELP: &str = "Valid `sandbox` is a lowercase token matching [a-z][a-z0-9_+:-]* (e.g. `node-test`, `docker:node-test`).";
const EXAMPLE_VERIFIED_REQUIRES_CHECKS_HELP: &str =
    "A verified example requires both `checks` and `sandbox`.";
const EXAMPLE_VERIFIED_REQUIRES_SANDBOX_HELP: &str =
    "A verified example requires both `checks` and `sandbox`.";
const EXAMPLE_MISSING_BODY_HELP: &str = "Examples require non-empty body text.";
const EXAMPLE_INVALID_STATUS_HELP: &str =
    "Valid example statuses are: draft, verified, deprecated.";

/// A code/API/usage example (PRD §13.5). Required fields: `id`, body, and at
/// least one of `lang` or `format`. Optional `status` (draft, verified,
/// deprecated). A `verified` example additionally requires `checks` and
/// `sandbox` — "verified" here means *executable-declared*, not ownership-
/// reviewed, so there is no `Verification` value object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Example {
    id: ObjectId,
    status: Option<LifecycleStatus>,
    lang: Option<Lang>,
    format: Option<String>,
    body: Body,
    checks: Option<String>,
    sandbox: Option<SandboxName>,
    fields: OptionalFields,
    relations: Relations,
    impacts: Option<NonEmpty<RelPath>>,
    span: SourceSpan,
}

/// Maps the shared [`LifecycleStatus`] parse errors into example's own error
/// vocabulary: a blank status keeps its historical `InvalidStatus("")` shape.
fn status_from_text(value: &str) -> Result<LifecycleStatus, ExampleError> {
    LifecycleStatus::try_new(value).map_err(|error| match error {
        LifecycleStatusError::Missing => ExampleError::InvalidStatus(String::new()),
        LifecycleStatusError::Invalid(value) => ExampleError::InvalidStatus(value),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExampleError {
    InvalidId(ObjectIdError),
    InvalidStatus(String),
    MissingLang,
    InvalidLang(String),
    InvalidSandbox(String),
    MissingBody,
    VerifiedRequiresChecks,
    VerifiedRequiresSandbox,
}

impl Example {
    pub(crate) fn build_from_parsed(
        mut parsed: ParsedTypedBlock,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Self> {
        if super::reject_duplicate_fields(&parsed, "example", diagnostics) {
            return None;
        }

        // 1. id
        let id = match ObjectId::new(&parsed.id_text) {
            Ok(id) => id,
            Err(error) => {
                emit_example_error(&parsed, ExampleError::InvalidId(error), diagnostics);
                return None;
            }
        };

        // 2. status
        let status = match super::take_optional_scalar(&mut parsed, STATUS_FIELD, status_from_text)
        {
            Ok(status) => status,
            Err(error) => {
                emit_example_error(&parsed, error, diagnostics);
                return None;
            }
        };

        // 3. lang
        let lang = match super::take_optional_scalar(&mut parsed, LANG_FIELD, Lang::try_new) {
            Ok(lang) => lang,
            Err(LangError::Missing) => None,
            Err(LangError::Invalid(val)) => {
                emit_example_error(&parsed, ExampleError::InvalidLang(val), diagnostics);
                return None;
            }
        };

        // 4. format
        let format = super::take_scalar_text(&mut parsed, FORMAT_FIELD);

        // 5. sandbox
        let sandbox =
            match super::take_optional_scalar(&mut parsed, SANDBOX_FIELD, SandboxName::try_new) {
                Ok(sandbox) => sandbox,
                Err(SandboxNameError::Missing) => None,
                Err(SandboxNameError::Invalid(val)) => {
                    emit_example_error(&parsed, ExampleError::InvalidSandbox(val), diagnostics);
                    return None;
                }
            };

        // 6. checks (preserve verbatim after edge-trim)
        let checks = super::take_scalar_text(&mut parsed, CHECKS_FIELD);

        // 7. body
        let body = match super::body_from_parsed(&parsed) {
            Some(b) => b,
            None => {
                emit_example_error(&parsed, ExampleError::MissingBody, diagnostics);
                return None;
            }
        };

        // 8. requirement diagnostics — emit ALL applicable, then return None if any
        let before = diagnostics.len();
        if lang.is_none() && format.is_none() {
            emit_example_error(&parsed, ExampleError::MissingLang, diagnostics);
        }
        if status == Some(LifecycleStatus::Verified) {
            if checks.is_none() {
                emit_example_error(&parsed, ExampleError::VerifiedRequiresChecks, diagnostics);
            }
            if sandbox.is_none() {
                emit_example_error(&parsed, ExampleError::VerifiedRequiresSandbox, diagnostics);
            }
        }
        if diagnostics.len() > before {
            return None;
        }

        // 9. relations, impacts, remaining fields
        let relations = super::extract_relations(&mut parsed, diagnostics);
        let impacts = super::extract_impacts(&mut parsed, diagnostics);
        let optional_fields = std::mem::take(&mut parsed.raw_fields);

        // 10. build
        match Self::from_parts(
            id,
            status,
            lang,
            format,
            body,
            checks,
            sandbox,
            optional_fields,
            relations,
            parsed.span.clone(),
        ) {
            Ok(example) => Some(example.with_impacts(impacts)),
            Err(error) => {
                emit_example_error(&parsed, error, diagnostics);
                None
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn from_parts(
        id: ObjectId,
        status: Option<LifecycleStatus>,
        lang: Option<Lang>,
        format: Option<String>,
        body: Body,
        checks: Option<String>,
        sandbox: Option<SandboxName>,
        optional_fields: BTreeMap<String, String>,
        relations: Relations,
        span: SourceSpan,
    ) -> Result<Self, ExampleError> {
        if lang.is_none() && format.is_none() {
            return Err(ExampleError::MissingLang);
        }
        if status == Some(LifecycleStatus::Verified) {
            if checks.is_none() {
                return Err(ExampleError::VerifiedRequiresChecks);
            }
            if sandbox.is_none() {
                return Err(ExampleError::VerifiedRequiresSandbox);
            }
        }
        debug_assert!(
            !optional_fields.contains_key(STATUS_FIELD),
            "optional example fields must not contain stripped field `status`"
        );
        debug_assert!(
            !optional_fields.contains_key(LANG_FIELD),
            "optional example fields must not contain stripped field `lang`"
        );
        debug_assert!(
            !optional_fields.contains_key(FORMAT_FIELD),
            "optional example fields must not contain stripped field `format`"
        );
        debug_assert!(
            !optional_fields.contains_key(CHECKS_FIELD),
            "optional example fields must not contain stripped field `checks`"
        );
        debug_assert!(
            !optional_fields.contains_key(SANDBOX_FIELD),
            "optional example fields must not contain stripped field `sandbox`"
        );
        Ok(Self {
            id,
            status,
            lang,
            format,
            body,
            checks,
            sandbox,
            fields: OptionalFields::from_map(optional_fields),
            relations,
            impacts: None,
            span,
        })
    }

    /// Attach the (already validated) opt-in `impacts:` list. Returns `self`
    /// for fluent composition by the build pipeline, mirroring `Constraint`.
    pub(crate) fn with_impacts(mut self, impacts: Option<NonEmpty<RelPath>>) -> Self {
        self.impacts = impacts;
        self
    }

    pub(crate) fn id(&self) -> &ObjectId {
        &self.id
    }

    pub(crate) fn status(&self) -> Option<&LifecycleStatus> {
        self.status.as_ref()
    }

    pub(crate) fn lang(&self) -> Option<&Lang> {
        self.lang.as_ref()
    }

    pub(crate) fn format(&self) -> Option<&str> {
        self.format.as_deref()
    }

    pub(crate) fn body(&self) -> &Body {
        &self.body
    }

    pub(crate) fn body_mut(&mut self) -> &mut Body {
        &mut self.body
    }

    pub(crate) fn checks(&self) -> Option<&str> {
        self.checks.as_deref()
    }

    pub(crate) fn sandbox(&self) -> Option<&SandboxName> {
        self.sandbox.as_ref()
    }

    pub(crate) fn fields(&self) -> &OptionalFields {
        &self.fields
    }

    pub(crate) fn relations(&self) -> &Relations {
        &self.relations
    }

    pub(crate) fn impacts(&self) -> Option<&[RelPath]> {
        self.impacts.as_ref().map(NonEmpty::as_slice)
    }

    pub(crate) fn span(&self) -> &SourceSpan {
        &self.span
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn try_new(
        id_text: &str,
        status_text: Option<&str>,
        lang_text: Option<&str>,
        format_text: Option<&str>,
        body_text: &str,
        checks: Option<&str>,
        sandbox_text: Option<&str>,
        optional_fields: BTreeMap<String, String>,
        span: SourceSpan,
    ) -> Result<Self, ExampleError> {
        let id = ObjectId::new(id_text).map_err(ExampleError::InvalidId)?;
        let status = status_text.map(status_from_text).transpose()?;
        let lang = lang_text
            .map(|v| {
                Lang::try_new(v).map_err(|e| match e {
                    LangError::Missing => ExampleError::MissingLang,
                    LangError::Invalid(val) => ExampleError::InvalidLang(val),
                })
            })
            .transpose()?;
        let format = format_text.and_then(|v| {
            let trimmed = crate::domain::values::trim_ascii_edges(v);
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });
        let sandbox = sandbox_text
            .map(|v| {
                SandboxName::try_new(v).map_err(|e| match e {
                    SandboxNameError::Missing => ExampleError::InvalidSandbox(String::new()),
                    SandboxNameError::Invalid(val) => ExampleError::InvalidSandbox(val),
                })
            })
            .transpose()?;
        let checks = checks.and_then(|v| {
            let trimmed = crate::domain::values::trim_ascii_edges(v);
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });
        let body = Body::from_plain_text(body_text).ok_or(ExampleError::MissingBody)?;
        Self::from_parts(
            id,
            status,
            lang,
            format,
            body,
            checks,
            sandbox,
            optional_fields,
            Relations::empty(),
            span,
        )
    }
}

fn emit_example_error(
    parsed: &ParsedTypedBlock,
    error: ExampleError,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match error {
        ExampleError::InvalidId(error) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::IdInvalid,
                format!("invalid example id `{}`: {error}", parsed.id_text),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(OBJECT_ID_GRAMMAR_HELP),
        ),
        ExampleError::InvalidStatus(status) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaInvalidStatus,
                format!("example `{}` has invalid status `{status}`", parsed.id_text),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(EXAMPLE_INVALID_STATUS_HELP),
        ),
        ExampleError::MissingLang => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaExampleMissingLang,
                format!(
                    "example `{}` requires either `lang` or `format`",
                    parsed.id_text
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(EXAMPLE_MISSING_LANG_HELP),
        ),
        ExampleError::InvalidLang(lang) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaExampleInvalidLang,
                format!("example `{}` has invalid lang `{lang}`", parsed.id_text),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(EXAMPLE_INVALID_LANG_HELP),
        ),
        ExampleError::InvalidSandbox(sandbox) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaExampleInvalidSandbox,
                format!(
                    "example `{}` has invalid sandbox `{sandbox}`",
                    parsed.id_text
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(EXAMPLE_INVALID_SANDBOX_HELP),
        ),
        ExampleError::MissingBody => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaMissingField,
                "example is missing required body",
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(EXAMPLE_MISSING_BODY_HELP),
        ),
        ExampleError::VerifiedRequiresChecks => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaExampleVerifiedRequiresChecks,
                format!(
                    "verified example `{}` requires both `checks` and `sandbox`",
                    parsed.id_text
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(EXAMPLE_VERIFIED_REQUIRES_CHECKS_HELP),
        ),
        ExampleError::VerifiedRequiresSandbox => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaExampleVerifiedRequiresSandbox,
                format!(
                    "verified example `{}` requires both `checks` and `sandbox`",
                    parsed.id_text
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(EXAMPLE_VERIFIED_REQUIRES_SANDBOX_HELP),
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::domain::diagnostic::{SourcePosition, SourceSpan};

    const BODY: &str = "const x = 1 + 1;";
    const ID: &str = "auth.credits.example";

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

    fn parsed_example(fields: BTreeMap<String, String>, body_text: &str) -> ParsedTypedBlock {
        ParsedTypedBlock {
            kind_word: "example".to_string(),
            kind_word_span: span(),
            id_text: ID.to_string(),
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

    // --- try_new / from_parts acceptance tests ---

    #[test]
    fn accepts_lang_only_example_with_no_status() {
        let example = Example::try_new(
            ID,
            None,
            Some("ts"),
            None,
            BODY,
            None,
            None,
            BTreeMap::new(),
            span(),
        )
        .expect("valid lang-only example");

        assert_eq!(example.id().as_str(), ID);
        assert!(example.status().is_none());
        assert_eq!(example.lang().map(Lang::as_str), Some("ts"));
        assert!(example.format().is_none());
        assert!(example.checks().is_none());
        assert!(example.sandbox().is_none());
    }

    #[test]
    fn accepts_format_only_example() {
        let example = Example::try_new(
            ID,
            None,
            None,
            Some("openapi-yaml"),
            BODY,
            None,
            None,
            BTreeMap::new(),
            span(),
        )
        .expect("valid format-only example");

        assert!(example.lang().is_none());
        assert_eq!(example.format(), Some("openapi-yaml"));
    }

    #[test]
    fn accepts_verified_example_with_lang_checks_sandbox() {
        let example = Example::try_new(
            ID,
            Some("verified"),
            Some("ts"),
            None,
            BODY,
            Some("npm run test -- credits"),
            Some("node-test"),
            BTreeMap::new(),
            span(),
        )
        .expect("valid verified example");

        assert_eq!(example.status(), Some(&LifecycleStatus::Verified));
        assert!(example.status().copied().unwrap().is_verified());
        assert_eq!(example.checks(), Some("npm run test -- credits"));
        assert_eq!(
            example.sandbox().map(SandboxName::as_str),
            Some("node-test")
        );
    }

    // --- build_from_parsed error tests ---

    #[test]
    fn build_from_parsed_reports_missing_lang_when_neither_lang_nor_format() {
        let parsed = parsed_example(BTreeMap::new(), BODY);
        let mut diagnostics = Vec::new();

        let result = Example::build_from_parsed(parsed, &mut diagnostics);

        assert!(result.is_none());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::SchemaExampleMissingLang
        );
        assert_eq!(diagnostics[0].object_id.as_deref(), Some(ID));
    }

    #[test]
    fn build_from_parsed_reports_verified_requires_sandbox_when_checks_but_no_sandbox() {
        let parsed = parsed_example(
            BTreeMap::from([
                (STATUS_FIELD.to_string(), "verified".to_string()),
                (LANG_FIELD.to_string(), "ts".to_string()),
                (CHECKS_FIELD.to_string(), "npm test".to_string()),
            ]),
            BODY,
        );
        let mut diagnostics = Vec::new();

        let result = Example::build_from_parsed(parsed, &mut diagnostics);

        assert!(result.is_none());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::SchemaExampleVerifiedRequiresSandbox
        );
    }

    #[test]
    fn build_from_parsed_reports_verified_requires_checks_when_sandbox_but_no_checks() {
        let parsed = parsed_example(
            BTreeMap::from([
                (STATUS_FIELD.to_string(), "verified".to_string()),
                (LANG_FIELD.to_string(), "ts".to_string()),
                (SANDBOX_FIELD.to_string(), "node-test".to_string()),
            ]),
            BODY,
        );
        let mut diagnostics = Vec::new();

        let result = Example::build_from_parsed(parsed, &mut diagnostics);

        assert!(result.is_none());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::SchemaExampleVerifiedRequiresChecks
        );
    }

    #[test]
    fn build_from_parsed_reports_both_requires_checks_and_requires_sandbox_when_verified_with_neither()
     {
        let parsed = parsed_example(
            BTreeMap::from([
                (STATUS_FIELD.to_string(), "verified".to_string()),
                (LANG_FIELD.to_string(), "ts".to_string()),
            ]),
            BODY,
        );
        let mut diagnostics = Vec::new();

        let result = Example::build_from_parsed(parsed, &mut diagnostics);

        assert!(result.is_none());
        assert_eq!(
            diagnostics.len(),
            2,
            "expected both requires-checks and requires-sandbox diagnostics: {diagnostics:?}"
        );
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::SchemaExampleVerifiedRequiresChecks
        );
        assert_eq!(
            diagnostics[1].code,
            DiagnosticCode::SchemaExampleVerifiedRequiresSandbox
        );
    }

    #[test]
    fn build_from_parsed_reports_invalid_lang_for_malformed_lang() {
        let parsed = parsed_example(
            BTreeMap::from([(LANG_FIELD.to_string(), "TS".to_string())]),
            BODY,
        );
        let mut diagnostics = Vec::new();

        let result = Example::build_from_parsed(parsed, &mut diagnostics);

        assert!(result.is_none());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::SchemaExampleInvalidLang
        );
        assert!(
            diagnostics[0].message.contains("TS"),
            "message should quote rejected lang: {:?}",
            diagnostics[0]
        );
    }

    #[test]
    fn build_from_parsed_reports_invalid_sandbox_for_malformed_sandbox() {
        let parsed = parsed_example(
            BTreeMap::from([
                (LANG_FIELD.to_string(), "ts".to_string()),
                (SANDBOX_FIELD.to_string(), "Node".to_string()),
            ]),
            BODY,
        );
        let mut diagnostics = Vec::new();

        let result = Example::build_from_parsed(parsed, &mut diagnostics);

        assert!(result.is_none());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::SchemaExampleInvalidSandbox
        );
        assert!(
            diagnostics[0].message.contains("Node"),
            "message should quote rejected sandbox: {:?}",
            diagnostics[0]
        );
    }

    #[test]
    fn build_from_parsed_reports_schema_invalid_status_for_bad_status() {
        let parsed = parsed_example(
            BTreeMap::from([
                (STATUS_FIELD.to_string(), "varified".to_string()),
                (LANG_FIELD.to_string(), "ts".to_string()),
            ]),
            BODY,
        );
        let mut diagnostics = Vec::new();

        let result = Example::build_from_parsed(parsed, &mut diagnostics);

        assert!(result.is_none());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::SchemaInvalidStatus);
        assert!(
            diagnostics[0].message.contains("varified"),
            "message should quote rejected status: {:?}",
            diagnostics[0]
        );
    }

    #[test]
    fn build_from_parsed_reports_schema_missing_field_for_missing_body() {
        let parsed = parsed_example(
            BTreeMap::from([(LANG_FIELD.to_string(), "ts".to_string())]),
            " ",
        );
        let mut diagnostics = Vec::new();

        let result = Example::build_from_parsed(parsed, &mut diagnostics);

        assert!(result.is_none());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::SchemaMissingField);
    }

    #[test]
    fn build_from_parsed_strips_typed_fields_and_preserves_optional_fields() {
        let parsed = parsed_example(
            BTreeMap::from([
                (STATUS_FIELD.to_string(), "draft".to_string()),
                (LANG_FIELD.to_string(), "ts".to_string()),
                (FORMAT_FIELD.to_string(), "esm".to_string()),
                (CHECKS_FIELD.to_string(), "npm test".to_string()),
                (SANDBOX_FIELD.to_string(), "node-test".to_string()),
                ("owner".to_string(), "team-billing".to_string()),
            ]),
            BODY,
        );
        let mut diagnostics = Vec::new();

        let example = Example::build_from_parsed(parsed, &mut diagnostics).expect("valid example");

        assert!(
            diagnostics.is_empty(),
            "unexpected diagnostics: {diagnostics:?}"
        );
        let field_keys: Vec<&str> = example.fields().iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(field_keys, vec!["owner"]);
    }

    #[test]
    fn build_from_parsed_captures_impacts() {
        let parsed = parsed_example(
            BTreeMap::from([
                (LANG_FIELD.to_string(), "ts".to_string()),
                (
                    "impacts".to_string(),
                    "[crates/billing/src/credits.ts]".to_string(),
                ),
            ]),
            BODY,
        );
        let mut diagnostics = Vec::new();

        let example = Example::build_from_parsed(parsed, &mut diagnostics).expect("valid example");

        assert!(diagnostics.is_empty());
        let impacts: Vec<&str> = example
            .impacts()
            .expect("impacts present")
            .iter()
            .map(RelPath::as_str)
            .collect();
        assert_eq!(impacts, vec!["crates/billing/src/credits.ts"]);
    }

    #[test]
    fn build_from_parsed_preserves_checks_command_line_verbatim() {
        let parsed = parsed_example(
            BTreeMap::from([
                (STATUS_FIELD.to_string(), "verified".to_string()),
                (LANG_FIELD.to_string(), "ts".to_string()),
                (
                    CHECKS_FIELD.to_string(),
                    "npm run test -- credits".to_string(),
                ),
                (SANDBOX_FIELD.to_string(), "node-test".to_string()),
            ]),
            BODY,
        );
        let mut diagnostics = Vec::new();

        let example =
            Example::build_from_parsed(parsed, &mut diagnostics).expect("valid verified example");

        assert!(
            diagnostics.is_empty(),
            "unexpected diagnostics: {diagnostics:?}"
        );
        assert_eq!(example.checks(), Some("npm run test -- credits"));
    }
}
