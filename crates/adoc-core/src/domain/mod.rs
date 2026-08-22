// see ADR-0009
pub(crate) mod artifact;
pub(crate) mod ast;
pub(crate) mod diagnostic;
pub(crate) mod graph;
pub(crate) mod identity;
pub(crate) mod inline;
pub(crate) mod knowledge_object;
// E1.2: the managed identity contract is consumed by its domain tests and
// the stacked E1 slices; no local adapter surface exists yet (Cloud is the
// adapter, in its own repository), so the lib build allows dead_code here.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) mod managed;
pub(crate) mod obligation;
pub(crate) mod patch;
pub(crate) mod ports;
pub(crate) mod project_config;
pub(crate) mod retrieval;
pub(crate) mod review;
pub(crate) mod rules;
pub(crate) mod scan;
pub(crate) mod services;
pub(crate) mod source;
pub(crate) mod source_edit;
pub(crate) mod url_safety;
pub(crate) mod value_objects;
pub(crate) mod values;
