//! V3 review aggregate family.
//!
//! Pure domain types and projections. Must not import `infrastructure/`.
//! See `docs/V3-DESIGN.md` for the slice contract and `docs/adr/0018-v3-review-architecture.md`
//! for the architectural rationale.

pub(crate) mod object_change;
pub(crate) mod object_diff;
