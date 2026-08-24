//! Provider-neutral semantic assessment (`adoc.semantic_assessment.v0`, E3.2).
//!
//! Untrusted JSON becomes a typed assessment only through validation against
//! the exact [`SemanticContext`]. Provider invocation remains outside core.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use super::diagnostic::DiagnosticCode;
use super::identity::ObjectId;
use super::semantic_context::{
    CitationHandle, ExactRevision, SemanticContext, SemanticContextOutcome,
    is_semantic_context_text, is_sha256_digest,
};

pub const SEMANTIC_ASSESSMENT_SCHEMA_VERSION: &str = "adoc.semantic_assessment.v0";
pub const MATERIALITY_POLICY_VERSION: &str = "adoc.materiality.v0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticClassification {
    Consistent,
    ExtendsExistingKnowledge,
    ContradictsExistingKnowledge,
    InsufficientEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticMateriality {
    Material,
    Immaterial,
    Undetermined,
}

impl SemanticClassification {
    fn parse(value: &str) -> Result<Self, SemanticAssessmentError> {
        match value {
            "consistent" => Ok(Self::Consistent),
            "extends_existing_knowledge" => Ok(Self::ExtendsExistingKnowledge),
            "contradicts_existing_knowledge" => Ok(Self::ContradictsExistingKnowledge),
            "insufficient_evidence" => Ok(Self::InsufficientEvidence),
            _ => Err(SemanticAssessmentError::UnknownClassification {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticDisposition {
    NoChangeRequired,
    UpdateExisting,
    CreateKnowledge,
    NeedsHumanReview,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AffectedObject {
    pub object_id: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateUpdate {
    pub object_id: String,
    pub body: Option<String>,
    pub fields: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticExecutorIdentity {
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HumanReviewIndependence {
    SelfAssessment,
    Independent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HumanReview {
    pub authority: HumanReviewAuthority,
    pub reviewing_principal_id: String,
    pub requesting_principal_id: String,
    pub independence: HumanReviewIndependence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HumanReviewAuthority {
    SemanticReview,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticAssessmentScope {
    pub handle_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SemanticFinding {
    finding_id: String,
    classification: SemanticClassification,
    affected_objects: Vec<AffectedObject>,
    citations: Vec<String>,
    materiality: SemanticMateriality,
    proposed_disposition: SemanticDisposition,
    candidate_updates: Vec<CandidateUpdate>,
    unresolved_questions: Vec<String>,
    explanation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SemanticAssessment {
    schema_version: String,
    context_digest: String,
    base_revision: ExactRevision,
    head_revision: ExactRevision,
    identity: SemanticExecutorIdentity,
    #[serde(skip_serializing_if = "Option::is_none")]
    human_review: Option<HumanReview>,
    materiality_policy_version: String,
    scope: SemanticAssessmentScope,
    findings: Vec<SemanticFinding>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSemanticAssessment {
    schema_version: String,
    context_digest: String,
    base_revision: ExactRevision,
    head_revision: ExactRevision,
    identity: Option<RawSemanticExecutorIdentity>,
    human_review: Option<HumanReview>,
    materiality_policy_version: String,
    scope: SemanticAssessmentScope,
    findings: Vec<RawSemanticFinding>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSemanticExecutorIdentity {
    provider: Option<String>,
    model: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSemanticFinding {
    finding_id: String,
    classification: String,
    affected_objects: Vec<AffectedObject>,
    citations: Vec<String>,
    materiality: SemanticMateriality,
    proposed_disposition: SemanticDisposition,
    candidate_updates: Vec<RawCandidateUpdate>,
    unresolved_questions: Vec<String>,
    explanation: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCandidateUpdate {
    object_id: String,
    body: Value,
    fields: BTreeMap<String, Value>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SemanticAssessmentError {
    #[error("semantic assessment document is invalid: {message}")]
    InvalidDocument { message: String },
    #[error("unsupported semantic assessment version '{version}'")]
    UnsupportedVersion { version: String },
    #[error("semantic assessment provider and model identity are required")]
    IdentityMissing,
    #[error("unknown semantic assessment classification '{value}'")]
    UnknownClassification { value: String },
    #[error("semantic assessment revision identity does not match its exact context")]
    RevisionMismatch,
    #[error("semantic assessment citation is invalid: {message}")]
    CitationInvalid { message: String },
    #[error("semantic assessment serialization failed: {message}")]
    Serialization { message: String },
}

impl SemanticAssessmentError {
    pub fn diagnostic_code(&self) -> DiagnosticCode {
        match self {
            Self::UnsupportedVersion { .. } => DiagnosticCode::AssessmentSemanticVersionUnsupported,
            Self::IdentityMissing => DiagnosticCode::AssessmentSemanticIdentityMissing,
            Self::UnknownClassification { .. } => {
                DiagnosticCode::AssessmentSemanticClassificationUnknown
            }
            Self::RevisionMismatch => DiagnosticCode::AssessmentSemanticRevisionMismatch,
            Self::CitationInvalid { .. } => DiagnosticCode::AssessmentSemanticCitationInvalid,
            Self::InvalidDocument { .. } | Self::Serialization { .. } => {
                DiagnosticCode::AssessmentSemanticSchemaInvalid
            }
        }
    }
}

impl SemanticAssessment {
    pub fn to_canonical_json(&self) -> Result<String, SemanticAssessmentError> {
        let mut serialized = serde_json::to_string_pretty(self).map_err(|error| {
            SemanticAssessmentError::Serialization {
                message: error.to_string(),
            }
        })?;
        serialized.push('\n');
        Ok(serialized)
    }

    pub fn identity(&self) -> &SemanticExecutorIdentity {
        &self.identity
    }

    pub fn findings(&self) -> &[SemanticFinding] {
        &self.findings
    }

    pub fn human_review(&self) -> Option<&HumanReview> {
        self.human_review.as_ref()
    }

    pub fn allows_no_change_required(&self) -> bool {
        !self.findings.is_empty()
            && self.findings.iter().all(|finding| {
                finding.materiality == SemanticMateriality::Immaterial
                    && finding.proposed_disposition == SemanticDisposition::NoChangeRequired
            })
    }
}

impl SemanticFinding {
    pub fn classification(&self) -> SemanticClassification {
        self.classification
    }

    pub fn materiality(&self) -> SemanticMateriality {
        self.materiality
    }
}

pub fn validate_semantic_assessment(
    bytes: &[u8],
    context: &SemanticContext,
) -> Result<SemanticAssessment, SemanticAssessmentError> {
    let raw: RawSemanticAssessment = serde_json::from_slice(bytes).map_err(|error| {
        SemanticAssessmentError::InvalidDocument {
            message: error.to_string(),
        }
    })?;
    if raw.schema_version != SEMANTIC_ASSESSMENT_SCHEMA_VERSION {
        return Err(SemanticAssessmentError::UnsupportedVersion {
            version: raw.schema_version,
        });
    }
    if raw.materiality_policy_version != MATERIALITY_POLICY_VERSION {
        return Err(invalid(format!(
            "unsupported materiality policy version '{}'",
            raw.materiality_policy_version
        )));
    }
    if context.outcome() != SemanticContextOutcome::Ready {
        return Err(SemanticAssessmentError::CitationInvalid {
            message: "the supplied semantic context is not ready".to_string(),
        });
    }
    if raw.context_digest != context.context_digest() {
        return Err(SemanticAssessmentError::CitationInvalid {
            message: "context digest does not match".to_string(),
        });
    }
    if &raw.base_revision != context.base_revision()
        || &raw.head_revision != context.head_revision()
    {
        return Err(SemanticAssessmentError::RevisionMismatch);
    }

    let identity = raw
        .identity
        .ok_or(SemanticAssessmentError::IdentityMissing)?;
    let (Some(provider), Some(model)) = (identity.provider, identity.model) else {
        return Err(SemanticAssessmentError::IdentityMissing);
    };
    if !is_semantic_context_text(&provider) || !is_semantic_context_text(&model) {
        return Err(SemanticAssessmentError::IdentityMissing);
    }
    let human_review = match (provider.as_str(), raw.human_review) {
        ("human", Some(review)) => {
            require_text(
                "human_review.reviewing_principal_id",
                &review.reviewing_principal_id,
            )?;
            require_text(
                "human_review.requesting_principal_id",
                &review.requesting_principal_id,
            )?;
            let derived = if review.reviewing_principal_id == review.requesting_principal_id {
                HumanReviewIndependence::SelfAssessment
            } else {
                HumanReviewIndependence::Independent
            };
            if review.independence != derived {
                return Err(invalid(
                    "human_review independence does not match the recorded principals",
                ));
            }
            Some(review)
        }
        ("human", None) => {
            return Err(invalid(
                "human assessment requires reviewing principal and independence facts",
            ));
        }
        (_, Some(_)) => {
            return Err(invalid(
                "human_review facts are valid only for the human provider",
            ));
        }
        (_, None) => None,
    };
    if raw.findings.is_empty() {
        return Err(invalid(
            "semantic assessment must contain at least one finding",
        ));
    }

    let mut handle_ids = raw.scope.handle_ids;
    handle_ids.sort();
    reject_duplicates(&handle_ids, "assessment scope")?;
    for handle_id in &handle_ids {
        require_resolved_handle(context, handle_id)?;
    }
    let scoped_handles = handle_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();

    let mut finding_ids = BTreeSet::new();
    let mut findings = Vec::with_capacity(raw.findings.len());
    for raw_finding in raw.findings {
        require_text("finding_id", &raw_finding.finding_id)?;
        if !finding_ids.insert(raw_finding.finding_id.clone()) {
            return Err(invalid(format!(
                "duplicate finding id '{}'",
                raw_finding.finding_id
            )));
        }
        let classification = SemanticClassification::parse(&raw_finding.classification)?;

        let mut citations = raw_finding.citations;
        citations.sort();
        if citations.is_empty() {
            return Err(SemanticAssessmentError::CitationInvalid {
                message: format!("finding '{}' has no citations", raw_finding.finding_id),
            });
        }
        reject_duplicates(&citations, "finding citations")?;
        for citation in &citations {
            require_resolved_handle(context, citation)?;
            if !scoped_handles.contains(citation.as_str()) {
                return Err(SemanticAssessmentError::CitationInvalid {
                    message: format!("handle '{citation}' is outside the assessment scope"),
                });
            }
        }
        if !citations.iter().any(|citation| {
            matches!(
                context.citation_handle(citation),
                Some(CitationHandle::DiffHunk { .. })
            )
        }) {
            return Err(SemanticAssessmentError::CitationInvalid {
                message: format!(
                    "finding '{}' cites no deterministic diff hunk",
                    raw_finding.finding_id
                ),
            });
        }
        let materiality = match classification {
            SemanticClassification::Consistent => SemanticMateriality::Immaterial,
            SemanticClassification::ExtendsExistingKnowledge
            | SemanticClassification::ContradictsExistingKnowledge => SemanticMateriality::Material,
            SemanticClassification::InsufficientEvidence => SemanticMateriality::Undetermined,
        };
        if raw_finding.materiality != materiality {
            return Err(invalid(format!(
                "finding '{}' materiality does not match policy {}",
                raw_finding.finding_id, MATERIALITY_POLICY_VERSION
            )));
        }
        match raw_finding.proposed_disposition {
            SemanticDisposition::NoChangeRequired
                if materiality != SemanticMateriality::Immaterial =>
            {
                return Err(invalid(format!(
                    "finding '{}' cannot claim no_change_required with {:?} materiality",
                    raw_finding.finding_id, materiality
                )));
            }
            SemanticDisposition::UpdateExisting | SemanticDisposition::CreateKnowledge
                if materiality != SemanticMateriality::Material =>
            {
                return Err(invalid(format!(
                    "finding '{}' cannot propose a knowledge change with {:?} materiality",
                    raw_finding.finding_id, materiality
                )));
            }
            _ => {}
        }
        if raw_finding.proposed_disposition == SemanticDisposition::NoChangeRequired
            && (!raw_finding.candidate_updates.is_empty()
                || !raw_finding.unresolved_questions.is_empty())
        {
            return Err(invalid(format!(
                "finding '{}' cannot combine no_change_required with candidates or unresolved questions",
                raw_finding.finding_id
            )));
        }

        let mut affected_objects = raw_finding.affected_objects;
        affected_objects.sort();
        if affected_objects.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(invalid("duplicate affected object"));
        }
        for object in &affected_objects {
            ObjectId::new(&object.object_id)
                .map_err(|_| invalid(format!("invalid affected object '{}'", object.object_id)))?;
            if !is_sha256_digest(&object.content_hash) {
                return Err(invalid(format!(
                    "affected object '{}' has an invalid content hash",
                    object.object_id
                )));
            }
            let resolves = citations.iter().any(|citation| {
                matches!(
                    context.citation_handle(citation),
                    Some(CitationHandle::KnowledgeObject { object_id, semantic_hash })
                        if object_id == &object.object_id && semantic_hash == &object.content_hash
                )
            });
            if !resolves {
                return Err(SemanticAssessmentError::CitationInvalid {
                    message: format!(
                        "affected object '{}' does not resolve through a cited context handle",
                        object.object_id
                    ),
                });
            }
        }
        let affected_object_ids = affected_objects
            .iter()
            .map(|object| object.object_id.as_str())
            .collect::<BTreeSet<_>>();

        if raw_finding.proposed_disposition == SemanticDisposition::CreateKnowledge
            && !raw_finding.candidate_updates.is_empty()
        {
            return Err(SemanticAssessmentError::CitationInvalid {
                message: format!(
                    "finding '{}' cannot carry create candidates without a trusted creation scope",
                    raw_finding.finding_id
                ),
            });
        }
        let mut candidate_updates = raw_finding
            .candidate_updates
            .into_iter()
            .map(|update| {
                Ok(CandidateUpdate {
                    object_id: update.object_id,
                    body: match update.body {
                        Value::Null => None,
                        Value::String(body) => Some(body),
                        _ => return Err(invalid("candidate update body must be a string or null")),
                    },
                    fields: update.fields,
                })
            })
            .collect::<Result<Vec<_>, SemanticAssessmentError>>()?;
        candidate_updates.sort_by(|left, right| left.object_id.cmp(&right.object_id));
        if candidate_updates
            .windows(2)
            .any(|pair| pair[0].object_id == pair[1].object_id)
        {
            return Err(invalid(format!(
                "finding '{}' contains duplicate candidate object IDs",
                raw_finding.finding_id
            )));
        }
        for update in &candidate_updates {
            ObjectId::new(&update.object_id)
                .map_err(|_| invalid(format!("invalid candidate object '{}'", update.object_id)))?;
            if !affected_object_ids.contains(update.object_id.as_str()) {
                return Err(SemanticAssessmentError::CitationInvalid {
                    message: format!(
                        "candidate update '{}' is outside the cited affected object set",
                        update.object_id
                    ),
                });
            }
            if update
                .body
                .as_deref()
                .is_some_and(|body| body.trim().is_empty())
            {
                return Err(invalid(format!(
                    "candidate update '{}' body must be non-blank when present",
                    update.object_id
                )));
            }
            if update.body.is_none() && update.fields.is_empty() {
                return Err(invalid(format!(
                    "candidate update '{}' has no body or fields",
                    update.object_id
                )));
            }
        }
        for question in &raw_finding.unresolved_questions {
            require_text("unresolved_questions[]", question)?;
        }
        require_text("explanation", &raw_finding.explanation)?;

        findings.push(SemanticFinding {
            finding_id: raw_finding.finding_id,
            classification,
            affected_objects,
            citations,
            materiality,
            proposed_disposition: raw_finding.proposed_disposition,
            candidate_updates,
            unresolved_questions: raw_finding.unresolved_questions,
            explanation: raw_finding.explanation,
        });
    }
    findings.sort_by(|left, right| left.finding_id.cmp(&right.finding_id));

    Ok(SemanticAssessment {
        schema_version: SEMANTIC_ASSESSMENT_SCHEMA_VERSION.to_string(),
        context_digest: raw.context_digest,
        base_revision: raw.base_revision,
        head_revision: raw.head_revision,
        identity: SemanticExecutorIdentity { provider, model },
        human_review,
        materiality_policy_version: MATERIALITY_POLICY_VERSION.to_string(),
        scope: SemanticAssessmentScope { handle_ids },
        findings,
    })
}

fn require_resolved_handle(
    context: &SemanticContext,
    handle_id: &str,
) -> Result<(), SemanticAssessmentError> {
    require_text("citation handle", handle_id)?;
    if context.citation_handle(handle_id).is_some() {
        return Ok(());
    }
    Err(SemanticAssessmentError::CitationInvalid {
        message: format!("handle '{handle_id}' does not resolve"),
    })
}

fn reject_duplicates(values: &[String], field: &str) -> Result<(), SemanticAssessmentError> {
    if values.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(invalid(format!("{field} contains duplicates")));
    }
    Ok(())
}

fn require_text(field: &str, value: &str) -> Result<(), SemanticAssessmentError> {
    if is_semantic_context_text(value) {
        return Ok(());
    }
    Err(invalid(format!("{field} must be non-blank semantic text")))
}

fn invalid(message: impl Into<String>) -> SemanticAssessmentError {
    SemanticAssessmentError::InvalidDocument {
        message: message.into(),
    }
}
