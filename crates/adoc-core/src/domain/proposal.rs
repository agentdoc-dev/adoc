//! Canonical proposal record (`adoc.proposal.v0`, E5.1; ADR-0053 §8,
//! ADR-0054, ADR-0062).
//!
//! A proposal record binds a validated set of `adoc.patch.v0` patches to the
//! exact source revisions, deterministic assessment, semantic context, and
//! semantic assessment that produced them, plus the exact `content_hash` of
//! every existing Knowledge Object it edits. Its identity is the
//! proposal-set digest over the exact sorted patch bytes: any byte change to
//! any patch mints a new record, and a record superseding another names the
//! prior digest so the invalidation consequence is visible before submission.
//!
//! The record carries no branch names, titles, or wall-clock fields, so a
//! Git-delivered proposal and an API-submitted one produce byte-identical
//! records. Fields are private and the type has no `Deserialize`: a record
//! with any binding missing is unconstructible, and bytes become a record only
//! through the application validator that re-derives every digest.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use super::diagnostic::DiagnosticCode;
use super::hashing::sha256_prefixed;
use super::patch::{PatchDocument, PatchIntent, PatchOperation};
use super::semantic_context::{ExactRevision, is_semantic_context_text, is_sha256_digest};

pub const PROPOSAL_SCHEMA_VERSION: &str = "adoc.proposal.v0";

