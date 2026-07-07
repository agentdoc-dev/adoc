//! Domain services for behavior that spans an aggregate family.
//!
//! Keep orchestration over pages/workspaces in `application/`; services here
//! operate on domain inputs and return domain values.

pub(crate) mod resolve_pending_block;
pub(crate) mod suggest_typed_blocks;
