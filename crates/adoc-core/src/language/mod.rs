//! Pure AgentDoc source-language mechanics.
//!
//! These modules parse, validate, project, and render in-memory domain values.
//! They perform no filesystem, process, network, or environment I/O.

pub(crate) mod graph_projection;
pub(crate) mod parser;
pub(crate) mod render;
pub(crate) mod validate;
