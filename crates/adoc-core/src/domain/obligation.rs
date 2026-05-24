//! Shared `ProofObligation` value object.
//!
//! Promoted from `domain/patch/mod.rs` in V3.4 so both the V2 patch
//! validation surface (`domain/patch/`) and the V3 review surface
//! (`domain/review/`) can speak the same vocabulary about proof. See
//! ADR-0020.
//!
//! The wire shape (`{ object_id, reason, required_evidence }`) is the
//! contract embedded in `adoc.patch.check.v0` and `adoc.review.v0`. Field
//! values are populated by the respective trigger logic — V2 by
//! `PatchValidator`, V3 by `domain/review/obligation_rules.rs`.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProofObligation {
    pub object_id: String,
    pub reason: String,
    pub required_evidence: Vec<String>,
}
