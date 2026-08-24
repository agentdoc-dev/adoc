// see ADR-0009
pub(crate) mod artifact;
pub(crate) mod ast;
pub(crate) mod diagnostic;
pub(crate) mod executor_qualification;
pub(crate) mod graph;
pub(crate) mod hashing;
pub(crate) mod identity;
pub(crate) mod inline;
pub(crate) mod knowledge_object;
// E1.2–E1.6: the managed identity, reconciliation, managed state,
// lifecycle mapping, and stage-bound obligation contracts are consumed
// by their domain tests and the stacked E1 slices; no local adapter
// surface exists yet (Cloud is the adapter, in its own repository), so
// the lib build allows dead_code here.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) mod lifecycle_mapping;
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) mod managed;
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) mod managed_state;
pub(crate) mod obligation;
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) mod obligation_record;
pub(crate) mod patch;
pub(crate) mod ports;
pub(crate) mod project_config;
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) mod reconciliation;
pub(crate) mod retrieval;
pub(crate) mod review;
pub(crate) mod rules;
pub(crate) mod scan;
pub(crate) mod semantic_assessment;
pub(crate) mod semantic_context;
pub(crate) mod services;
pub(crate) mod source;
pub(crate) mod source_edit;
pub(crate) mod url_safety;
pub(crate) mod value_objects;
pub(crate) mod values;
