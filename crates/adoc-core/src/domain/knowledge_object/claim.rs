use std::collections::BTreeMap;

use crate::domain::ast::ParsedTypedBlock;
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, SourceSpan};
use crate::domain::identity::{OBJECT_ID_GRAMMAR_HELP, ObjectId, ObjectIdError};
use crate::domain::knowledge_object::Relations;
use crate::domain::value_objects::rel_path::RelPath;
use crate::domain::values::{Body, NonEmpty, NonEmptyText, OptionalFields, trim_ascii_edges};

// Re-export the V5.8 typed Evidence so that modules that previously imported
// it from `claim` continue to compile unchanged.
pub(crate) use crate::domain::value_objects::evidence::Evidence;

pub(crate) const STATUS_FIELD: &str = "status";
pub(crate) const OWNER_FIELD: &str = "owner";
pub(crate) const VERIFIED_AT_FIELD: &str = "verified_at";
pub(crate) const SOURCE_FIELD: &str = "source";
pub(crate) const TEST_FIELD: &str = "test";
pub(crate) const REVIEWED_BY_FIELD: &str = "reviewed_by";
/// V5.2 evidence field accepted by `verified` procedures (not claims), where a
/// manual run stands in for an automated `test`. The shared `Evidence` type
/// carries it so procedures reuse claim's `Verification`; claim's own accepted
/// evidence set is unchanged.
pub(crate) const HUMAN_REVIEW_FIELD: &str = "human_review";
pub(crate) const VERIFIED_STATUS: &str = "verified";

