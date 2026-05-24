//! V3.1 git adapter — the only place in the crate that shells out to the
//! system `git` binary. See V3-DESIGN.md §"Workspace Layout" and ADR-0018.
//!
//! Domain and application layers never import this module directly; the
//! composition root in `lib.rs` is the only wiring site.

pub(crate) mod changed_files;
pub(crate) mod error;
pub(crate) mod util;
pub(crate) mod worktree;