/// ADR-0053 §2: the only kind/status pairs a proposal may create.
const CREATE_FLOORS: [(&str, &str); 4] = [
    ("claim", "draft"),
    ("decision", "proposed"),
    ("api", "draft"),
    ("task", "open"),
];
/// ADR-0054 §3: an update leaves the object at a reviewable lifecycle, and
/// (ADR-0062 §6) must say so — the record cannot see the current one.
const REVIEWABLE_STATUSES: [&str; 3] = ["draft", "proposed", "open"];
/// ADR-0053 §3: fields that mint authority and never come from a proposal.
const AUTHORITY_FIELDS: [&str; 5] = [
    "verified_at",
    "reviewed_by",
    "approved_by",
    "decided_by",
    "resolved_by",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposalChangeRequest {
    pub system: String,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposalBindings {
    pub base_revision: ExactRevision,
    pub head_revision: ExactRevision,
    pub change_request: ProposalChangeRequest,
    pub assessment_digest: String,
    pub semantic_context_digest: String,
    pub semantic_assessment_digest: String,
}

/// One producer-supplied patch: its finding correlation, exact-head placement,
/// and the exact `adoc.patch.v0` bytes (sorted compact JSON plus one newline).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposalPatchInput {
    pub finding_id: String,
    pub placement_path: String,
    pub page_id: String,
    pub patch_bytes: Vec<u8>,
}

/// A patch input whose bytes the application layer already parsed.
pub(crate) struct ParsedProposalPatch {
    pub(crate) input: ProposalPatchInput,
    pub(crate) document: PatchDocument,
    pub(crate) patch: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProposalPatch {
    finding_id: String,
    placement_path: String,
    page_id: String,
    target: String,
    operation: PatchOperation,
    patch_digest: String,
    patch: Value,
}

impl ProposalPatch {
    pub fn finding_id(&self) -> &str {
        &self.finding_id
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn operation(&self) -> PatchOperation {
        self.operation
    }

    pub fn patch_digest(&self) -> &str {
        &self.patch_digest
    }

    /// The exact patch bytes the digest covers (ADR-0053 §8).
    pub fn patch_bytes(&self) -> Result<Vec<u8>, ProposalRecordError> {
        canonical_patch_bytes(&self.patch)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct ContentBinding {
    object_id: String,
    content_hash: String,
}

impl ContentBinding {
    pub fn object_id(&self) -> &str {
        &self.object_id
    }

    pub fn content_hash(&self) -> &str {
        &self.content_hash
    }
}

/// Constructible only by [`crate::build_proposal_record`] or
/// [`crate::validate_proposal_record`]; a record with any binding missing has
/// no representation:
///
/// ```compile_fail
/// // Private fields (E0451): no literal construction outside adoc-core. All
/// // six fields are named so only field privacy can fail this.
/// let record = adoc_core::ProposalRecord {
///     schema_version: todo!(),
///     proposal_set_digest: todo!(),
///     supersedes: todo!(),
///     bindings: todo!(),
///     content_bindings: todo!(),
///     patches: todo!(),
/// };
/// ```
///
/// ```compile_fail
/// // No `Deserialize` (E0277): wire bytes never become a record without the
/// // validator re-deriving every digest.
/// let record: adoc_core::ProposalRecord = serde_json::from_str("{}").unwrap();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProposalRecord {
    schema_version: String,
    proposal_set_digest: String,
    supersedes: Option<String>,
    bindings: ProposalBindings,
    content_bindings: Vec<ContentBinding>,
    patches: Vec<ProposalPatch>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProposalRecordError {
    #[error("proposal record document is invalid: {message}")]
    InvalidDocument { message: String },
    #[error("unsupported proposal record version '{version}'")]
    UnsupportedVersion { version: String },
    #[error("proposal record binding '{field}' is missing or invalid")]
    BindingInvalid { field: String },
    #[error("proposal patch is invalid: {message}")]
    PatchInvalid { message: String },
    #[error("proposal patch for '{target}' would mint authority: {reason}")]
    AuthorityRejected { target: String, reason: String },
    #[error(
        "proposal revision changes no patch byte: it would supersede its own digest {proposal_set_digest}"
    )]
    RevisionUnchanged { proposal_set_digest: String },
}

impl ProposalRecordError {
    pub fn diagnostic_code(&self) -> DiagnosticCode {
        match self {
            Self::InvalidDocument { .. } | Self::UnsupportedVersion { .. } => {
                DiagnosticCode::ProposalRecordInvalidDocument
            }
            Self::BindingInvalid { .. } => DiagnosticCode::ProposalRecordBindingInvalid,
            Self::PatchInvalid { .. } => DiagnosticCode::ProposalRecordPatchInvalid,
            Self::AuthorityRejected { .. } => DiagnosticCode::ProposalRecordAuthorityRejected,
            Self::RevisionUnchanged { .. } => DiagnosticCode::ProposalRecordRevisionUnchanged,
        }
    }
}

impl ProposalRecord {
    pub(crate) fn assemble(
        bindings: ProposalBindings,
        patches: Vec<ParsedProposalPatch>,
        supersedes: Option<String>,
    ) -> Result<Self, ProposalRecordError> {
        validate_bindings(&bindings)?;
        if patches.is_empty() {
            return Err(ProposalRecordError::PatchInvalid {
                message: "a proposal record needs at least one patch".to_string(),
            });
        }
        let mut sequences = BTreeMap::new();
        let mut patches = patches
            .into_iter()
            .map(|patch| assemble_patch(patch, &mut sequences))
            .collect::<Result<Vec<_>, _>>()?;
        let content_bindings = sequences
            .into_iter()
            .map(|(object_id, sequence)| {
                let content_hash = sequence.head_hash(&object_id)?;
                Ok(ContentBinding {
                    object_id,
                    content_hash,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        // Identity is placement-blind (E1.1, MILESTONES §E5.1 acceptance):
        // patches order by their digest alone, so a source-placement move
        // that would reorder a placement-sorted set cannot mint a version.
        patches.sort_by(|left, right| left.patch_digest.cmp(&right.patch_digest));
        let digests: Vec<&str> = patches
            .iter()
            .map(|patch| patch.patch_digest.as_str())
            .collect();
        if digests.iter().collect::<BTreeSet<_>>().len() != digests.len() {
            return Err(ProposalRecordError::PatchInvalid {
                message: "duplicate patch bytes in one proposal set".to_string(),
            });
        }
        let proposal_set_digest = proposal_set_digest(&digests)?;
        if let Some(prior) = &supersedes {
            if !is_sha256_digest(prior) {
                return Err(ProposalRecordError::BindingInvalid {
                    field: "supersedes".to_string(),
                });
            }
            if *prior == proposal_set_digest {
                return Err(ProposalRecordError::RevisionUnchanged {
                    proposal_set_digest,
                });
            }
        }
        Ok(Self {
            schema_version: PROPOSAL_SCHEMA_VERSION.to_string(),
            proposal_set_digest,
            supersedes,
            bindings,
            content_bindings,
            patches,
        })
    }

    pub fn proposal_set_digest(&self) -> &str {
        &self.proposal_set_digest
    }

    /// The prior proposal-set digest this record replaces: its approvals no
    /// longer bind once this record is submitted.
    pub fn supersedes(&self) -> Option<&str> {
        self.supersedes.as_deref()
    }

    pub fn bindings(&self) -> &ProposalBindings {
        &self.bindings
    }

    pub fn patches(&self) -> &[ProposalPatch] {
        &self.patches
    }

    pub fn content_bindings(&self) -> &[ContentBinding] {
        &self.content_bindings
    }

    pub fn to_canonical_json(&self) -> Result<String, ProposalRecordError> {
        let mut json = serde_json::to_string_pretty(self).map_err(|error| {
            ProposalRecordError::InvalidDocument {
                message: error.to_string(),
            }
        })?;
        json.push('\n');
        Ok(json)
    }
}

fn validate_bindings(bindings: &ProposalBindings) -> Result<(), ProposalRecordError> {
    let texts = [
        ("base_revision.system", &bindings.base_revision.system),
        ("base_revision.value", &bindings.base_revision.value),
        ("head_revision.system", &bindings.head_revision.system),
        ("head_revision.value", &bindings.head_revision.value),
        ("change_request.system", &bindings.change_request.system),
        ("change_request.id", &bindings.change_request.id),
    ];
    for (field, value) in texts {
        if !is_semantic_context_text(value) {
            return Err(binding_invalid(field));
        }
    }
    let digests = [
        ("assessment_digest", &bindings.assessment_digest),
        ("semantic_context_digest", &bindings.semantic_context_digest),
        (
            "semantic_assessment_digest",
            &bindings.semantic_assessment_digest,
        ),
    ];
    for (field, value) in digests {
        if !is_sha256_digest(value) {
            return Err(binding_invalid(field));
        }
    }
    Ok(())
}

fn binding_invalid(field: &str) -> ProposalRecordError {
    ProposalRecordError::BindingInvalid {
        field: field.to_string(),
    }
}

/// ADR-0054 §5: one logical update of an existing object is at most one
/// `update_fields` followed by at most one `replace_body`. Each patch binds
/// the object's hash at its point in the sequence, so the body patch carries
/// the hash re-derived after the field patch (PRD §51.5); the record binds
/// the exact-head hash the first patch carries. Application order is fixed
/// by the operations, not by the digest-ordered record.
#[derive(Default)]
struct TargetSequence {
    update_fields: Option<String>,
    replace_body: Option<String>,
    /// The record cannot see the object's current lifecycle, so the edit
    /// itself must carry the ADR-0054 §3 downgrade: an `update_fields`
    /// setting a reviewable status.
    sets_reviewable_status: bool,
}

impl TargetSequence {
    fn bind(&mut self, intent: &PatchIntent, target: &str) -> Result<(), ProposalRecordError> {
        let Some(base_hash) = intent.base_hash() else {
            return Ok(());
        };
        if !is_sha256_digest(base_hash) {
            return Err(ProposalRecordError::PatchInvalid {
                message: format!("patch base_hash for '{target}' is not a sha256 digest"),
            });
        }
        let slot = match intent {
            PatchIntent::UpdateFields { fields, .. } => {
                // enforce_floors already rejected any non-reviewable status.
                self.sets_reviewable_status |= fields.contains_key("status");
                &mut self.update_fields
            }
            PatchIntent::ReplaceBody { .. } => &mut self.replace_body,
            // Governance operations never reach a sequence (enforce_floors).
            _ => return Ok(()),
        };
        if slot.is_some() {
            return Err(ProposalRecordError::PatchInvalid {
                message: format!(
                    "more than one {} patch for '{target}' in one proposal set",
                    intent.operation().as_str()
                ),
            });
        }
        *slot = Some(base_hash.to_string());
        Ok(())
    }

    fn head_hash(self, target: &str) -> Result<String, ProposalRecordError> {
        if !self.sets_reviewable_status {
            return Err(ProposalRecordError::AuthorityRejected {
                target: target.to_string(),
                reason: "an existing-object edit must set a reviewable status in update_fields"
                    .to_string(),
            });
        }
        match (self.update_fields, self.replace_body) {
            (Some(first), Some(second)) if first == second => {
                Err(ProposalRecordError::PatchInvalid {
                    message: format!(
                        "replace_body for '{target}' must bind the content hash re-derived after update_fields, not the same base_hash"
                    ),
                })
            }
            (Some(first), _) | (None, Some(first)) => Ok(first),
            (None, None) => Err(ProposalRecordError::PatchInvalid {
                message: format!("no content hash bound for '{target}'"),
            }),
        }
    }
}

fn assemble_patch(
    parsed: ParsedProposalPatch,
    sequences: &mut BTreeMap<String, TargetSequence>,
) -> Result<ProposalPatch, ProposalRecordError> {
    let ParsedProposalPatch {
        input,
        document,
        patch,
    } = parsed;
    for (field, value) in [
        ("finding_id", &input.finding_id),
        ("placement_path", &input.placement_path),
        ("page_id", &input.page_id),
    ] {
        if !is_semantic_context_text(value) {
            return Err(ProposalRecordError::PatchInvalid {
                message: format!("patch {field} is missing or invalid"),
            });
        }
    }
    if canonical_patch_bytes(&patch)? != input.patch_bytes {
        return Err(ProposalRecordError::PatchInvalid {
            message: format!(
                "patch bytes for '{}' are not sorted compact JSON with one trailing newline",
                document.target
            ),
        });
    }
    enforce_floors(&document)?;
    if document.intent.base_hash().is_some() {
        sequences
            .entry(document.target.clone())
            .or_default()
            .bind(&document.intent, &document.target)?;
    }
    Ok(ProposalPatch {
        finding_id: input.finding_id,
        placement_path: input.placement_path,
        page_id: input.page_id,
        patch_digest: sha256_prefixed(&input.patch_bytes),
        target: document.target,
        operation: document.intent.operation(),
        // Explicitly key-sorted so record bytes never depend on the
        // producer's key order or on serde_json's map implementation.
        patch: canonicalize_object_keys(patch),
    })
}

fn enforce_floors(document: &PatchDocument) -> Result<(), ProposalRecordError> {
    let reject = |reason: String| ProposalRecordError::AuthorityRejected {
        target: document.target.clone(),
        reason,
    };
    let fields = match &document.intent {
        PatchIntent::CreateObject {
            kind,
            status,
            fields,
            ..
        } => {
            let status = status.as_deref().unwrap_or_default();
            if !CREATE_FLOORS.contains(&(kind.as_str(), status)) {
                return Err(reject(format!(
                    "create_object {kind}/{status} is outside the create-only floors"
                )));
            }
            fields
        }
        PatchIntent::UpdateFields { fields, .. } => {
            if let Some(status) = fields.get("status")
                && !REVIEWABLE_STATUSES.contains(&status.as_str())
            {
                return Err(reject(format!(
                    "update_fields status '{status}' is not a reviewable lifecycle"
                )));
            }
            fields
        }
        PatchIntent::ReplaceBody { .. } => return Ok(()),
        PatchIntent::Supersede { .. } | PatchIntent::Revoke { .. } => {
            return Err(reject(format!(
                "operation {} changes governance state",
                document.intent.operation().as_str()
            )));
        }
    };
    if let Some(field) = AUTHORITY_FIELDS
        .iter()
        .find(|field| fields.contains_key(**field))
    {
        return Err(reject(format!("field '{field}' carries authority")));
    }
    Ok(())
}

impl PatchIntent {
    fn base_hash(&self) -> Option<&str> {
        match self {
            Self::ReplaceBody { base_hash, .. }
            | Self::UpdateFields { base_hash, .. }
            | Self::Supersede { base_hash, .. }
            | Self::Revoke { base_hash } => Some(base_hash),
            Self::CreateObject { .. } => None,
        }
    }
}

/// ADR-0053 §8: sorted compact JSON plus one trailing newline.
pub(crate) fn canonical_patch_bytes(patch: &Value) -> Result<Vec<u8>, ProposalRecordError> {
    let mut bytes =
        serde_json::to_vec(&canonicalize_object_keys(patch.clone())).map_err(|error| {
            ProposalRecordError::PatchInvalid {
                message: error.to_string(),
            }
        })?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// ADR-0053 §8: the digest of the compact JSON array of ordered patch
/// digests plus one trailing newline.
pub(crate) fn proposal_set_digest(digests: &[&str]) -> Result<String, ProposalRecordError> {
    let mut bytes =
        serde_json::to_vec(digests).map_err(|error| ProposalRecordError::InvalidDocument {
            message: error.to_string(),
        })?;
    bytes.push(b'\n');
    Ok(sha256_prefixed(&bytes))
}

fn canonicalize_object_keys(value: Value) -> Value {
    match value {
        Value::Object(fields) => Value::Object(
            fields
                .into_iter()
                .map(|(key, value)| (key, canonicalize_object_keys(value)))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        Value::Array(values) => {
            Value::Array(values.into_iter().map(canonicalize_object_keys).collect())
        }
        value => value,
    }
}

/// The wire shape a validator reads before re-deriving the record.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawProposalRecord {
    pub(crate) schema_version: String,
    pub(crate) proposal_set_digest: String,
    pub(crate) supersedes: Option<String>,
    pub(crate) bindings: ProposalBindings,
    pub(crate) content_bindings: Vec<RawContentBinding>,
    pub(crate) patches: Vec<RawProposalPatch>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawContentBinding {
    pub(crate) object_id: String,
    pub(crate) content_hash: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawProposalPatch {
    pub(crate) finding_id: String,
    pub(crate) placement_path: String,
    pub(crate) page_id: String,
    pub(crate) target: String,
    pub(crate) operation: PatchOperation,
    pub(crate) patch_digest: String,
    pub(crate) patch: Value,
}
