//! `api` Knowledge Object aggregate (V6.5.1, PRD §13.7).
//!
//! A typed API contract. Required fields: `id`, exactly one of `method`
//! (closed [`HttpMethod`]) or `interface_type` (open string: `grpc`,
//! `graphql`, …), exactly one of `path` (`/`-prefixed route template) or
//! `symbol` (code symbol), and `body`. Both one-of invariants follow
//! `source`'s path-XOR-url sum-type pattern. Statuses are the closed
//! `draft | verified | deprecated` set (the procedure pattern) and optional;
//! a `verified` api requires `owner`, `verified_at`, and evidence — the
//! schema-quality evidence rule (`api.verified_missing_schema_evidence`)
//! lives in `language/validate/api_verified_evidence.rs` because it
//! resolves `evidence_ref` targets across the workspace.

use std::collections::BTreeMap;

use crate::domain::ast::ParsedTypedBlock;
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, SourceSpan};
use crate::domain::identity::{OBJECT_ID_GRAMMAR_HELP, ObjectId, ObjectIdError};
use crate::domain::knowledge_object::Relations;
use crate::domain::knowledge_object::claim::{
    Evidence, OWNER_FIELD, Owner, REVIEWED_BY_FIELD, SOURCE_FIELD, TEST_FIELD, VERIFIED_AT_FIELD,
    Verification, VerifiedAt,
};
use crate::domain::value_objects::http_method::{HttpMethod, HttpMethodError};
use crate::domain::value_objects::lifecycle_status::{LifecycleStatus, LifecycleStatusError};
use crate::domain::value_objects::rel_path::RelPath;
use crate::domain::values::{Body, NonEmpty, OptionalFields, trim_ascii_edges};

pub(crate) const METHOD_FIELD: &str = "method";
pub(crate) const INTERFACE_TYPE_FIELD: &str = "interface_type";
pub(crate) const PATH_FIELD: &str = "path";
pub(crate) const SYMBOL_FIELD: &str = "symbol";
const STATUS_FIELD: &str = "status";

const API_INVALID_STATUS_HELP: &str = "Valid api statuses are: draft, verified, deprecated.";
const API_MISSING_BODY_HELP: &str =
    "Apis require non-empty body text describing the contract's behavior.";
const VERIFIED_API_HELP: &str = "Verified apis require `owner`, `verified_at`, and schema \
     evidence: an inline `source:` entry or an `evidence_ref` to an `api_schema`/`source_code` \
     source object.";

/// The operation half of the contract: a closed HTTP method or an open
/// interface type (`grpc`, `graphql`, …) — exactly one, never both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ApiOperation {
    Method(HttpMethod),
    InterfaceType(String),
}

/// The location half of the contract: a `/`-prefixed route template or a
/// code symbol — exactly one, never both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ApiLocation {
    Path(String),
    Symbol(String),
}

/// A typed API contract (PRD §13.7, V6.5.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Api {
    id: ObjectId,
    status: Option<LifecycleStatus>,
    operation: ApiOperation,
    location: ApiLocation,
    body: Body,
    fields: OptionalFields,
    verification: Option<Verification>,
    evidence_refs: Vec<Evidence>,
    relations: Relations,
    impacts: Option<NonEmpty<RelPath>>,
    span: SourceSpan,
}

/// Why an `api` failed to build from parsed input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ApiError {
    InvalidId(ObjectIdError),
    InvalidStatus(String),
    MissingBody,
    MissingMethodOrInterfaceType,
    ConflictingMethodAndInterfaceType,
    InvalidMethod(String),
    MissingPathOrSymbol,
    ConflictingPathAndSymbol,
    InvalidPath(String),
    // Constructed only by the test-only `try_new`; `build_from_parsed`
    // decides the pairing itself so these never reach production diagnostics.
    #[allow(dead_code)]
    MissingVerification,
    #[allow(dead_code)]
    UnexpectedVerification,
}

