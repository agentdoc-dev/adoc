//! Digest-bound semantic context (`adoc.semantic_context.v0`, E3.1).
//!
//! Construction and validation live in the domain: adapters may supply
//! bytes, but cannot bypass revision, digest, identity, or ordering rules.

use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use super::diagnostic::DiagnosticCode;
use super::hashing::sha256_prefixed;
use super::identity::ObjectId;

pub const SEMANTIC_CONTEXT_SCHEMA_VERSION: &str = "adoc.semantic_context.v0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExactRevision {
    pub system: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticContextBasis {
    pub assessment_digest: String,
    pub knowledge_basis: KnowledgeBasis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextRequirement {
    Required,
    Optional,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextClass {
    pub class_id: String,
    pub requirement: ContextRequirement,
    pub byte_budget: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnavailabilityReason {
    Permission,
    Retention,
    SourceOutage,
    Truncation,
    ResourceLimit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnavailabilityOutcome {
    Insufficient,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityPolicyRule {
    pub reason: UnavailabilityReason,
    pub outcome: UnavailabilityOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityPolicy {
    pub version: String,
    pub rules: Vec<CapabilityPolicyRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticContextSelection {
    pub algorithm: String,
    pub version: String,
    pub authorized_scope: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextUnavailabilityKind {
    Redaction,
    Omission,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextUnavailability {
    pub record_id: String,
    pub class_id: String,
    pub kind: ContextUnavailabilityKind,
    pub reason: UnavailabilityReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum KnowledgeBasis {
    GraphArtifact { digest: String },
    ManagedRevision { digest: String },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CitationHandle {
    KnowledgeObject {
        object_id: String,
        semantic_hash: String,
    },
    DiffHunk {
        changed_source_id: String,
        hunk_digest: String,
    },
    SourceAssertion {
        source_assertion_id: String,
        source_record_id: String,
    },
    SourceBinding {
        object_id: String,
    },
    Evidence {
        object_id: String,
        evidence_index: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticContextItem {
    pub handle_id: String,
    pub class_id: String,
    pub scope_ref: String,
    pub handle: CitationHandle,
    pub content: Value,
    pub truncated: bool,
}

#[derive(Debug, Clone)]
pub struct SemanticContextInput {
    pub evaluation_date: NaiveDate,
    pub subject_revision: ExactRevision,
    pub source_revision: ExactRevision,
    pub base_revision: ExactRevision,
    pub head_revision: ExactRevision,
    pub basis: SemanticContextBasis,
    pub selection: SemanticContextSelection,
    pub capability_policy: CapabilityPolicy,
    pub context_classes: Vec<ContextClass>,
    pub items: Vec<SemanticContextItem>,
    pub unavailability: Vec<ContextUnavailability>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphCitationObject {
    pub object_id: String,
    pub semantic_hash: String,
    pub has_source_binding: bool,
    pub evidence_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunkCitation {
    pub changed_source_id: String,
    pub hunk_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceAssertionCitation {
    pub source_assertion_id: String,
    pub source_record_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationContentProjection {
    pub handle: CitationHandle,
    pub class_id: String,
    pub scope_ref: String,
    pub content_digest: String,
    pub truncated_content_digests: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphObjectContextExpectation {
    pub object_id: String,
    pub class_id: String,
    pub scope_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticContextExpectedBindings {
    pub subject_revision: ExactRevision,
    pub source_revision: ExactRevision,
    pub base_revision: ExactRevision,
    pub head_revision: ExactRevision,
    pub assessment_digest: String,
    pub selection_algorithm: String,
    pub selection_version: String,
    pub context_classes: Vec<ContextClass>,
    pub authorized_scope: Vec<String>,
    pub capability_policy: CapabilityPolicy,
    pub graph_object_contexts: Vec<GraphObjectContextExpectation>,
}

#[derive(Debug, Clone)]
pub struct SemanticContextValidationBasis {
    pub evaluation_date: NaiveDate,
    pub subject_revision: ExactRevision,
    pub source_revision: ExactRevision,
    pub base_revision: ExactRevision,
    pub head_revision: ExactRevision,
    pub assessment_digest: String,
    pub selection_algorithm: String,
    pub selection_version: String,
    pub context_classes: Vec<ContextClass>,
    pub authorized_scope: Vec<String>,
    pub capability_policy: CapabilityPolicy,
    pub graph_artifact_digest: Option<String>,
    pub managed_revision_digest: Option<String>,
    pub graph_objects: Vec<GraphCitationObject>,
    pub diff_hunks: Vec<DiffHunkCitation>,
    pub source_assertions: Vec<SourceAssertionCitation>,
    pub citation_contents: Vec<CitationContentProjection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticContextOutcome {
    Ready,
    Insufficient,
    Failed,
}

impl SemanticContextOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Insufficient => "insufficient",
            Self::Failed => "failed",
        }
    }

    pub fn diagnostic_code(self) -> Option<DiagnosticCode> {
        match self {
            Self::Ready => None,
            Self::Insufficient => Some(DiagnosticCode::SemanticContextInsufficientContext),
            Self::Failed => Some(DiagnosticCode::SemanticContextFailed),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextCoverage {
    class_id: String,
    requirement: ContextRequirement,
    item_count: u64,
    included_bytes: u64,
    byte_budget: u64,
    truncated: bool,
    unavailable_count: u64,
    reasons: Vec<UnavailabilityReason>,
    complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SemanticContext {
    schema_version: String,
    evaluation_date: String,
    subject_revision: ExactRevision,
    source_revision: ExactRevision,
    base_revision: ExactRevision,
    head_revision: ExactRevision,
    basis: SemanticContextBasis,
    selection: SemanticContextSelection,
    capability_policy: CapabilityPolicy,
    context_classes: Vec<ContextClass>,
    items: Vec<SemanticContextItem>,
    unavailability: Vec<ContextUnavailability>,
    coverage: Vec<ContextCoverage>,
    outcome: SemanticContextOutcome,
    context_digest: String,
}

#[derive(Debug, Serialize)]
struct DigestInput<'a> {
    schema_version: &'a str,
    evaluation_date: &'a str,
    subject_revision: &'a ExactRevision,
    source_revision: &'a ExactRevision,
    base_revision: &'a ExactRevision,
    head_revision: &'a ExactRevision,
    basis: &'a SemanticContextBasis,
    selection: &'a SemanticContextSelection,
    capability_policy: &'a CapabilityPolicy,
    context_classes: &'a [ContextClass],
    items: &'a [SemanticContextItem],
    unavailability: &'a [ContextUnavailability],
    coverage: &'a [ContextCoverage],
    outcome: SemanticContextOutcome,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SemanticContextDocument {
    schema_version: String,
    evaluation_date: String,
    subject_revision: ExactRevision,
    source_revision: ExactRevision,
    base_revision: ExactRevision,
    head_revision: ExactRevision,
    basis: SemanticContextBasis,
    selection: SemanticContextSelection,
    capability_policy: CapabilityPolicy,
    context_classes: Vec<ContextClass>,
    items: Vec<SemanticContextItem>,
    unavailability: Vec<ContextUnavailability>,
    coverage: Vec<ContextCoverage>,
    outcome: SemanticContextOutcome,
    context_digest: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SemanticContextError {
    #[error("semantic context document is invalid: {message}")]
    InvalidDocument { message: String },
    #[error("unsupported semantic context version '{version}'")]
    UnsupportedVersion { version: String },
    #[error("{field} must be non-blank without surrounding whitespace or control characters")]
    InvalidText { field: String },
    #[error("{field} must be 'sha256:' plus 64 lowercase hex characters")]
    InvalidDigest { field: String },
    #[error("Knowledge Object handle '{handle_id}' contains an invalid Object ID")]
    InvalidObjectId { handle_id: String },
    #[error("semantic context contains duplicate handle id '{handle_id}'")]
    DuplicateHandleId { handle_id: String },
    #[error("semantic context contains duplicate class id '{class_id}'")]
    DuplicateClassId { class_id: String },
    #[error("semantic context contains duplicate authorized scope reference '{scope_ref}'")]
    DuplicateAuthorizedScope { scope_ref: String },
    #[error("semantic context item '{handle_id}' is outside authorized scope '{scope_ref}'")]
    UnauthorizedScope {
        handle_id: String,
        scope_ref: String,
    },
    #[error("semantic context item '{handle_id}' references unknown class '{class_id}'")]
    UnknownClass { handle_id: String, class_id: String },
    #[error(
        "semantic context unavailability record '{record_id}' references unknown class '{class_id}'"
    )]
    UnknownUnavailabilityClass { record_id: String, class_id: String },
    #[error("semantic context {field} must use canonical order")]
    NonCanonicalOrder { field: String },
    #[error("semantic context class '{class_id}' must have a positive byte budget")]
    InvalidByteBudget { class_id: String },
    #[error("semantic context class '{class_id}' exceeds its {byte_budget}-byte budget")]
    ByteBudgetExceeded { class_id: String, byte_budget: u64 },
    #[error("semantic context coverage or outcome does not match its items")]
    DerivedStateMismatch,
    #[error("capability policy must contain exactly one rule for every unavailability reason")]
    InvalidCapabilityPolicy,
    #[error("semantic context contains duplicate unavailability record id '{record_id}'")]
    DuplicateUnavailabilityId { record_id: String },
    #[error("semantic context digest does not match its canonical content")]
    DigestMismatch,
    #[error("semantic context evaluation date does not match the validation runtime")]
    EvaluationDateMismatch,
    #[error("semantic context basis does not match the supplied validation basis: {message}")]
    BasisMismatch { message: String },
    #[error("semantic context citation handle '{handle_id}' does not resolve in its exact basis")]
    UnresolvedCitation { handle_id: String },
    #[error("semantic context citation scope for handle '{handle_id}' does not match its basis")]
    CitationScopeMismatch { handle_id: String },
    #[error("semantic context citation class for handle '{handle_id}' does not match its basis")]
    CitationClassMismatch { handle_id: String },
    #[error("semantic context citation content for handle '{handle_id}' does not match its basis")]
    CitationContentMismatch { handle_id: String },
    #[error("semantic context serialization failed: {message}")]
    Serialization { message: String },
}

impl SemanticContextError {
    pub fn diagnostic_code(&self) -> DiagnosticCode {
        match self {
            Self::UnsupportedVersion { .. } => DiagnosticCode::SchemaUnsupportedVersion,
            Self::DigestMismatch => DiagnosticCode::SemanticContextDigestMismatch,
            Self::EvaluationDateMismatch
            | Self::BasisMismatch { .. }
            | Self::UnresolvedCitation { .. }
            | Self::CitationScopeMismatch { .. }
            | Self::CitationClassMismatch { .. }
            | Self::CitationContentMismatch { .. } => DiagnosticCode::SemanticContextBasisMismatch,
            Self::InvalidDocument { .. }
            | Self::InvalidText { .. }
            | Self::InvalidDigest { .. }
            | Self::InvalidObjectId { .. }
            | Self::DuplicateHandleId { .. }
            | Self::DuplicateClassId { .. }
            | Self::DuplicateAuthorizedScope { .. }
            | Self::UnauthorizedScope { .. }
            | Self::UnknownClass { .. }
            | Self::UnknownUnavailabilityClass { .. }
            | Self::NonCanonicalOrder { .. }
            | Self::InvalidByteBudget { .. }
            | Self::ByteBudgetExceeded { .. }
            | Self::DerivedStateMismatch
            | Self::InvalidCapabilityPolicy
            | Self::DuplicateUnavailabilityId { .. }
            | Self::Serialization { .. } => DiagnosticCode::SemanticContextInvalidDocument,
        }
    }
}

impl SemanticContext {
    pub fn to_canonical_json(&self) -> Result<String, SemanticContextError> {
        let mut serialized = serde_json::to_string_pretty(self).map_err(|error| {
            SemanticContextError::Serialization {
                message: error.to_string(),
            }
        })?;
        serialized.push('\n');
        Ok(serialized)
    }

    pub fn context_digest(&self) -> &str {
        &self.context_digest
    }

    pub fn outcome(&self) -> SemanticContextOutcome {
        self.outcome
    }

    pub fn allows_no_change_required(&self) -> bool {
        self.outcome == SemanticContextOutcome::Ready
    }
}

pub fn build_semantic_context(
    mut input: SemanticContextInput,
) -> Result<SemanticContext, SemanticContextError> {
    validate_revision("subject_revision", &input.subject_revision)?;
    validate_revision("source_revision", &input.source_revision)?;
    validate_revision("base_revision", &input.base_revision)?;
    validate_revision("head_revision", &input.head_revision)?;
    require_digest("basis.assessment_digest", &input.basis.assessment_digest)?;
    match &input.basis.knowledge_basis {
        KnowledgeBasis::GraphArtifact { digest } => {
            require_digest("basis.knowledge_basis.digest", digest)?;
        }
        KnowledgeBasis::ManagedRevision { digest } => {
            require_digest("basis.knowledge_basis.digest", digest)?;
        }
    }
    require_text("selection.algorithm", &input.selection.algorithm)?;
    require_text("selection.version", &input.selection.version)?;
    input.selection.authorized_scope.sort();
    for scope in &input.selection.authorized_scope {
        require_text("selection.authorized_scope[]", scope)?;
    }
    for duplicate in input.selection.authorized_scope.windows(2) {
        if duplicate[0] == duplicate[1] {
            return Err(SemanticContextError::DuplicateAuthorizedScope {
                scope_ref: duplicate[0].clone(),
            });
        }
    }
    let authorized_scope: BTreeSet<_> = input
        .selection
        .authorized_scope
        .iter()
        .map(String::as_str)
        .collect();
    input
        .capability_policy
        .rules
        .sort_by_key(|rule| rule.reason);
    if !is_valid_capability_policy(&input.capability_policy) {
        return Err(SemanticContextError::InvalidCapabilityPolicy);
    }
    let policy: BTreeMap<_, _> = input
        .capability_policy
        .rules
        .iter()
        .map(|rule| (rule.reason, rule.outcome))
        .collect();

    input
        .context_classes
        .sort_by(|left, right| left.class_id.cmp(&right.class_id));
    let mut coverage_by_class = BTreeMap::new();
    for class in &input.context_classes {
        require_text("context_classes[].class_id", &class.class_id)?;
        if class.byte_budget == 0 {
            return Err(SemanticContextError::InvalidByteBudget {
                class_id: class.class_id.clone(),
            });
        }
        let coverage = ContextCoverage {
            class_id: class.class_id.clone(),
            requirement: class.requirement,
            item_count: 0,
            included_bytes: 0,
            byte_budget: class.byte_budget,
            truncated: false,
            unavailable_count: 0,
            reasons: Vec::new(),
            complete: false,
        };
        if coverage_by_class
            .insert(class.class_id.clone(), coverage)
            .is_some()
        {
            return Err(SemanticContextError::DuplicateClassId {
                class_id: class.class_id.clone(),
            });
        }
    }

    input
        .items
        .sort_by(|left, right| left.handle_id.cmp(&right.handle_id));
    let mut handle_ids = BTreeSet::new();
    for item in &input.items {
        require_text("items[].handle_id", &item.handle_id)?;
        require_text("items[].class_id", &item.class_id)?;
        require_text("items[].scope_ref", &item.scope_ref)?;
        if !handle_ids.insert(item.handle_id.as_str()) {
            return Err(SemanticContextError::DuplicateHandleId {
                handle_id: item.handle_id.clone(),
            });
        }
        if !authorized_scope.contains(item.scope_ref.as_str()) {
            return Err(SemanticContextError::UnauthorizedScope {
                handle_id: item.handle_id.clone(),
                scope_ref: item.scope_ref.clone(),
            });
        }
        let coverage = coverage_by_class.get_mut(&item.class_id).ok_or_else(|| {
            SemanticContextError::UnknownClass {
                handle_id: item.handle_id.clone(),
                class_id: item.class_id.clone(),
            }
        })?;
        let content_bytes = serde_json::to_vec(&item.content).map_err(|error| {
            SemanticContextError::Serialization {
                message: error.to_string(),
            }
        })?;
        let content_len = u64::try_from(content_bytes.len()).map_err(|_| {
            SemanticContextError::ByteBudgetExceeded {
                class_id: item.class_id.clone(),
                byte_budget: coverage.byte_budget,
            }
        })?;
        coverage.included_bytes = coverage
            .included_bytes
            .checked_add(content_len)
            .ok_or_else(|| SemanticContextError::ByteBudgetExceeded {
                class_id: item.class_id.clone(),
                byte_budget: coverage.byte_budget,
            })?;
        coverage.item_count += 1;
        coverage.truncated |= item.truncated;
        if item.truncated {
            coverage.reasons.push(UnavailabilityReason::Truncation);
        }
        if coverage.included_bytes > coverage.byte_budget {
            return Err(SemanticContextError::ByteBudgetExceeded {
                class_id: item.class_id.clone(),
                byte_budget: coverage.byte_budget,
            });
        }
        match &item.handle {
            CitationHandle::KnowledgeObject {
                object_id,
                semantic_hash,
            } => {
                ObjectId::new(object_id).map_err(|_| SemanticContextError::InvalidObjectId {
                    handle_id: item.handle_id.clone(),
                })?;
                require_digest("items[].handle.semantic_hash", semantic_hash)?;
            }
            CitationHandle::DiffHunk {
                changed_source_id,
                hunk_digest,
            } => {
                require_text("items[].handle.changed_source_id", changed_source_id)?;
                require_digest("items[].handle.hunk_digest", hunk_digest)?;
            }
            CitationHandle::SourceAssertion {
                source_assertion_id,
                source_record_id,
            } => {
                require_text("items[].handle.source_assertion_id", source_assertion_id)?;
                require_text("items[].handle.source_record_id", source_record_id)?;
            }
            CitationHandle::SourceBinding { object_id }
            | CitationHandle::Evidence { object_id, .. } => {
                ObjectId::new(object_id).map_err(|_| SemanticContextError::InvalidObjectId {
                    handle_id: item.handle_id.clone(),
                })?;
            }
        }
    }
    input
        .unavailability
        .sort_by(|left, right| left.record_id.cmp(&right.record_id));
    let mut unavailability_ids = BTreeSet::new();
    for record in &input.unavailability {
        require_text("unavailability[].record_id", &record.record_id)?;
        require_text("unavailability[].class_id", &record.class_id)?;
        if !unavailability_ids.insert(record.record_id.as_str()) {
            return Err(SemanticContextError::DuplicateUnavailabilityId {
                record_id: record.record_id.clone(),
            });
        }
        let coverage = coverage_by_class.get_mut(&record.class_id).ok_or_else(|| {
            SemanticContextError::UnknownUnavailabilityClass {
                record_id: record.record_id.clone(),
                class_id: record.class_id.clone(),
            }
        })?;
        coverage.unavailable_count += 1;
        coverage.reasons.push(record.reason);
    }

    let mut coverage: Vec<_> = coverage_by_class.into_values().collect();
    for entry in &mut coverage {
        entry.reasons.sort();
        entry.reasons.dedup();
        entry.complete = entry.item_count > 0 && !entry.truncated && entry.unavailable_count == 0;
    }
    let mut outcome = if coverage.iter().any(|entry| {
        entry
            .reasons
            .iter()
            .any(|reason| policy.get(reason).copied() == Some(UnavailabilityOutcome::Failed))
    }) {
        SemanticContextOutcome::Failed
    } else {
        SemanticContextOutcome::Ready
    };
    if outcome != SemanticContextOutcome::Failed
        && coverage
            .iter()
            .any(|entry| entry.requirement == ContextRequirement::Required && !entry.complete)
    {
        outcome = SemanticContextOutcome::Insufficient;
    }

    let evaluation_date = input.evaluation_date.format("%Y-%m-%d").to_string();
    let digest_input = DigestInput {
        schema_version: SEMANTIC_CONTEXT_SCHEMA_VERSION,
        evaluation_date: &evaluation_date,
        subject_revision: &input.subject_revision,
        source_revision: &input.source_revision,
        base_revision: &input.base_revision,
        head_revision: &input.head_revision,
        basis: &input.basis,
        selection: &input.selection,
        capability_policy: &input.capability_policy,
        context_classes: &input.context_classes,
        items: &input.items,
        unavailability: &input.unavailability,
        coverage: &coverage,
        outcome,
    };
    let canonical =
        serde_json::to_vec(&digest_input).map_err(|error| SemanticContextError::Serialization {
            message: error.to_string(),
        })?;

    Ok(SemanticContext {
        schema_version: SEMANTIC_CONTEXT_SCHEMA_VERSION.to_string(),
        evaluation_date,
        subject_revision: input.subject_revision,
        source_revision: input.source_revision,
        base_revision: input.base_revision,
        head_revision: input.head_revision,
        basis: input.basis,
        selection: input.selection,
        capability_policy: input.capability_policy,
        context_classes: input.context_classes,
        items: input.items,
        unavailability: input.unavailability,
        coverage,
        outcome,
        context_digest: sha256_prefixed(&canonical),
    })
}

pub fn validate_semantic_context(
    bytes: &[u8],
    validation_basis: &SemanticContextValidationBasis,
) -> Result<SemanticContext, SemanticContextError> {
    let document: SemanticContextDocument =
        serde_json::from_slice(bytes).map_err(|error| SemanticContextError::InvalidDocument {
            message: error.to_string(),
        })?;
    if document.schema_version != SEMANTIC_CONTEXT_SCHEMA_VERSION {
        return Err(SemanticContextError::UnsupportedVersion {
            version: document.schema_version,
        });
    }
    require_canonical_order(
        "selection.authorized_scope",
        &document.selection.authorized_scope,
        |left, right| left <= right,
    )?;
    require_canonical_order(
        "capability_policy.rules",
        &document.capability_policy.rules,
        |left, right| left.reason <= right.reason,
    )?;
    require_canonical_order(
        "context_classes",
        &document.context_classes,
        |left, right| left.class_id <= right.class_id,
    )?;
    require_canonical_order("items", &document.items, |left, right| {
        left.handle_id <= right.handle_id
    })?;
    require_canonical_order("unavailability", &document.unavailability, |left, right| {
        left.record_id <= right.record_id
    })?;
    let evaluation_date = NaiveDate::parse_from_str(&document.evaluation_date, "%Y-%m-%d")
        .map_err(|_| SemanticContextError::InvalidText {
            field: "evaluation_date".to_string(),
        })?;
    let context = build_semantic_context(SemanticContextInput {
        evaluation_date,
        subject_revision: document.subject_revision,
        source_revision: document.source_revision,
        base_revision: document.base_revision,
        head_revision: document.head_revision,
        basis: document.basis,
        selection: document.selection,
        capability_policy: document.capability_policy,
        context_classes: document.context_classes,
        items: document.items,
        unavailability: document.unavailability,
    })?;
    if context.coverage != document.coverage || context.outcome != document.outcome {
        return Err(SemanticContextError::DerivedStateMismatch);
    }
    if context.context_digest != document.context_digest {
        return Err(SemanticContextError::DigestMismatch);
    }
    if context.evaluation_date
        != validation_basis
            .evaluation_date
            .format("%Y-%m-%d")
            .to_string()
    {
        return Err(SemanticContextError::EvaluationDateMismatch);
    }
    for (name, actual, expected) in [
        (
            "subject revision",
            &context.subject_revision,
            &validation_basis.subject_revision,
        ),
        (
            "source revision",
            &context.source_revision,
            &validation_basis.source_revision,
        ),
        (
            "base revision",
            &context.base_revision,
            &validation_basis.base_revision,
        ),
        (
            "head revision",
            &context.head_revision,
            &validation_basis.head_revision,
        ),
    ] {
        if actual != expected {
            return Err(SemanticContextError::BasisMismatch {
                message: format!("{name} differs"),
            });
        }
    }
    if context.basis.assessment_digest != validation_basis.assessment_digest {
        return Err(SemanticContextError::BasisMismatch {
            message: "assessment digest differs".to_string(),
        });
    }
    for (name, actual, expected) in [
        (
            "selection algorithm",
            context.selection.algorithm.as_str(),
            validation_basis.selection_algorithm.as_str(),
        ),
        (
            "selection version",
            context.selection.version.as_str(),
            validation_basis.selection_version.as_str(),
        ),
    ] {
        if actual != expected {
            return Err(SemanticContextError::BasisMismatch {
                message: format!("{name} differs"),
            });
        }
    }
    let mut expected_classes = BTreeMap::new();
    for class in &validation_basis.context_classes {
        if !is_semantic_context_text(&class.class_id) || class.byte_budget == 0 {
            return Err(SemanticContextError::BasisMismatch {
                message: "context-class basis is invalid".to_string(),
            });
        }
        if expected_classes
            .insert(
                class.class_id.as_str(),
                (class.requirement, class.byte_budget),
            )
            .is_some()
        {
            return Err(SemanticContextError::BasisMismatch {
                message: "context-class basis contains duplicates".to_string(),
            });
        }
    }
    let actual_classes: BTreeMap<_, _> = context
        .context_classes
        .iter()
        .map(|class| {
            (
                class.class_id.as_str(),
                (class.requirement, class.byte_budget),
            )
        })
        .collect();
    if actual_classes != expected_classes {
        return Err(SemanticContextError::BasisMismatch {
            message: "context classes differ".to_string(),
        });
    }
    let expected_scope: BTreeSet<_> = validation_basis
        .authorized_scope
        .iter()
        .map(String::as_str)
        .collect();
    if expected_scope.len() != validation_basis.authorized_scope.len() {
        return Err(SemanticContextError::BasisMismatch {
            message: "authorized scope basis contains duplicates".to_string(),
        });
    }
    if context
        .selection
        .authorized_scope
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
        != expected_scope
    {
        return Err(SemanticContextError::BasisMismatch {
            message: "authorized scope differs".to_string(),
        });
    }
    if !is_valid_capability_policy(&validation_basis.capability_policy) {
        return Err(SemanticContextError::BasisMismatch {
            message: "capability policy basis is invalid".to_string(),
        });
    }
    let expected_policy: BTreeMap<_, _> = validation_basis
        .capability_policy
        .rules
        .iter()
        .map(|rule| (rule.reason, rule.outcome))
        .collect();
    let actual_policy: BTreeMap<_, _> = context
        .capability_policy
        .rules
        .iter()
        .map(|rule| (rule.reason, rule.outcome))
        .collect();
    if context.capability_policy.version != validation_basis.capability_policy.version
        || actual_policy != expected_policy
    {
        return Err(SemanticContextError::BasisMismatch {
            message: "capability policy differs".to_string(),
        });
    }
    let (basis_name, expected_digest, digest) = match &context.basis.knowledge_basis {
        KnowledgeBasis::GraphArtifact { digest } => (
            "graph artifact",
            validation_basis.graph_artifact_digest.as_deref(),
            digest,
        ),
        KnowledgeBasis::ManagedRevision { digest } => (
            "managed revision",
            validation_basis.managed_revision_digest.as_deref(),
            digest,
        ),
    };
    if expected_digest != Some(digest.as_str()) {
        return Err(SemanticContextError::BasisMismatch {
            message: format!("{basis_name} digest differs"),
        });
    }

    let mut objects = BTreeMap::new();
    for object in &validation_basis.graph_objects {
        if objects.insert(object.object_id.as_str(), object).is_some() {
            return Err(SemanticContextError::BasisMismatch {
                message: "citation basis contains duplicate Object IDs".to_string(),
            });
        }
    }
    let diff_hunks: BTreeSet<_> = validation_basis
        .diff_hunks
        .iter()
        .map(|citation| {
            (
                citation.changed_source_id.as_str(),
                citation.hunk_digest.as_str(),
            )
        })
        .collect();
    if diff_hunks.len() != validation_basis.diff_hunks.len() {
        return Err(SemanticContextError::BasisMismatch {
            message: "citation basis contains duplicate diff hunks".to_string(),
        });
    }
    let source_assertions: BTreeSet<_> = validation_basis
        .source_assertions
        .iter()
        .map(|citation| {
            (
                citation.source_assertion_id.as_str(),
                citation.source_record_id.as_str(),
            )
        })
        .collect();
    if source_assertions.len() != validation_basis.source_assertions.len() {
        return Err(SemanticContextError::BasisMismatch {
            message: "citation basis contains duplicate source assertions".to_string(),
        });
    }
    let mut citation_contents = BTreeMap::new();
    for projection in &validation_basis.citation_contents {
        let truncated_digests: BTreeSet<_> = projection
            .truncated_content_digests
            .iter()
            .map(String::as_str)
            .collect();
        if !is_semantic_context_text(&projection.class_id)
            || !is_semantic_context_text(&projection.scope_ref)
            || !is_sha256_digest(&projection.content_digest)
            || truncated_digests.len() != projection.truncated_content_digests.len()
            || !truncated_digests
                .iter()
                .all(|digest| is_sha256_digest(digest))
        {
            return Err(SemanticContextError::BasisMismatch {
                message: "citation content projection is invalid".to_string(),
            });
        }
        if citation_contents
            .insert(&projection.handle, projection)
            .is_some()
        {
            return Err(SemanticContextError::BasisMismatch {
                message: "citation content projection contains duplicate handles".to_string(),
            });
        }
    }
    for item in &context.items {
        let resolves = match &item.handle {
            CitationHandle::KnowledgeObject {
                object_id,
                semantic_hash,
            } => objects
                .get(object_id.as_str())
                .is_some_and(|object| object.semantic_hash == *semantic_hash),
            CitationHandle::SourceBinding { object_id } => objects
                .get(object_id.as_str())
                .is_some_and(|object| object.has_source_binding),
            CitationHandle::Evidence {
                object_id,
                evidence_index,
            } => objects
                .get(object_id.as_str())
                .is_some_and(|object| (*evidence_index as usize) < object.evidence_count),
            CitationHandle::DiffHunk {
                changed_source_id,
                hunk_digest,
            } => diff_hunks.contains(&(changed_source_id.as_str(), hunk_digest.as_str())),
            CitationHandle::SourceAssertion {
                source_assertion_id,
                source_record_id,
            } => source_assertions
                .contains(&(source_assertion_id.as_str(), source_record_id.as_str())),
        };
        if !resolves {
            return Err(SemanticContextError::UnresolvedCitation {
                handle_id: item.handle_id.clone(),
            });
        }
        let projection = citation_contents
            .get(&item.handle)
            .copied()
            .ok_or_else(|| SemanticContextError::CitationContentMismatch {
                handle_id: item.handle_id.clone(),
            })?;
        if projection.scope_ref != item.scope_ref {
            return Err(SemanticContextError::CitationScopeMismatch {
                handle_id: item.handle_id.clone(),
            });
        }
        if projection.class_id != item.class_id {
            return Err(SemanticContextError::CitationClassMismatch {
                handle_id: item.handle_id.clone(),
            });
        }
        let content_digest = semantic_context_content_digest(&item.content);
        let content_resolves = if item.truncated {
            projection
                .truncated_content_digests
                .iter()
                .any(|digest| digest == &content_digest)
        } else {
            projection.content_digest == content_digest
        };
        if !content_resolves {
            return Err(SemanticContextError::CitationContentMismatch {
                handle_id: item.handle_id.clone(),
            });
        }
    }
    Ok(context)
}

fn require_canonical_order<T>(
    field: &str,
    values: &[T],
    in_order: impl Fn(&T, &T) -> bool,
) -> Result<(), SemanticContextError> {
    if values.windows(2).all(|pair| in_order(&pair[0], &pair[1])) {
        return Ok(());
    }
    Err(SemanticContextError::NonCanonicalOrder {
        field: field.to_string(),
    })
}

fn validate_revision(field: &str, revision: &ExactRevision) -> Result<(), SemanticContextError> {
    require_text(&format!("{field}.system"), &revision.system)?;
    require_text(&format!("{field}.value"), &revision.value)
}

fn require_text(field: &str, value: &str) -> Result<(), SemanticContextError> {
    if !is_semantic_context_text(value) {
        return Err(SemanticContextError::InvalidText {
            field: field.to_string(),
        });
    }
    Ok(())
}

fn require_digest(field: &str, digest: &str) -> Result<(), SemanticContextError> {
    if is_sha256_digest(digest) {
        return Ok(());
    }
    Err(SemanticContextError::InvalidDigest {
        field: field.to_string(),
    })
}

pub fn is_semantic_context_text(value: &str) -> bool {
    !value.is_empty() && value.trim() == value && !value.chars().any(char::is_control)
}

pub fn is_sha256_digest(value: &str) -> bool {
    let hex = value.strip_prefix("sha256:").unwrap_or_default();
    hex.len() == 64
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub fn is_valid_capability_policy(policy: &CapabilityPolicy) -> bool {
    let reasons: BTreeSet<_> = policy.rules.iter().map(|rule| rule.reason).collect();
    is_semantic_context_text(&policy.version)
        && policy.rules.len() == 5
        && reasons
            == BTreeSet::from([
                UnavailabilityReason::Permission,
                UnavailabilityReason::Retention,
                UnavailabilityReason::SourceOutage,
                UnavailabilityReason::Truncation,
                UnavailabilityReason::ResourceLimit,
            ])
}

pub fn semantic_context_content_digest(content: &Value) -> String {
    let bytes = serde_json::to_vec(content)
        .unwrap_or_else(|_| unreachable!("serde_json::Value serialization is infallible"));
    sha256_prefixed(&bytes)
}
