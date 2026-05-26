//! Wire envelopes for `adoc.diff.v0` and `adoc.review.v0`.
//!
//! Extracted from `application/review.rs` in Step E2 — these types are
//! serialization-shaped DTOs the CLI and MCP layers turn into JSON.
//! Keeping them adjacent to the orchestration in `application::review`
//! but in their own file keeps `review.rs` focused on the snapshot →
//! compile → diff → enrich pipeline.
//!
//! Visibility is intentionally `pub` for now so existing callers in
//! `adoc-local`, `adoc-cli`, and `adoc-mcp` keep working unchanged.
//! E3 demotes them to `pub(crate)` once the public API of `adoc-core`
//! exposes `build_*_envelope_value(...) -> serde_json::Value` instead.

use serde::Serialize;

use crate::application::patch::PatchCheckResult;
use crate::application::review::{
    DIFF_SCHEMA_VERSION, REVIEW_SCHEMA_VERSION, ReviewSession, diff_objects,
};
use crate::domain::diagnostic::Diagnostic;
use crate::domain::graph::GraphKnowledgeObjectNode;
use crate::domain::obligation::ProofObligation;
use crate::domain::review::impact::ImpactedObject;
use crate::domain::review::object_change::ChangedObject;
use crate::domain::review::object_diff::ObjectDiff;
use crate::domain::review::reviewer::RequiredReviewer;

/// Wire envelope for `adoc.diff.v0`. The CLI and (V3.6) MCP layers serialize
/// this struct directly to JSON.
#[derive(Debug, Clone, Serialize)]
pub struct ObjectDiffEnvelope {
    pub schema_version: &'static str,
    pub(crate) created: Vec<GraphKnowledgeObjectNode>,
    pub(crate) deleted: Vec<GraphKnowledgeObjectNode>,
    pub(crate) changed: Vec<ChangedObject>,
    pub diagnostics: Vec<Diagnostic>,
}

impl ObjectDiffEnvelope {
    pub fn from_diff(diff: ObjectDiff, diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            schema_version: DIFF_SCHEMA_VERSION,
            created: diff.created,
            deleted: diff.deleted,
            changed: diff.changed,
            diagnostics,
        }
    }

    /// Number of Knowledge Objects only present on the head side.
    pub fn created_count(&self) -> usize {
        self.created.len()
    }

    /// Number of Knowledge Objects only present on the base side.
    pub fn deleted_count(&self) -> usize {
        self.deleted.len()
    }

    /// Number of Knowledge Objects whose `content_hash` differs between
    /// base and head.
    pub fn changed_count(&self) -> usize {
        self.changed.len()
    }

    /// Object IDs of created entries, in deterministic sort order.
    pub fn created_ids(&self) -> impl Iterator<Item = &str> {
        self.created.iter().map(|node| node.id.as_str())
    }

    /// Object IDs of deleted entries, in deterministic sort order.
    pub fn deleted_ids(&self) -> impl Iterator<Item = &str> {
        self.deleted.iter().map(|node| node.id.as_str())
    }

    /// Object IDs of changed entries, in deterministic sort order.
    pub fn changed_ids(&self) -> impl Iterator<Item = &str> {
        self.changed.iter().map(|entry| entry.id.as_str())
    }

    /// Changed entries in deterministic sort order. Exposed so the CLI can
    /// render the V3.2 field-level projection beneath each id.
    pub fn changed(&self) -> &[ChangedObject] {
        &self.changed
    }
}

/// Wire envelope for `adoc.review.v0` (V3.3). Embeds the V3.1 diff envelope
/// alongside the V3.3 impact and required-reviewer projections. The schema
/// stays at `v0` across V3 — later slices add optional fields (proof
/// obligations in V3.4, patch_check in V3.7); tolerant readers required.
#[derive(Debug, Clone, Serialize)]
pub struct ReviewEnvelope {
    pub schema_version: &'static str,
    pub diff: ObjectDiffEnvelope,
    pub impact: Vec<ImpactedObject>,
    pub required_reviewers: Vec<RequiredReviewer>,
    /// V3.4 proof obligations. Empty list when no triggers fire.
    /// V3.7 unions this with `patch_check.proof_obligations` (when present),
    /// deduped by `(object_id, reason)` — tolerant readers that only consult
    /// this top-level field still see the complete obligation set.
    pub proof_obligations: Vec<ProofObligation>,
    /// V3.7 — embedded `adoc.patch.check.v0` validation result, present only
    /// when `adoc review` was invoked with `--patch`. Omitted from the JSON
    /// envelope entirely (not `null`) when no patch was supplied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch_check: Option<PatchCheckResult>,
    pub diagnostics: Vec<Diagnostic>,
}

impl ReviewEnvelope {
    /// Build the wire envelope from a loaded [`ReviewSession`] and the diff
    /// computed against it. The session's `impact_analysis`,
    /// `required_reviewers`, and `proof_obligations` are cloned in.
    ///
    /// V3.7 callers that also have a patch to validate go through
    /// [`crate::application::review::review_with_patch`] instead, which
    /// folds in `patch_check` and unions obligations.
    pub fn from_session(session: &ReviewSession, diagnostics: Vec<Diagnostic>) -> Self {
        Self::from_session_with_patch_check(session, diagnostics, None)
    }

    /// V3.7 envelope constructor. Embeds an optional `patch_check` and
    /// unions its `proof_obligations` with the session's diff/impact-driven
    /// obligations, deduplicated by `(object_id, reason)` — same predicate
    /// the V3.4 aggregator and the V2 patch validator already use.
    pub fn from_session_with_patch_check(
        session: &ReviewSession,
        diagnostics: Vec<Diagnostic>,
        patch_check: Option<PatchCheckResult>,
    ) -> Self {
        let diff = diff_objects(session);
        // Session obligations come first so they win ties on (object_id, reason)
        // with patch-check obligations that target the same row.
        let session_obligations = session.proof_obligations().iter().cloned();
        let patch_obligations = patch_check
            .iter()
            .flat_map(|report| report.proof_obligations.iter().cloned());
        let proof_obligations =
            ProofObligation::merge_dedup_sorted(session_obligations.chain(patch_obligations));
        Self {
            schema_version: REVIEW_SCHEMA_VERSION,
            diff: ObjectDiffEnvelope::from_diff(diff, Vec::new()),
            impact: session.impact_analysis().to_vec(),
            required_reviewers: session.required_reviewers().to_vec(),
            proof_obligations,
            patch_check,
            diagnostics,
        }
    }
}
