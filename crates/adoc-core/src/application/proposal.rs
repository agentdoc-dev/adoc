//! E5.1 — build and validate canonical proposal records.
//!
//! The domain owns the record's invariants; this layer only turns exact
//! patch bytes into parsed `adoc.patch.v0` documents through the artifact
//! reader and re-derives a received record from its wire bytes.

use serde_json::Value;

use crate::domain::proposal::{
    PROPOSAL_SCHEMA_VERSION, ParsedProposalPatch, ProposalBindings, ProposalDispositionInput,
    ProposalPatchInput, ProposalRecord, ProposalRecordError, RawProposalRecord,
    canonical_patch_bytes,
};
use crate::infrastructure::artifact::read_patch_document_value;

/// Assemble a canonical proposal record from exact patch bytes.
pub fn build_proposal_record(
    bindings: ProposalBindings,
    patches: Vec<ProposalPatchInput>,
    supersedes: Option<String>,
) -> Result<ProposalRecord, ProposalRecordError> {
    ProposalRecord::assemble(bindings, parse_patches(patches)?, Vec::new(), supersedes)
}

/// Assemble a canonical proposal record with human-authorized per-finding
/// no-change evidence. The patch-set digest remains patch-only.
pub fn build_proposal_record_with_dispositions(
    bindings: ProposalBindings,
    patches: Vec<ProposalPatchInput>,
    dispositions: Vec<ProposalDispositionInput>,
    supersedes: Option<String>,
) -> Result<ProposalRecord, ProposalRecordError> {
    ProposalRecord::assemble(bindings, parse_patches(patches)?, dispositions, supersedes)
}

fn parse_patches(
    patches: Vec<ProposalPatchInput>,
) -> Result<Vec<ParsedProposalPatch>, ProposalRecordError> {
    patches
        .into_iter()
        .map(|input| {
            let patch: Value = serde_json::from_slice(&input.patch_bytes).map_err(|error| {
                ProposalRecordError::PatchInvalid {
                    message: format!(
                        "patch for finding '{}' is not JSON: {error}",
                        input.finding_id
                    ),
                }
            })?;
            parse_patch(input, patch)
        })
        .collect()
}

/// Read `adoc.proposal.v0` bytes and re-derive every digest and ordering;
/// any field that does not match its canonical derivation fails closed.
pub fn validate_proposal_record(bytes: &[u8]) -> Result<ProposalRecord, ProposalRecordError> {
    let received: Value =
        serde_json::from_slice(bytes).map_err(|error| ProposalRecordError::InvalidDocument {
            message: error.to_string(),
        })?;
    let raw: RawProposalRecord = serde_json::from_value(received.clone()).map_err(|error| {
        ProposalRecordError::InvalidDocument {
            message: error.to_string(),
        }
    })?;
    if raw.schema_version != PROPOSAL_SCHEMA_VERSION {
        return Err(ProposalRecordError::UnsupportedVersion {
            version: raw.schema_version,
        });
    }
    let mut claimed = Vec::with_capacity(raw.patches.len());
    let parsed = raw
        .patches
        .into_iter()
        .map(|patch| {
            claimed.push((patch.target, patch.operation, patch.patch_digest));
            let input = ProposalPatchInput {
                finding_id: patch.finding_id,
                placement_path: patch.placement_path,
                page_id: patch.page_id,
                patch_bytes: canonical_patch_bytes(&patch.patch)?,
            };
            parse_patch(input, patch.patch)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let rebuilt = ProposalRecord::assemble(raw.bindings, parsed, raw.dispositions, raw.supersedes)?;
    let mismatch = |what: &str| ProposalRecordError::InvalidDocument {
        message: format!("{what} does not match its canonical derivation"),
    };
    if raw.proposal_set_digest != rebuilt.proposal_set_digest() {
        return Err(mismatch("proposal_set_digest"));
    }
    let derived_bindings: Vec<_> = rebuilt
        .content_bindings()
        .iter()
        .map(|binding| (binding.object_id(), binding.content_hash()))
        .collect();
    let claimed_bindings: Vec<_> = raw
        .content_bindings
        .iter()
        .map(|binding| (binding.object_id.as_str(), binding.content_hash.as_str()))
        .collect();
    if claimed_bindings != derived_bindings {
        return Err(mismatch("content_bindings"));
    }
    let derived: Vec<_> = rebuilt
        .patches()
        .iter()
        .map(|patch| (patch.target(), patch.operation(), patch.patch_digest()))
        .collect();
    let claimed: Vec<_> = claimed
        .iter()
        .map(|(target, operation, digest)| (target.as_str(), *operation, digest.as_str()))
        .collect();
    if claimed != derived {
        return Err(mismatch("patch order, targets, operations, or digests"));
    }
    let canonical =
        serde_json::to_value(&rebuilt).map_err(|error| ProposalRecordError::InvalidDocument {
            message: error.to_string(),
        })?;
    if canonical != received {
        return Err(ProposalRecordError::InvalidDocument {
            message: "record fields do not match their canonical derivation".to_string(),
        });
    }
    Ok(rebuilt)
}

fn parse_patch(
    input: ProposalPatchInput,
    patch: Value,
) -> Result<ParsedProposalPatch, ProposalRecordError> {
    let label = format!("Proposal patch for finding '{}'", input.finding_id);
    let document = read_patch_document_value(patch.clone(), &label).map_err(|diagnostics| {
        ProposalRecordError::PatchInvalid {
            message: diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.clone())
                .collect::<Vec<_>>()
                .join("; "),
        }
    })?;
    Ok(ParsedProposalPatch {
        input,
        document,
        patch,
    })
}

impl ProposalRecord {
    /// Mint the successor of this record from edited patch bytes. The new
    /// record supersedes this one by digest; byte-identical patches are not a
    /// new version and fail with `proposal_record.revision_unchanged`.
    pub fn revise(
        &self,
        patches: Vec<ProposalPatchInput>,
    ) -> Result<ProposalRecord, ProposalRecordError> {
        build_proposal_record_with_dispositions(
            self.bindings().clone(),
            patches,
            self.dispositions().to_vec(),
            Some(self.proposal_set_digest().to_string()),
        )
    }
}