const VERIFIED_CLAIM_HELP: &str = "Verified claims require `owner`, `verified_at`, and at least one of `source`, `test`, `reviewed_by`, or `evidence_ref`.";
const CLAIM_MISSING_STATUS_HELP: &str = "Claims require non-empty `status`.";
const CLAIM_MISSING_BODY_HELP: &str = "Claims require non-empty body text.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Claim {
    id: ObjectId,
    status: ClaimStatus,
    body: Body,
    fields: OptionalFields,
    verification: Option<Verification>,
    /// V5.8 TB2: object-reference evidence entries. Each entry is an
    /// [`Evidence::ObjectRef`] naming a `source` Knowledge Object. Parsed from
    /// the `evidence_ref:` field; consumed before storing optional fields so
    /// it does not appear in generic output.
    evidence_refs: Vec<Evidence>,
    relations: Relations,
    impacts: Option<NonEmpty<RelPath>>,
    span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ClaimError {
    InvalidId(ObjectIdError),
    MissingStatus,
    MissingBody,
    MissingVerification,
    UnexpectedVerification,
    UnexpectedDedicatedField(&'static str),
}

impl Claim {
    pub(crate) fn build_from_parsed(
        mut parsed: ParsedTypedBlock,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Self> {
        if super::reject_duplicate_fields(&parsed, "claim", diagnostics) {
            return None;
        }

        let status_text = parsed.raw_fields.remove(STATUS_FIELD);
        let status_text = status_text.as_deref();

        let (id, status, body) = match Self::parse_basics_from_parsed(&parsed, status_text) {
            Ok(basics) => basics,
            Err(error) => {
                emit_claim_error(&parsed, error, diagnostics);
                return None;
            }
        };

        if status.is_verified() {
            return Self::build_verified_from_parsed(parsed, id, status, body, diagnostics);
        }

        let evidence_refs = super::parse_evidence_refs(&mut parsed, diagnostics);
        let relations = super::extract_relations(&mut parsed, diagnostics);
        let impacts = super::extract_impacts(&mut parsed, diagnostics);
        let optional_fields = std::mem::take(&mut parsed.raw_fields);

        match Self::from_parts(
            id,
            status,
            body,
            optional_fields,
            evidence_refs,
            None,
            relations,
            parsed.span.clone(),
        ) {
            Ok(claim) => {
                if claim.status().is_verified_ascii_case_variant() {
                    diagnostics.push(status_casing_diagnostic(&parsed, claim.status().as_str()));
                }
                Some(claim.with_impacts(impacts))
            }
            Err(error) => {
                emit_claim_error(&parsed, error, diagnostics);
                None
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn try_new(
        id_text: &str,
        status_text: Option<&str>,
        body_text: &str,
        optional_fields: BTreeMap<String, String>,
        verification: Option<Verification>,
        span: SourceSpan,
    ) -> Result<Self, ClaimError> {
        let (id, status, body) = Self::parse_basics(id_text, status_text, body_text)?;
        Self::from_parts(
            id,
            status,
            body,
            optional_fields,
            Vec::new(),
            verification,
            Relations::empty(),
            span,
        )
    }

    /// Test-only constructor that also accepts evidence refs.
    ///
    /// Each `ObjectId` in `ref_ids` is wrapped in `Evidence::ObjectRef`.
    #[cfg(test)]
    pub(crate) fn try_new_with_refs(
        id_text: &str,
        status_text: Option<&str>,
        body_text: &str,
        optional_fields: BTreeMap<String, String>,
        ref_ids: Vec<ObjectId>,
        verification: Option<Verification>,
        span: SourceSpan,
    ) -> Result<Self, ClaimError> {
        let (id, status, body) = Self::parse_basics(id_text, status_text, body_text)?;
        let evidence_refs = ref_ids.into_iter().map(Evidence::object_ref).collect();
        Self::from_parts(
            id,
            status,
            body,
            optional_fields,
            evidence_refs,
            verification,
            Relations::empty(),
            span,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn from_parts(
        id: ObjectId,
        status: ClaimStatus,
        body: Body,
        optional_fields: BTreeMap<String, String>,
        evidence_refs: Vec<Evidence>,
        verification: Option<Verification>,
        relations: Relations,
        span: SourceSpan,
    ) -> Result<Self, ClaimError> {
        if status.is_verified() && verification.is_none() {
            return Err(ClaimError::MissingVerification);
        }
        if !status.is_verified() && verification.is_some() {
            return Err(ClaimError::UnexpectedVerification);
        }
        debug_assert!(
            !optional_fields.contains_key(STATUS_FIELD),
            "optional claim fields must not contain required field `status`"
        );
        if verification.is_some()
            && let Some(field) = verified_claim_dedicated_field_in(&optional_fields)
        {
            return Err(ClaimError::UnexpectedDedicatedField(field));
        }
        let fields = OptionalFields::from_map(optional_fields);
        Ok(Self {
            id,
            status,
            body,
            fields,
            verification,
            evidence_refs,
            relations,
            impacts: None,
            span,
        })
    }

    /// Attach the (already validated) opt-in `impacts:` list. Returns `self`
    /// for fluent composition by the V3.3 build pipeline.
    pub(crate) fn with_impacts(mut self, impacts: Option<NonEmpty<RelPath>>) -> Self {
        self.impacts = impacts;
        self
    }

    #[cfg(test)]
    fn parse_basics(
        id_text: &str,
        status_text: Option<&str>,
        body_text: &str,
    ) -> Result<(ObjectId, ClaimStatus, Body), ClaimError> {
        let id = ObjectId::new(id_text).map_err(ClaimError::InvalidId)?;
        let status = ClaimStatus::try_new(status_text.unwrap_or(""))?;
        let body = Body::from_plain_text(body_text).ok_or(ClaimError::MissingBody)?;
        Ok((id, status, body))
    }

    fn parse_basics_from_parsed(
        parsed: &ParsedTypedBlock,
        status_text: Option<&str>,
    ) -> Result<(ObjectId, ClaimStatus, Body), ClaimError> {
        let id = ObjectId::new(&parsed.id_text).map_err(ClaimError::InvalidId)?;
        let status = ClaimStatus::try_new(status_text.unwrap_or(""))?;
        let body = super::body_from_parsed(parsed).ok_or(ClaimError::MissingBody)?;
        Ok((id, status, body))
    }

    pub(crate) fn id(&self) -> &ObjectId {
        &self.id
    }

    pub(crate) fn status(&self) -> &ClaimStatus {
        &self.status
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

    pub(crate) fn relations(&self) -> &Relations {
        &self.relations
    }

    /// V5.8 TB2: the object-reference evidence entries from the
    /// `evidence_ref:` field. Each entry is `Evidence::ObjectRef`. Empty when
    /// none were authored.
    pub(crate) fn evidence_refs(&self) -> &[Evidence] {
        &self.evidence_refs
    }

    pub(crate) fn impacts(&self) -> Option<&[RelPath]> {
        self.impacts.as_ref().map(NonEmpty::as_slice)
    }

    pub(crate) fn span(&self) -> &SourceSpan {
        &self.span
    }

    fn build_verified_from_parsed(
        mut parsed: ParsedTypedBlock,
        id: ObjectId,
        status: ClaimStatus,
        body: Body,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Self> {
        // Parse evidence_refs BEFORE building verification so that a verified
        // claim whose only evidence is an `evidence_ref:` is accepted.  The
        // ref count is threaded into `build_verification` to suppress the
        // `ClaimVerifiedMissingEvidence` diagnostic when at least one ref exists.
        let evidence_refs = super::parse_evidence_refs(&mut parsed, diagnostics);
        let has_refs = !evidence_refs.is_empty();
        let verification = build_verification(&parsed, &parsed.raw_fields, has_refs, diagnostics)?;
        let relations = super::extract_relations(&mut parsed, diagnostics);
        let impacts = super::extract_impacts(&mut parsed, diagnostics);
        let optional_fields = std::mem::take(&mut parsed.raw_fields);
        let storage_fields = verified_claim_storage_fields(optional_fields);

        match Self::from_parts(
            id,
            status,
            body,
            storage_fields,
            evidence_refs,
            Some(verification),
            relations,
            parsed.span.clone(),
        ) {
            Ok(claim) => Some(claim.with_impacts(impacts)),
            Err(error) => {
                emit_claim_error(&parsed, error, diagnostics);
                None
            }
        }
    }
}

// parse_evidence_refs is now the shared implementation in `super` (mod.rs).
// It is called via `super::parse_evidence_refs` below.

/// Build a `Verification` from parsed fields.
///
/// `has_refs` signals that the caller already parsed at least one valid
/// `evidence_ref:` entry.  When `true` the missing-evidence diagnostic is
/// suppressed even if there is no inline evidence, because refs count as
/// evidence under the V5.8 rule.
///
/// NOTE — kind-gating of refs is deferred: we do not reject a verified claim
/// whose only evidence is a ref to a non-accepted-kind source; that requires
/// cross-object resolution which is out of scope for TB4.
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
    if let Some(value) = fields
        .get(SOURCE_FIELD)
        .and_then(|value| Evidence::from_field(SOURCE_FIELD, value))
    {
        evidence.push(value);
    }
    if let Some(value) = fields
        .get(TEST_FIELD)
        .and_then(|value| Evidence::from_field(TEST_FIELD, value))
    {
        evidence.push(value);
    }
    if let Some(value) = fields
        .get(REVIEWED_BY_FIELD)
        .and_then(|value| Evidence::from_field(REVIEWED_BY_FIELD, value))
    {
        evidence.push(value);
    }

    // Emit missing-evidence diagnostic only when NEITHER inline evidence NOR
    // an evidence_ref is present.
    let has_inline_evidence = !evidence.is_empty();
    if !has_inline_evidence && !has_refs {
        diagnostics.push(missing_evidence_diagnostic(parsed));
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

fn verified_claim_storage_fields(mut fields: BTreeMap<String, String>) -> BTreeMap<String, String> {
    fields.retain(|key, _| !is_verified_claim_dedicated_field(key));
    fields
}

fn emit_claim_error(
    parsed: &ParsedTypedBlock,
    error: ClaimError,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match error {
        ClaimError::InvalidId(error) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::IdInvalid,
                format!("invalid claim id `{}`: {error}", parsed.id_text),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(OBJECT_ID_GRAMMAR_HELP),
        ),
        ClaimError::MissingStatus => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaMissingField,
                "claim is missing required field `status`",
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(CLAIM_MISSING_STATUS_HELP),
        ),
        ClaimError::MissingBody => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaMissingField,
                "claim is missing required body",
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(CLAIM_MISSING_BODY_HELP),
        ),
        ClaimError::MissingVerification => {
            unreachable!("missing verification is handled by verified-claim diagnostics")
        }
        ClaimError::UnexpectedVerification => {
            unreachable!("claim builder only passes verification for exact verified claims")
        }
        ClaimError::UnexpectedDedicatedField(_) => {
            unreachable!("claim builder strips verification fields before construction")
        }
    }
}

fn missing_verified_field_diagnostic(parsed: &ParsedTypedBlock, field: &str) -> Diagnostic {
    Diagnostic::error(
        DiagnosticCode::SchemaMissingField,
        format!(
            "verified claim `{}` is missing required field `{field}`",
            parsed.id_text
        ),
    )
    .with_span(parsed.span.clone())
    .with_object_id(&parsed.id_text)
    .with_help(format!("Add `{field}`. {VERIFIED_CLAIM_HELP}"))
}

fn missing_evidence_diagnostic(parsed: &ParsedTypedBlock) -> Diagnostic {
    Diagnostic::error(
        DiagnosticCode::ClaimVerifiedMissingEvidence,
        format!(
            "verified claim `{}` requires at least one evidence field: `source`, `test`, `reviewed_by`, or `evidence_ref`",
            parsed.id_text
        ),
    )
    .with_span(parsed.span.clone())
    .with_object_id(&parsed.id_text)
    .with_help(VERIFIED_CLAIM_HELP)
}

fn status_casing_diagnostic(parsed: &ParsedTypedBlock, status: &str) -> Diagnostic {
    Diagnostic::warning(
        DiagnosticCode::ClaimStatusCasing,
        format!(
            "claim `{}` uses status `{status}`; use exact lowercase `verified` to enable verified-claim rules",
            parsed.id_text
        ),
    )
    .with_span(parsed.span.clone())
    .with_object_id(&parsed.id_text)
    .with_help("Status values are case-sensitive; only `verified` creates a Verified Claim.")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClaimStatus(String);

impl ClaimStatus {
    pub(crate) fn try_new(s: &str) -> Result<Self, ClaimError> {
        let trimmed = trim_ascii_edges(s);
        if trimmed.is_empty() {
            return Err(ClaimError::MissingStatus);
        }
        Ok(Self(trimmed.to_string()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn is_verified(&self) -> bool {
        self.0 == VERIFIED_STATUS
    }

    pub(crate) fn is_verified_ascii_case_variant(&self) -> bool {
        self.0 != VERIFIED_STATUS && self.0.eq_ignore_ascii_case(VERIFIED_STATUS)
    }
}

fn is_verified_claim_dedicated_field(key: &str) -> bool {
    matches!(
        key,
        OWNER_FIELD | VERIFIED_AT_FIELD | SOURCE_FIELD | TEST_FIELD | REVIEWED_BY_FIELD
    )
}

fn verified_claim_dedicated_field_in(fields: &BTreeMap<String, String>) -> Option<&'static str> {
    [
        OWNER_FIELD,
        VERIFIED_AT_FIELD,
        SOURCE_FIELD,
        TEST_FIELD,
        REVIEWED_BY_FIELD,
    ]
    .into_iter()
    .find(|field| fields.contains_key(*field))
}

/// Verified-claim verification data.
///
/// `evidence` holds the **inline** evidence entries (`source`, `test`,
/// `reviewed_by`).  It may be empty when the claim supplies evidence
/// exclusively via `evidence_ref:` entries (V5.8 TB4).  The ref entries live
/// in `Claim::evidence_refs` and are emitted separately by the graph
/// assembler, so they are not duplicated here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Verification {
    owner: Owner,
    verified_at: VerifiedAt,
    /// Inline evidence entries.  May be empty when all evidence comes from
    /// `evidence_ref:` entries — in that case `Claim::evidence_refs()` is
    /// non-empty and the missing-evidence diagnostic was not emitted.
    evidence: Vec<Evidence>,
}

impl Verification {
    pub(crate) fn new(owner: Owner, verified_at: VerifiedAt, evidence: Vec<Evidence>) -> Self {
        Self {
            owner,
            verified_at,
            evidence,
        }
    }

    pub(crate) fn owner(&self) -> &Owner {
        &self.owner
    }

    pub(crate) fn verified_at(&self) -> &VerifiedAt {
        &self.verified_at
    }

    pub(crate) fn evidence(&self) -> &[Evidence] {
        &self.evidence
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Owner(String);

impl Owner {
    pub(crate) fn try_new(value: &str) -> Option<Self> {
        NonEmptyText::try_new(value).map(|value| Self(value.as_str().to_string()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VerifiedAt(String);

impl VerifiedAt {
    pub(crate) fn try_new(value: &str) -> Option<Self> {
        NonEmptyText::try_new(value).map(|value| Self(value.as_str().to_string()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

// Evidence and EvidenceValue are now defined in
// `crate::domain::value_objects::evidence` and re-exported at the top of this
// module via `pub(crate) use`. The old enum variants (Source/Test/ReviewedBy/
// HumanReview) and `field_key()` have been removed in V5.8 (TB1).

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::domain::diagnostic::{SourcePosition, SourceSpan};
    use crate::domain::knowledge_object::EVIDENCE_REF_FIELD;
    use crate::domain::value_objects::evidence::EvidenceValue;

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

    fn parsed_claim(fields: BTreeMap<String, String>, body_text: &str) -> ParsedTypedBlock {
        ParsedTypedBlock {
            kind_word: "claim".to_string(),
            kind_word_span: span(),
            id_text: "billing.credits".to_string(),
            raw_fields: fields,
            raw_field_spans: BTreeMap::new(),
            duplicate_keys: Vec::new(),
            body_text: body_text.to_string(),
            body_inlines: ParsedTypedBlock::test_body_inlines_from_text(body_text),
            body_spans: Vec::new(),
            content_spans: Vec::new(),
            span: span(),
        }
    }

    #[test]
    fn claim_status_try_new_rejects_empty() {
        assert_eq!(ClaimStatus::try_new(""), Err(ClaimError::MissingStatus));
    }

    #[test]
    fn claim_status_try_new_rejects_whitespace_only() {
        assert_eq!(
            ClaimStatus::try_new("   \t  "),
            Err(ClaimError::MissingStatus)
        );
    }

    #[test]
    fn claim_status_try_new_trims_and_accepts() {
        let status = ClaimStatus::try_new("  verified  ").expect("valid status");
        assert_eq!(status.as_str(), "verified");
    }

    #[test]
    fn claim_status_try_new_preserves_non_ascii_edge_whitespace() {
        let status = ClaimStatus::try_new("\u{00a0}verified\u{00a0}").expect("valid status");
        assert_eq!(status.as_str(), "\u{00a0}verified\u{00a0}");
    }

    #[test]
    fn claim_body_try_new_rejects_empty() {
        assert!(Body::from_plain_text("").is_none());
    }

    #[test]
    fn claim_body_try_new_rejects_ascii_whitespace_only() {
        assert!(Body::from_plain_text("   \t  ").is_none());
    }

    #[test]
    fn claim_body_try_new_trims_and_accepts() {
        let body = Body::from_plain_text("  some claim body  ").expect("valid body");
        assert_eq!(body.to_source(), "some claim body");
    }

    #[test]
    fn claim_body_try_new_preserves_non_ascii_edge_whitespace() {
        let body = Body::from_plain_text("\u{00a0}some claim body\u{00a0}").expect("valid body");
        assert_eq!(body.to_source(), "\u{00a0}some claim body\u{00a0}");
    }

    #[test]
    fn claim_fields_default_is_empty_and_iterates_in_sorted_key_order() {
        let default_fields = OptionalFields::default();
        assert!(default_fields.is_empty());

        let mut map = BTreeMap::new();
        map.insert("zebra".to_string(), "z".to_string());
        map.insert("apple".to_string(), "a".to_string());
        map.insert("mango".to_string(), "m".to_string());
        let fields = OptionalFields::from_map(map);

        assert!(!fields.is_empty());
        let keys: Vec<&str> = fields.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, vec!["apple", "mango", "zebra"]);
    }

    #[test]
    fn claim_try_new_happy_path_with_required_fields_only() {
        let claim = Claim::try_new(
            "billing.credits",
            Some("plain"),
            "x",
            BTreeMap::new(),
            None,
            span(),
        )
        .expect("valid claim");

        assert_eq!(claim.id().as_str(), "billing.credits");
        assert_eq!(claim.status().as_str(), "plain");
        assert_eq!(claim.body().to_source(), "x");
        assert!(claim.fields().is_empty());
        assert!(claim.verification().is_none());
    }

    #[test]
    fn claim_try_new_happy_path_with_optional_fields() {
        let optional = BTreeMap::from([("owner".to_string(), "team-a".to_string())]);
        let claim = Claim::try_new(
            "billing.credits",
            Some("plain"),
            "some body",
            optional,
            None,
            span(),
        )
        .expect("valid claim");

        // status_text does not contaminate ClaimFields
        let field_keys: Vec<&str> = claim.fields().iter().map(|(k, _)| k.as_str()).collect();
        assert!(!field_keys.contains(&"status"));
        assert_eq!(field_keys, vec!["owner"]);
    }

    #[test]
    fn claim_try_new_rejects_verified_dedicated_fields_when_verification_exists() {
        let optional = BTreeMap::from([
            (OWNER_FIELD.to_string(), "team-a".to_string()),
            ("audience".to_string(), "support".to_string()),
        ]);

        let result = Claim::try_new(
            "billing.credits",
            Some("verified"),
            "some body",
            optional,
            Some(verification()),
            span(),
        );

        assert_eq!(
            result,
            Err(ClaimError::UnexpectedDedicatedField(OWNER_FIELD))
        );
    }

    #[test]
    fn claim_try_new_invalid_id_returns_invalid_id() {
        let result = Claim::try_new(
            "BadId",
            Some("plain"),
            "body",
            BTreeMap::new(),
            None,
            span(),
        );
        assert!(matches!(result, Err(ClaimError::InvalidId(_))));
    }

    #[test]
    fn claim_try_new_missing_status_returns_missing_status() {
        let result = Claim::try_new(
            "billing.credits",
            None,
            "body",
            BTreeMap::new(),
            None,
            span(),
        );
        assert_eq!(result, Err(ClaimError::MissingStatus));
    }

    #[test]
    fn claim_try_new_missing_status_when_empty_string() {
        let result = Claim::try_new(
            "billing.credits",
            Some("   "),
            "body",
            BTreeMap::new(),
            None,
            span(),
        );
        assert_eq!(result, Err(ClaimError::MissingStatus));
    }

    #[test]
    fn claim_try_new_missing_body_returns_missing_body() {
        let result = Claim::try_new(
            "billing.credits",
            Some("plain"),
            "",
            BTreeMap::new(),
            None,
            span(),
        );
        assert_eq!(result, Err(ClaimError::MissingBody));
    }

    #[test]
    fn claim_try_new_requires_verification_for_exact_verified_status() {
        let result = Claim::try_new(
            "billing.credits",
            Some("verified"),
            "body",
            BTreeMap::new(),
            None,
            span(),
        );

        assert_eq!(result, Err(ClaimError::MissingVerification));
    }

    #[test]
    fn claim_try_new_rejects_verification_for_non_verified_status() {
        let result = Claim::try_new(
            "billing.credits",
            Some("plain"),
            "body",
            BTreeMap::new(),
            Some(verification()),
            span(),
        );

        assert_eq!(result, Err(ClaimError::UnexpectedVerification));
    }

    #[test]
    fn claim_try_new_accepts_exact_verified_with_verification() {
        let claim = Claim::try_new(
            "billing.credits",
            Some("verified"),
            "body",
            BTreeMap::new(),
            Some(verification()),
            span(),
        )
        .expect("valid verified claim");

        assert!(claim.status().is_verified());
        assert_eq!(
            claim.verification().expect("verification").owner().as_str(),
            "team-billing"
        );
    }

    #[test]
    fn claim_build_from_parsed_reports_verified_claim_missing_evidence() {
        let mut diagnostics = Vec::new();
        let parsed = parsed_claim(
            BTreeMap::from([
                (STATUS_FIELD.to_string(), VERIFIED_STATUS.to_string()),
                (OWNER_FIELD.to_string(), "team-billing".to_string()),
                (VERIFIED_AT_FIELD.to_string(), "2026-05-05".to_string()),
            ]),
            "body",
        );

        let claim = Claim::build_from_parsed(parsed, &mut diagnostics);

        assert!(claim.is_none());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::ClaimVerifiedMissingEvidence
        );
        assert_eq!(diagnostics[0].object_id.as_deref(), Some("billing.credits"));
    }

    #[test]
    fn claim_build_from_parsed_extracts_impacts_field_sorted_and_deduplicated() {
        let mut diagnostics = Vec::new();
        let parsed = parsed_claim(
            BTreeMap::from([
                (STATUS_FIELD.to_string(), "plain".to_string()),
                (
                    "impacts".to_string(),
                    "crates/billing/src/refund.rs, src/a.rs, src/a.rs".to_string(),
                ),
            ]),
            "body",
        );

        let claim = Claim::build_from_parsed(parsed, &mut diagnostics).expect("valid claim");

        assert!(diagnostics.is_empty(), "diagnostics: {diagnostics:?}");
        let impacts = claim.impacts().expect("impacts present");
        let strs: Vec<&str> = impacts.iter().map(|p| p.as_str()).collect();
        assert_eq!(strs, vec!["crates/billing/src/refund.rs", "src/a.rs"]);
    }

    #[test]
    fn claim_build_from_parsed_accepts_bracketed_impacts_syntax() {
        let mut diagnostics = Vec::new();
        let parsed = parsed_claim(
            BTreeMap::from([
                (STATUS_FIELD.to_string(), "plain".to_string()),
                ("impacts".to_string(), "[a.rs, b.rs]".to_string()),
            ]),
            "body",
        );

        let claim = Claim::build_from_parsed(parsed, &mut diagnostics).expect("valid claim");

        assert!(diagnostics.is_empty(), "diagnostics: {diagnostics:?}");
        let impacts = claim.impacts().expect("impacts present");
        let strs: Vec<&str> = impacts.iter().map(|p| p.as_str()).collect();
        assert_eq!(strs, vec!["a.rs", "b.rs"]);
    }

    #[test]
    fn claim_build_from_parsed_rejects_parent_segment_impacts_path() {
        let mut diagnostics = Vec::new();
        let parsed = parsed_claim(
            BTreeMap::from([
                (STATUS_FIELD.to_string(), "plain".to_string()),
                ("impacts".to_string(), "..".to_string()),
            ]),
            "body",
        );

        let claim = Claim::build_from_parsed(parsed, &mut diagnostics).expect("valid claim");

        assert!(claim.impacts().is_none());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::SchemaImpactsInvalidPath
        );
        assert_eq!(diagnostics[0].object_id.as_deref(), Some("billing.credits"));
    }

    #[test]
    fn claim_build_from_parsed_rejects_absolute_impacts_path() {
        let mut diagnostics = Vec::new();
        let parsed = parsed_claim(
            BTreeMap::from([
                (STATUS_FIELD.to_string(), "plain".to_string()),
                ("impacts".to_string(), "/etc/passwd".to_string()),
            ]),
            "body",
        );

        Claim::build_from_parsed(parsed, &mut diagnostics).expect("valid claim");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::SchemaImpactsInvalidPath
        );
    }

    #[test]
    fn claim_build_from_parsed_rejects_backslash_impacts_path() {
        // Windows-shape author input never matches `git diff --name-only`
        // output, so the schema validator must surface it as
        // `SchemaImpactsInvalidPath` rather than letting it through silently.
        let mut diagnostics = Vec::new();
        let parsed = parsed_claim(
            BTreeMap::from([
                (STATUS_FIELD.to_string(), "plain".to_string()),
                (
                    "impacts".to_string(),
                    "crates\\billing\\refund.rs".to_string(),
                ),
            ]),
            "body",
        );

        let claim = Claim::build_from_parsed(parsed, &mut diagnostics).expect("valid claim");

        assert!(claim.impacts().is_none());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::SchemaImpactsInvalidPath
        );
    }

    #[test]
    fn claim_build_from_parsed_rejects_drive_letter_impacts_path() {
        let mut diagnostics = Vec::new();
        let parsed = parsed_claim(
            BTreeMap::from([
                (STATUS_FIELD.to_string(), "plain".to_string()),
                ("impacts".to_string(), "C:/billing/refund.rs".to_string()),
            ]),
            "body",
        );

        let claim = Claim::build_from_parsed(parsed, &mut diagnostics).expect("valid claim");

        assert!(claim.impacts().is_none());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::SchemaImpactsInvalidPath
        );
    }

    #[test]
    fn claim_build_from_parsed_reports_empty_impacts_field() {
        let mut diagnostics = Vec::new();
        let parsed = parsed_claim(
            BTreeMap::from([
                (STATUS_FIELD.to_string(), "plain".to_string()),
                ("impacts".to_string(), "".to_string()),
            ]),
            "body",
        );

        let claim = Claim::build_from_parsed(parsed, &mut diagnostics).expect("valid claim");

        assert!(claim.impacts().is_none());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::SchemaImpactsEmpty);
    }

    #[test]
    fn claim_build_from_parsed_reports_empty_bracketed_impacts_field() {
        let mut diagnostics = Vec::new();
        let parsed = parsed_claim(
            BTreeMap::from([
                (STATUS_FIELD.to_string(), "plain".to_string()),
                ("impacts".to_string(), "[]".to_string()),
            ]),
            "body",
        );

        let claim = Claim::build_from_parsed(parsed, &mut diagnostics).expect("valid claim");

        assert!(claim.impacts().is_none());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::SchemaImpactsEmpty);
    }

    #[test]
    fn claim_build_from_parsed_keeps_valid_impacts_alongside_invalid_ones() {
        let mut diagnostics = Vec::new();
        let parsed = parsed_claim(
            BTreeMap::from([
                (STATUS_FIELD.to_string(), "plain".to_string()),
                ("impacts".to_string(), "src/good.rs, ..".to_string()),
            ]),
            "body",
        );

        let claim = Claim::build_from_parsed(parsed, &mut diagnostics).expect("valid claim");

        let impacts = claim.impacts().expect("good path retained");
        let strs: Vec<&str> = impacts.iter().map(|p| p.as_str()).collect();
        assert_eq!(strs, vec!["src/good.rs"]);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::SchemaImpactsInvalidPath
        );
    }

    #[test]
    fn claim_status_detects_ascii_case_verified_variant() {
        let status = ClaimStatus::try_new("Verified").expect("valid status");

        assert!(status.is_verified_ascii_case_variant());
        assert!(!status.is_verified());
    }

    #[test]
    fn evidence_values_trim_and_reject_empty() {
        assert_eq!(
            EvidenceValue::try_new("  test evidence  ")
                .expect("evidence")
                .as_str(),
            "test evidence"
        );
        assert!(EvidenceValue::try_new(" \t ").is_none());
    }

    #[test]
    fn non_empty_rejects_empty_vec() {
        assert!(NonEmpty::<Evidence>::from_vec(Vec::new()).is_none());
    }

    #[test]
    fn non_empty_preserves_values() {
        use crate::domain::value_objects::evidence_kind::EvidenceKind;
        let evidence = NonEmpty::from_vec(vec![
            Evidence::inline(EvidenceKind::SourceCode, "runbook").expect("evidence"),
        ])
        .expect("non-empty evidence");

        assert_eq!(evidence.as_slice().len(), 1);
        assert_eq!(
            evidence.as_slice()[0].kind(),
            Some(EvidenceKind::SourceCode)
        );
    }

    fn verification() -> Verification {
        use crate::domain::value_objects::evidence_kind::EvidenceKind;
        Verification::new(
            Owner::try_new("team-billing").expect("owner"),
            VerifiedAt::try_new("2026-05-05").expect("verified_at"),
            vec![Evidence::inline(EvidenceKind::SourceCode, "runbook").expect("evidence")],
        )
    }

    // ── evidence_ref parsing (V5.8 TB2) ──────────────────────────────────────

    #[test]
    fn claim_build_from_parsed_accepts_scalar_evidence_ref() {
        let mut diagnostics = Vec::new();
        let parsed = parsed_claim(
            BTreeMap::from([
                (STATUS_FIELD.to_string(), "plain".to_string()),
                (
                    EVIDENCE_REF_FIELD.to_string(),
                    "billing.consume-use-case".to_string(),
                ),
            ]),
            "body",
        );

        let claim = Claim::build_from_parsed(parsed, &mut diagnostics).expect("valid claim");

        assert!(diagnostics.is_empty(), "diagnostics: {diagnostics:?}");
        let refs = claim.evidence_refs();
        assert_eq!(refs.len(), 1);
        assert_eq!(
            refs[0].target_id().expect("ObjectRef has target").as_str(),
            "billing.consume-use-case"
        );
    }

    #[test]
    fn claim_build_from_parsed_accepts_comma_separated_evidence_refs() {
        let mut diagnostics = Vec::new();
        let parsed = parsed_claim(
            BTreeMap::from([
                (STATUS_FIELD.to_string(), "plain".to_string()),
                (
                    EVIDENCE_REF_FIELD.to_string(),
                    "billing.consume-use-case, billing.other-source".to_string(),
                ),
            ]),
            "body",
        );

        let claim = Claim::build_from_parsed(parsed, &mut diagnostics).expect("valid claim");

        assert!(diagnostics.is_empty(), "diagnostics: {diagnostics:?}");
        let refs = claim.evidence_refs();
        assert_eq!(refs.len(), 2);
        let strs: Vec<&str> = refs
            .iter()
            .filter_map(|ev| ev.target_id())
            .map(|id| id.as_str())
            .collect();
        assert!(
            strs.contains(&"billing.consume-use-case"),
            "must contain first ref"
        );
        assert!(
            strs.contains(&"billing.other-source"),
            "must contain second ref"
        );
    }

    #[test]
    fn claim_build_from_parsed_accepts_bracketed_evidence_ref_list() {
        let mut diagnostics = Vec::new();
        let parsed = parsed_claim(
            BTreeMap::from([
                (STATUS_FIELD.to_string(), "plain".to_string()),
                (
                    EVIDENCE_REF_FIELD.to_string(),
                    "[billing.consume-use-case, billing.other-source]".to_string(),
                ),
            ]),
            "body",
        );

        let claim = Claim::build_from_parsed(parsed, &mut diagnostics).expect("valid claim");

        assert!(diagnostics.is_empty(), "diagnostics: {diagnostics:?}");
        assert_eq!(claim.evidence_refs().len(), 2);
    }

    #[test]
    fn claim_build_from_parsed_deduplicates_evidence_refs() {
        let mut diagnostics = Vec::new();
        let parsed = parsed_claim(
            BTreeMap::from([
                (STATUS_FIELD.to_string(), "plain".to_string()),
                (
                    EVIDENCE_REF_FIELD.to_string(),
                    "billing.consume-use-case, billing.consume-use-case".to_string(),
                ),
            ]),
            "body",
        );

        let claim = Claim::build_from_parsed(parsed, &mut diagnostics).expect("valid claim");

        assert!(diagnostics.is_empty(), "diagnostics: {diagnostics:?}");
        assert_eq!(claim.evidence_refs().len(), 1);
    }

    #[test]
    fn claim_build_from_parsed_rejects_invalid_evidence_ref_id() {
        let mut diagnostics = Vec::new();
        let parsed = parsed_claim(
            BTreeMap::from([
                (STATUS_FIELD.to_string(), "plain".to_string()),
                (EVIDENCE_REF_FIELD.to_string(), "INVALID_ID".to_string()),
            ]),
            "body",
        );

        let claim = Claim::build_from_parsed(parsed, &mut diagnostics).expect("claim builds");

        // The claim still builds (invalid IDs are dropped), but a diagnostic is emitted.
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::IdInvalid);
        assert!(
            claim.evidence_refs().is_empty(),
            "invalid id must be dropped"
        );
    }

    #[test]
    fn claim_build_from_parsed_evidence_ref_not_in_optional_fields() {
        // evidence_ref: must be consumed and must NOT appear in generic fields.
        let mut diagnostics = Vec::new();
        let parsed = parsed_claim(
            BTreeMap::from([
                (STATUS_FIELD.to_string(), "plain".to_string()),
                (
                    EVIDENCE_REF_FIELD.to_string(),
                    "billing.consume-use-case".to_string(),
                ),
            ]),
            "body",
        );

        let claim = Claim::build_from_parsed(parsed, &mut diagnostics).expect("valid claim");

        assert!(diagnostics.is_empty(), "diagnostics: {diagnostics:?}");
        let field_keys: Vec<&str> = claim.fields().iter().map(|(k, _)| k.as_str()).collect();
        assert!(
            !field_keys.contains(&EVIDENCE_REF_FIELD),
            "evidence_ref must not appear in generic fields; got: {field_keys:?}"
        );
    }

    #[test]
    fn draft_claim_can_carry_evidence_refs() {
        // evidence_ref is valid on draft (non-verified) claims.
        let mut diagnostics = Vec::new();
        let parsed = parsed_claim(
            BTreeMap::from([
                (STATUS_FIELD.to_string(), "draft".to_string()),
                (
                    EVIDENCE_REF_FIELD.to_string(),
                    "billing.consume-use-case".to_string(),
                ),
            ]),
            "body",
        );

        let claim = Claim::build_from_parsed(parsed, &mut diagnostics).expect("valid claim");

        assert!(diagnostics.is_empty(), "diagnostics: {diagnostics:?}");
        assert_eq!(claim.evidence_refs().len(), 1);
    }

    // ── V5.8 TB4: evidence_ref counts toward verified-claim evidence ──────────

    #[test]
    fn verified_claim_with_only_evidence_ref_builds_successfully() {
        // A verified claim whose ONLY evidence is an `evidence_ref:` must build
        // without a ClaimVerifiedMissingEvidence diagnostic.
        let mut diagnostics = Vec::new();
        let parsed = parsed_claim(
            BTreeMap::from([
                (STATUS_FIELD.to_string(), VERIFIED_STATUS.to_string()),
                (OWNER_FIELD.to_string(), "team-billing".to_string()),
                (VERIFIED_AT_FIELD.to_string(), "2026-05-05".to_string()),
                (
                    EVIDENCE_REF_FIELD.to_string(),
                    "billing.consume-use-case".to_string(),
                ),
            ]),
            "body",
        );

        let claim = Claim::build_from_parsed(parsed, &mut diagnostics);

        assert!(
            claim.is_some(),
            "verified claim with only evidence_ref should build; diagnostics: {diagnostics:?}"
        );
        assert!(
            diagnostics.is_empty(),
            "no diagnostics expected; got: {diagnostics:?}"
        );
        let claim = claim.unwrap();
        // Inline evidence is empty — evidence comes from refs.
        assert!(
            claim
                .verification()
                .expect("has verification")
                .evidence()
                .is_empty(),
            "inline evidence should be empty when only refs are provided"
        );
        // The ref is present.
        assert_eq!(claim.evidence_refs().len(), 1);
    }

    #[test]
    fn verified_claim_with_no_evidence_and_no_refs_still_emits_missing_evidence() {
        // The missing-evidence diagnostic must still fire when neither inline
        // evidence nor evidence_ref is supplied.
        let mut diagnostics = Vec::new();
        let parsed = parsed_claim(
            BTreeMap::from([
                (STATUS_FIELD.to_string(), VERIFIED_STATUS.to_string()),
                (OWNER_FIELD.to_string(), "team-billing".to_string()),
                (VERIFIED_AT_FIELD.to_string(), "2026-05-05".to_string()),
            ]),
            "body",
        );

        let claim = Claim::build_from_parsed(parsed, &mut diagnostics);

        assert!(claim.is_none(), "claim should fail without any evidence");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::ClaimVerifiedMissingEvidence
        );
    }

    #[test]
    fn verified_claim_with_inline_test_and_evidence_ref_builds_successfully() {
        // Regression: a verified claim that has BOTH inline evidence AND an
        // evidence_ref must still build successfully.
        let mut diagnostics = Vec::new();
        let parsed = parsed_claim(
            BTreeMap::from([
                (STATUS_FIELD.to_string(), VERIFIED_STATUS.to_string()),
                (OWNER_FIELD.to_string(), "team-billing".to_string()),
                (VERIFIED_AT_FIELD.to_string(), "2026-05-05".to_string()),
                (TEST_FIELD.to_string(), "cargo test billing".to_string()),
                (
                    EVIDENCE_REF_FIELD.to_string(),
                    "billing.consume-use-case".to_string(),
                ),
            ]),
            "body",
        );

        let claim = Claim::build_from_parsed(parsed, &mut diagnostics);

        assert!(
            claim.is_some(),
            "verified claim with inline + ref evidence should build; diagnostics: {diagnostics:?}"
        );
        assert!(
            diagnostics.is_empty(),
            "no diagnostics expected; got: {diagnostics:?}"
        );
        let claim = claim.unwrap();
        // One inline evidence entry (test) and one ref.
        assert_eq!(
            claim
                .verification()
                .expect("has verification")
                .evidence()
                .len(),
            1,
            "inline evidence should contain the test entry"
        );
        assert_eq!(claim.evidence_refs().len(), 1);
    }
}
