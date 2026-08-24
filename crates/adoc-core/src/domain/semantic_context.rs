//! Digest-bound semantic context (`adoc.semantic_context.v0`, E3.1).
//!
//! Construction and validation live in the domain: adapters may supply
//! bytes, but cannot bypass revision, digest, identity, or ordering rules.

use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

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
    pub context_classes: Vec<ContextClass>,
    pub items: Vec<SemanticContextItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticContextOutcome {
    Ready,
    Insufficient,
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
    context_classes: Vec<ContextClass>,
    items: Vec<SemanticContextItem>,
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
    context_classes: &'a [ContextClass],
    items: &'a [SemanticContextItem],
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
    context_classes: Vec<ContextClass>,
    items: Vec<SemanticContextItem>,
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
    #[error("semantic context item '{handle_id}' references unknown class '{class_id}'")]
    UnknownClass { handle_id: String, class_id: String },
    #[error("semantic context class '{class_id}' must have a positive byte budget")]
    InvalidByteBudget { class_id: String },
    #[error("semantic context class '{class_id}' exceeds its {byte_budget}-byte budget")]
    ByteBudgetExceeded { class_id: String, byte_budget: u64 },
    #[error("semantic context coverage or outcome does not match its items")]
    DerivedStateMismatch,
    #[error("semantic context digest does not match its canonical content")]
    DigestMismatch,
    #[error("semantic context serialization failed: {message}")]
    Serialization { message: String },
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
        if !handle_ids.insert(item.handle_id.as_str()) {
            return Err(SemanticContextError::DuplicateHandleId {
                handle_id: item.handle_id.clone(),
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

    let mut coverage: Vec<_> = coverage_by_class.into_values().collect();
    for entry in &mut coverage {
        entry.complete = entry.item_count > 0 && !entry.truncated;
    }
    let outcome = if coverage
        .iter()
        .any(|entry| entry.requirement == ContextRequirement::Required && !entry.complete)
    {
        SemanticContextOutcome::Insufficient
    } else {
        SemanticContextOutcome::Ready
    };

    let evaluation_date = input.evaluation_date.format("%Y-%m-%d").to_string();
    let digest_input = DigestInput {
        schema_version: SEMANTIC_CONTEXT_SCHEMA_VERSION,
        evaluation_date: &evaluation_date,
        subject_revision: &input.subject_revision,
        source_revision: &input.source_revision,
        base_revision: &input.base_revision,
        head_revision: &input.head_revision,
        basis: &input.basis,
        context_classes: &input.context_classes,
        items: &input.items,
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
        context_classes: input.context_classes,
        items: input.items,
        coverage,
        outcome,
        context_digest: sha256_prefixed(&canonical),
    })
}

pub fn validate_semantic_context(bytes: &[u8]) -> Result<SemanticContext, SemanticContextError> {
    let document: SemanticContextDocument =
        serde_json::from_slice(bytes).map_err(|error| SemanticContextError::InvalidDocument {
            message: error.to_string(),
        })?;
    if document.schema_version != SEMANTIC_CONTEXT_SCHEMA_VERSION {
        return Err(SemanticContextError::UnsupportedVersion {
            version: document.schema_version,
        });
    }
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
        context_classes: document.context_classes,
        items: document.items,
    })?;
    if context.coverage != document.coverage || context.outcome != document.outcome {
        return Err(SemanticContextError::DerivedStateMismatch);
    }
    if context.context_digest != document.context_digest {
        return Err(SemanticContextError::DigestMismatch);
    }
    Ok(context)
}

fn validate_revision(field: &str, revision: &ExactRevision) -> Result<(), SemanticContextError> {
    require_text(&format!("{field}.system"), &revision.system)?;
    require_text(&format!("{field}.value"), &revision.value)
}

fn require_text(field: &str, value: &str) -> Result<(), SemanticContextError> {
    if value.is_empty() || value.trim() != value || value.chars().any(char::is_control) {
        return Err(SemanticContextError::InvalidText {
            field: field.to_string(),
        });
    }
    Ok(())
}

fn require_digest(field: &str, digest: &str) -> Result<(), SemanticContextError> {
    let suffix = digest.strip_prefix("sha256:").unwrap_or_default();
    if suffix.len() == 64
        && suffix
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Ok(());
    }
    Err(SemanticContextError::InvalidDigest {
        field: field.to_string(),
    })
}