impl Api {
    pub(crate) fn build_from_parsed(
        mut parsed: ParsedTypedBlock,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Self> {
        if super::reject_duplicate_fields(&parsed, "api", diagnostics) {
            return None;
        }

        let id = match ObjectId::new(&parsed.id_text) {
            Ok(id) => Some(id),
            Err(error) => {
                emit_api_error(&parsed, ApiError::InvalidId(error), diagnostics);
                None
            }
        };

        let status = match parse_status(&mut parsed) {
            Ok(status) => status,
            Err(error) => {
                emit_api_error(&parsed, error, diagnostics);
                return None;
            }
        };

        let operation = parse_operation(&mut parsed, diagnostics);
        let location = parse_location(&mut parsed, diagnostics);

        let body = match super::body_from_parsed(&parsed) {
            Some(body) => Some(body),
            None => {
                emit_api_error(&parsed, ApiError::MissingBody, diagnostics);
                None
            }
        };

        // Parse evidence_refs BEFORE building verification so a verified api
        // whose only evidence is an `evidence_ref:` is accepted (the claim
        // precedent); the schema-quality gate is the workspace rule's job.
        let evidence_refs = super::parse_evidence_refs(&mut parsed, diagnostics);
        let verification = if status.as_ref().is_some_and(LifecycleStatus::is_verified) {
            let has_refs = !evidence_refs.is_empty();
            match build_verification(&parsed, &parsed.raw_fields, has_refs, diagnostics) {
                Some(verification) => Some(verification),
                None => return None,
            }
        } else {
            None
        };

        if id.is_none() || operation.is_none() || location.is_none() || body.is_none() {
            return None;
        }

        let relations = super::extract_relations(&mut parsed, diagnostics);
        let impacts = super::extract_impacts(&mut parsed, diagnostics);
        let mut optional_fields = std::mem::take(&mut parsed.raw_fields);
        if verification.is_some() {
            optional_fields.retain(|key, _| !is_verified_api_dedicated_field(key));
        }

        Some(Self {
            id: id.expect("checked above"),
            status,
            operation: operation.expect("checked above"),
            location: location.expect("checked above"),
            body: body.expect("checked above"),
            fields: OptionalFields::from_map(optional_fields),
            verification,
            evidence_refs,
            relations,
            impacts: None,
            span: parsed.span.clone(),
        })
        .map(|api| api.with_impacts(impacts))
    }

    /// Test-only constructor that bypasses the parsed-block pipeline.
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn try_new(
        id_text: &str,
        status_text: Option<&str>,
        method_text: Option<&str>,
        interface_type_text: Option<&str>,
        path_text: Option<&str>,
        symbol_text: Option<&str>,
        body_text: &str,
        optional_fields: BTreeMap<String, String>,
        verification: Option<Verification>,
        span: SourceSpan,
    ) -> Result<Self, ApiError> {
        let id = ObjectId::new(id_text).map_err(ApiError::InvalidId)?;
        let status = status_text.map(status_from_text).transpose()?;
        let operation = match (method_text, interface_type_text) {
            (Some(_), Some(_)) => return Err(ApiError::ConflictingMethodAndInterfaceType),
            (None, None) => return Err(ApiError::MissingMethodOrInterfaceType),
            (Some(method), None) => {
                ApiOperation::Method(HttpMethod::try_new(method).map_err(|error| match error {
                    HttpMethodError::Missing => ApiError::MissingMethodOrInterfaceType,
                    HttpMethodError::Invalid(value) => ApiError::InvalidMethod(value),
                })?)
            }
            (None, Some(interface_type)) => {
                ApiOperation::InterfaceType(trim_ascii_edges(interface_type).to_string())
            }
        };
        let location = match (path_text, symbol_text) {
            (Some(_), Some(_)) => return Err(ApiError::ConflictingPathAndSymbol),
            (None, None) => return Err(ApiError::MissingPathOrSymbol),
            (Some(path), None) => ApiLocation::Path(parse_path_template(path)?),
            (None, Some(symbol)) => ApiLocation::Symbol(trim_ascii_edges(symbol).to_string()),
        };
        let is_verified = status.as_ref().is_some_and(LifecycleStatus::is_verified);
        if is_verified && verification.is_none() {
            return Err(ApiError::MissingVerification);
        }
        if !is_verified && verification.is_some() {
            return Err(ApiError::UnexpectedVerification);
        }
        let body = Body::from_plain_text(body_text).ok_or(ApiError::MissingBody)?;
        Ok(Self {
            id,
            status,
            operation,
            location,
            body,
            fields: OptionalFields::from_map(optional_fields),
            verification,
            evidence_refs: Vec::new(),
            relations: Relations::empty(),
            impacts: None,
            span,
        })
    }

    pub(crate) fn with_impacts(mut self, impacts: Option<NonEmpty<RelPath>>) -> Self {
        self.impacts = impacts;
        self
    }

    // ── Accessors ────────────────────────────────────────────────────────────

    pub(crate) fn id(&self) -> &ObjectId {
        &self.id
    }

    pub(crate) fn status(&self) -> Option<&LifecycleStatus> {
        self.status.as_ref()
    }

    pub(crate) fn method(&self) -> Option<HttpMethod> {
        match &self.operation {
            ApiOperation::Method(method) => Some(*method),
            ApiOperation::InterfaceType(_) => None,
        }
    }

    pub(crate) fn interface_type(&self) -> Option<&str> {
        match &self.operation {
            ApiOperation::Method(_) => None,
            ApiOperation::InterfaceType(interface_type) => Some(interface_type),
        }
    }

    pub(crate) fn path(&self) -> Option<&str> {
        match &self.location {
            ApiLocation::Path(path) => Some(path),
            ApiLocation::Symbol(_) => None,
        }
    }

    pub(crate) fn symbol(&self) -> Option<&str> {
        match &self.location {
            ApiLocation::Path(_) => None,
            ApiLocation::Symbol(symbol) => Some(symbol),
        }
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

    pub(crate) fn verification(&self) -> Option<&Verification> {
        self.verification.as_ref()
    }

    pub(crate) fn evidence_refs(&self) -> &[Evidence] {
        &self.evidence_refs
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
}

/// Optional closed status. `status:` absent → `None`; present-but-blank is
/// also treated as absent (nothing to validate).
fn parse_status(parsed: &mut ParsedTypedBlock) -> Result<Option<LifecycleStatus>, ApiError> {
    super::take_optional_scalar(parsed, STATUS_FIELD, status_from_text)
}

fn parse_operation(
    parsed: &mut ParsedTypedBlock,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<ApiOperation> {
    let method_raw = parsed.raw_fields.remove(METHOD_FIELD);
    let interface_type_raw = parsed.raw_fields.remove(INTERFACE_TYPE_FIELD);

    match (method_raw, interface_type_raw) {
        (Some(_), Some(_)) => {
            emit_api_error(
                parsed,
                ApiError::ConflictingMethodAndInterfaceType,
                diagnostics,
            );
            None
        }
        (None, None) => {
            emit_api_error(parsed, ApiError::MissingMethodOrInterfaceType, diagnostics);
            None
        }
        (Some(method), None) => match HttpMethod::try_new(&method) {
            Ok(method) => Some(ApiOperation::Method(method)),
            Err(HttpMethodError::Missing) => {
                emit_api_error(parsed, ApiError::MissingMethodOrInterfaceType, diagnostics);
                None
            }
            Err(HttpMethodError::Invalid(value)) => {
                emit_api_error(parsed, ApiError::InvalidMethod(value), diagnostics);
                None
            }
        },
        (None, Some(interface_type)) => {
            let trimmed = trim_ascii_edges(&interface_type);
            if trimmed.is_empty() {
                emit_api_error(parsed, ApiError::MissingMethodOrInterfaceType, diagnostics);
                return None;
            }
            Some(ApiOperation::InterfaceType(trimmed.to_string()))
        }
    }
}

fn parse_location(
    parsed: &mut ParsedTypedBlock,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<ApiLocation> {
    let path_raw = parsed.raw_fields.remove(PATH_FIELD);
    let symbol_raw = parsed.raw_fields.remove(SYMBOL_FIELD);

    match (path_raw, symbol_raw) {
        (Some(_), Some(_)) => {
            emit_api_error(parsed, ApiError::ConflictingPathAndSymbol, diagnostics);
            None
        }
        (None, None) => {
            emit_api_error(parsed, ApiError::MissingPathOrSymbol, diagnostics);
            None
        }
        (Some(path), None) => match parse_path_template(&path) {
            Ok(path) => Some(ApiLocation::Path(path)),
            Err(error) => {
                emit_api_error(parsed, error, diagnostics);
                None
            }
        },
        (None, Some(symbol)) => {
            let trimmed = trim_ascii_edges(&symbol);
            if trimmed.is_empty() {
                emit_api_error(parsed, ApiError::MissingPathOrSymbol, diagnostics);
                return None;
            }
            Some(ApiLocation::Symbol(trimmed.to_string()))
        }
    }
}

/// A route template is a non-empty `/`-prefixed string — no deeper grammar
/// (PRD §13.7 scope).
fn parse_path_template(value: &str) -> Result<String, ApiError> {
    let trimmed = trim_ascii_edges(value);
    if !trimmed.starts_with('/') {
        return Err(ApiError::InvalidPath(trimmed.to_string()));
    }
    Ok(trimmed.to_string())
}

/// Mirror of the claim/procedure verification builder: `owner`, `verified_at`,
/// and at least one inline evidence entry OR one `evidence_ref`. The
/// api-specific schema-quality requirement is enforced by the
/// `api_verified_evidence` workspace rule, which can resolve ref targets.
fn build_verification(
    parsed: &ParsedTypedBlock,
    fields: &BTreeMap<String, String>,
    has_refs: bool,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Verification> {
    let owner = fields
        .get(OWNER_FIELD)
        .and_then(|value| Owner::try_new(value));
    if owner.is_none() {
        diagnostics.push(missing_verified_field_diagnostic(parsed, OWNER_FIELD));
    }

    let verified_at = fields
        .get(VERIFIED_AT_FIELD)
        .and_then(|value| VerifiedAt::try_new(value));
    if verified_at.is_none() {
        diagnostics.push(missing_verified_field_diagnostic(parsed, VERIFIED_AT_FIELD));
    }

    let mut evidence = Vec::new();
    for field in [SOURCE_FIELD, TEST_FIELD, REVIEWED_BY_FIELD] {
        if let Some(value) = fields
            .get(field)
            .and_then(|value| Evidence::from_field(field, value))
        {
            evidence.push(value);
        }
    }

    let has_inline_evidence = !evidence.is_empty();
    if !has_inline_evidence && !has_refs {
        diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::ApiVerifiedMissingSchemaEvidence,
                format!(
                    "verified api `{}` requires schema evidence: an inline `source:` entry or an `evidence_ref` to an `api_schema`/`source_code` source",
                    parsed.id_text
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(VERIFIED_API_HELP),
        );
    }

    if owner.is_none() || verified_at.is_none() || (!has_inline_evidence && !has_refs) {
        return None;
    }

    Some(Verification::new(
        owner.expect("owner checked above"),
        verified_at.expect("verified_at checked above"),
        evidence,
    ))
}

fn is_verified_api_dedicated_field(key: &str) -> bool {
    matches!(
        key,
        OWNER_FIELD | VERIFIED_AT_FIELD | SOURCE_FIELD | TEST_FIELD | REVIEWED_BY_FIELD
    )
}

fn missing_verified_field_diagnostic(parsed: &ParsedTypedBlock, field: &str) -> Diagnostic {
    Diagnostic::error(
        DiagnosticCode::SchemaMissingField,
        format!(
            "verified api `{}` is missing required field `{field}`",
            parsed.id_text
        ),
    )
    .with_span(parsed.span.clone())
    .with_object_id(&parsed.id_text)
    .with_help(format!("Add `{field}`. {VERIFIED_API_HELP}"))
}

fn emit_api_error(parsed: &ParsedTypedBlock, error: ApiError, diagnostics: &mut Vec<Diagnostic>) {
    let diagnostic = match error {
        ApiError::InvalidId(error) => Diagnostic::error(
            DiagnosticCode::IdInvalid,
            format!("invalid api id `{}`: {error}", parsed.id_text),
        )
        .with_help(OBJECT_ID_GRAMMAR_HELP),
        ApiError::InvalidStatus(status) => Diagnostic::error(
            DiagnosticCode::SchemaInvalidStatus,
            format!("api `{}` has invalid status `{status}`", parsed.id_text),
        )
        .with_help(API_INVALID_STATUS_HELP),
        ApiError::MissingBody => Diagnostic::error(
            DiagnosticCode::SchemaMissingField,
            format!("api `{}` is missing required body", parsed.id_text),
        )
        .with_help(API_MISSING_BODY_HELP),
        ApiError::MissingMethodOrInterfaceType => Diagnostic::error(
            DiagnosticCode::SchemaApiMissingMethodOrInterfaceType,
            format!(
                "api `{}` requires one of `method` or `interface_type`",
                parsed.id_text
            ),
        )
        .with_help(DiagnosticCode::SchemaApiMissingMethodOrInterfaceType.default_help()),
        ApiError::ConflictingMethodAndInterfaceType => Diagnostic::error(
            DiagnosticCode::SchemaApiConflictingMethodAndInterfaceType,
            format!(
                "api `{}` provides both `method` and `interface_type`",
                parsed.id_text
            ),
        )
        .with_help(DiagnosticCode::SchemaApiConflictingMethodAndInterfaceType.default_help()),
        ApiError::InvalidMethod(method) => Diagnostic::error(
            DiagnosticCode::SchemaApiInvalidMethod,
            format!("api `{}` has invalid method `{method}`", parsed.id_text),
        )
        .with_help(DiagnosticCode::SchemaApiInvalidMethod.default_help()),
        ApiError::MissingPathOrSymbol => Diagnostic::error(
            DiagnosticCode::SchemaApiMissingPathOrSymbol,
            format!(
                "api `{}` requires one of `path` or `symbol`",
                parsed.id_text
            ),
        )
        .with_help(DiagnosticCode::SchemaApiMissingPathOrSymbol.default_help()),
        ApiError::ConflictingPathAndSymbol => Diagnostic::error(
            DiagnosticCode::SchemaApiConflictingPathAndSymbol,
            format!("api `{}` provides both `path` and `symbol`", parsed.id_text),
        )
        .with_help(DiagnosticCode::SchemaApiConflictingPathAndSymbol.default_help()),
        ApiError::InvalidPath(path) => Diagnostic::error(
            DiagnosticCode::SchemaApiInvalidPath,
            format!("api `{}` has invalid path `{path}`", parsed.id_text),
        )
        .with_help(DiagnosticCode::SchemaApiInvalidPath.default_help()),
        ApiError::MissingVerification | ApiError::UnexpectedVerification => {
            unreachable!("verification pairing is decided by the builder, not authored input")
        }
    };
    diagnostics.push(
        diagnostic
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text),
    );
}

/// Maps the shared [`LifecycleStatus`] parse errors into api's own error
/// vocabulary: a blank status keeps its historical `InvalidStatus("")` shape.
pub(crate) fn status_from_text(value: &str) -> Result<LifecycleStatus, ApiError> {
    LifecycleStatus::try_new(value).map_err(|error| match error {
        LifecycleStatusError::Missing => ApiError::InvalidStatus(String::new()),
        LifecycleStatusError::Invalid(value) => ApiError::InvalidStatus(value),
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::domain::diagnostic::{SourcePosition, SourceSpan};
    use crate::domain::value_objects::evidence_kind::EvidenceKind;

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

    fn parsed_api(fields: BTreeMap<String, String>) -> ParsedTypedBlock {
        let body_text = "Consumes one or more credits for a completed generation job.";
        ParsedTypedBlock {
            kind_word: "api".to_string(),
            kind_word_span: span(),
            id_text: "billing.consume-credit".to_string(),
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

    fn verification() -> Verification {
        Verification::new(
            Owner::try_new("backend-platform").expect("owner"),
            VerifiedAt::try_new("2026-05-06").expect("verified_at"),
            vec![
                Evidence::inline(EvidenceKind::SourceCode, "openapi/billing.yaml").expect("source"),
            ],
        )
    }

    #[test]
    fn try_new_accepts_method_and_path() {
        let api = Api::try_new(
            "billing.consume-credit",
            Some("draft"),
            Some("POST"),
            None,
            Some("/api/billing/credits/consume"),
            None,
            "Consumes credits.",
            BTreeMap::new(),
            None,
            span(),
        )
        .expect("valid api");

        assert_eq!(api.id().as_str(), "billing.consume-credit");
        assert_eq!(api.method().map(|m| m.as_str()), Some("POST"));
        assert_eq!(api.path(), Some("/api/billing/credits/consume"));
        assert_eq!(api.interface_type(), None);
        assert_eq!(api.symbol(), None);
        assert_eq!(api.status().map(LifecycleStatus::as_str), Some("draft"));
    }

    #[test]
    fn try_new_accepts_interface_type_and_symbol() {
        let api = Api::try_new(
            "billing.consume-credit-grpc",
            None,
            None,
            Some("grpc"),
            None,
            Some("billing.v1.CreditService/Consume"),
            "Consumes credits over gRPC.",
            BTreeMap::new(),
            None,
            span(),
        )
        .expect("valid api");

        assert_eq!(api.interface_type(), Some("grpc"));
        assert_eq!(api.symbol(), Some("billing.v1.CreditService/Consume"));
        assert!(api.status().is_none());
    }

    #[test]
    fn try_new_rejects_both_method_and_interface_type() {
        let result = Api::try_new(
            "billing.consume-credit",
            None,
            Some("POST"),
            Some("grpc"),
            Some("/api/x"),
            None,
            "Body.",
            BTreeMap::new(),
            None,
            span(),
        );

        assert_eq!(result, Err(ApiError::ConflictingMethodAndInterfaceType));
    }

    #[test]
    fn try_new_rejects_neither_path_nor_symbol() {
        let result = Api::try_new(
            "billing.consume-credit",
            None,
            Some("POST"),
            None,
            None,
            None,
            "Body.",
            BTreeMap::new(),
            None,
            span(),
        );

        assert_eq!(result, Err(ApiError::MissingPathOrSymbol));
    }

    #[test]
    fn try_new_rejects_unprefixed_path() {
        let result = Api::try_new(
            "billing.consume-credit",
            None,
            Some("POST"),
            None,
            Some("api/billing"),
            None,
            "Body.",
            BTreeMap::new(),
            None,
            span(),
        );

        assert_eq!(
            result,
            Err(ApiError::InvalidPath("api/billing".to_string()))
        );
    }

    #[test]
    fn try_new_requires_verification_for_verified_status() {
        let result = Api::try_new(
            "billing.consume-credit",
            Some("verified"),
            Some("POST"),
            None,
            Some("/api/x"),
            None,
            "Body.",
            BTreeMap::new(),
            None,
            span(),
        );

        assert_eq!(result, Err(ApiError::MissingVerification));
    }

    #[test]
    fn try_new_rejects_verification_for_non_verified_status() {
        let result = Api::try_new(
            "billing.consume-credit",
            Some("draft"),
            Some("POST"),
            None,
            Some("/api/x"),
            None,
            "Body.",
            BTreeMap::new(),
            Some(verification()),
            span(),
        );

        assert_eq!(result, Err(ApiError::UnexpectedVerification));
    }

    #[test]
    fn build_from_parsed_accepts_the_prd_example() {
        // PRD §13.7 verbatim field set.
        let parsed = parsed_api(BTreeMap::from([
            ("method".to_string(), "POST".to_string()),
            (
                "path".to_string(),
                "/api/billing/credits/consume".to_string(),
            ),
            ("status".to_string(), "verified".to_string()),
            (
                "source".to_string(),
                "openapi/billing.yaml#/paths/~1credits~1consume".to_string(),
            ),
            ("owner".to_string(), "backend-platform".to_string()),
            ("verified_at".to_string(), "2026-05-06".to_string()),
        ]));
        let mut diagnostics = Vec::new();

        let api = Api::build_from_parsed(parsed, &mut diagnostics).expect("valid api");

        assert!(diagnostics.is_empty(), "diagnostics: {diagnostics:?}");
        assert_eq!(api.method().map(|m| m.as_str()), Some("POST"));
        assert_eq!(api.path(), Some("/api/billing/credits/consume"));
        assert!(api.status().expect("status").is_verified());
        let verification = api.verification().expect("verification");
        assert_eq!(verification.owner().as_str(), "backend-platform");
        assert_eq!(verification.evidence().len(), 1);
        assert_eq!(
            verification.evidence()[0].kind(),
            Some(EvidenceKind::SourceCode)
        );
    }

    #[test]
    fn build_from_parsed_reports_missing_method_or_interface_type() {
        let parsed = parsed_api(BTreeMap::from([(
            "path".to_string(),
            "/api/billing/credits/consume".to_string(),
        )]));
        let mut diagnostics = Vec::new();

        let api = Api::build_from_parsed(parsed, &mut diagnostics);

        assert!(api.is_none());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::SchemaApiMissingMethodOrInterfaceType
        );
        assert_eq!(
            diagnostics[0].object_id.as_deref(),
            Some("billing.consume-credit")
        );
    }

    #[test]
    fn build_from_parsed_reports_conflicting_method_and_interface_type() {
        let parsed = parsed_api(BTreeMap::from([
            ("method".to_string(), "POST".to_string()),
            ("interface_type".to_string(), "grpc".to_string()),
            ("path".to_string(), "/api/x".to_string()),
        ]));
        let mut diagnostics = Vec::new();

        let api = Api::build_from_parsed(parsed, &mut diagnostics);

        assert!(api.is_none());
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::SchemaApiConflictingMethodAndInterfaceType
        );
    }

    #[test]
    fn build_from_parsed_reports_invalid_method() {
        let parsed = parsed_api(BTreeMap::from([
            ("method".to_string(), "post".to_string()),
            ("path".to_string(), "/api/x".to_string()),
        ]));
        let mut diagnostics = Vec::new();

        let api = Api::build_from_parsed(parsed, &mut diagnostics);

        assert!(api.is_none());
        assert_eq!(diagnostics[0].code, DiagnosticCode::SchemaApiInvalidMethod);
    }

    #[test]
    fn build_from_parsed_reports_verified_with_only_reviewed_by_as_missing_schema_evidence() {
        // Acceptance case: `reviewed_by:` alone is inline HumanReview evidence,
        // which satisfies the generic verification shape — but the aggregate
        // still owns nothing api-specific here; the schema-quality rejection
        // happens in the api_verified_evidence workspace rule. This test pins
        // the aggregate-level part: verification builds, evidence is HumanReview.
        let parsed = parsed_api(BTreeMap::from([
            ("method".to_string(), "POST".to_string()),
            ("path".to_string(), "/api/x".to_string()),
            ("status".to_string(), "verified".to_string()),
            ("owner".to_string(), "backend-platform".to_string()),
            ("verified_at".to_string(), "2026-05-06".to_string()),
            ("reviewed_by".to_string(), "api-guild".to_string()),
        ]));
        let mut diagnostics = Vec::new();

        let api = Api::build_from_parsed(parsed, &mut diagnostics).expect("valid api");

        assert!(diagnostics.is_empty(), "diagnostics: {diagnostics:?}");
        let verification = api.verification().expect("verification");
        assert_eq!(
            verification.evidence()[0].kind(),
            Some(EvidenceKind::HumanReview)
        );
    }

    #[test]
    fn build_from_parsed_reports_verified_without_any_evidence() {
        let parsed = parsed_api(BTreeMap::from([
            ("method".to_string(), "POST".to_string()),
            ("path".to_string(), "/api/x".to_string()),
            ("status".to_string(), "verified".to_string()),
            ("owner".to_string(), "backend-platform".to_string()),
            ("verified_at".to_string(), "2026-05-06".to_string()),
        ]));
        let mut diagnostics = Vec::new();

        let api = Api::build_from_parsed(parsed, &mut diagnostics);

        assert!(api.is_none());
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::ApiVerifiedMissingSchemaEvidence
        );
    }

    #[test]
    fn build_from_parsed_captures_impacts_and_relations() {
        let parsed = parsed_api(BTreeMap::from([
            ("method".to_string(), "POST".to_string()),
            ("path".to_string(), "/api/x".to_string()),
            ("impacts".to_string(), "[openapi/billing.yaml]".to_string()),
            ("depends_on".to_string(), "billing.credits".to_string()),
        ]));
        let mut diagnostics = Vec::new();

        let api = Api::build_from_parsed(parsed, &mut diagnostics).expect("valid api");

        assert!(diagnostics.is_empty(), "diagnostics: {diagnostics:?}");
        let impacts: Vec<&str> = api
            .impacts()
            .expect("impacts present")
            .iter()
            .map(RelPath::as_str)
            .collect();
        assert_eq!(impacts, vec!["openapi/billing.yaml"]);
        let depends_on: Vec<&str> = api
            .relations()
            .targets(crate::domain::graph::GraphRelationKind::DependsOn)
            .iter()
            .map(|target| target.id().as_str())
            .collect();
        assert_eq!(depends_on, vec!["billing.credits"]);
    }

    #[test]
    fn status_try_new_rejects_unknown_values() {
        assert_eq!(
            status_from_text("active"),
            Err(ApiError::InvalidStatus("active".to_string()))
        );
    }
}
