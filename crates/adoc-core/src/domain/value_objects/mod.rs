//! Cross-aggregate value objects. Each module owns one type with constructor-asserted
//! invariants; aggregates store these types directly rather than re-validating strings.
//!
//! Added in V3.3 to give `impacts:` a typed home; future value objects (e.g. owner
//! handles, version strings) belong here too once they outgrow their original
//! aggregate.

pub(crate) mod action;
pub(crate) mod action_set;
pub(crate) mod anchor_hash;
pub(crate) mod approved_by;
pub(crate) mod contradiction_claims;
pub(crate) mod contradiction_status;
pub(crate) mod effective_date;
pub(crate) mod evidence;
pub(crate) mod evidence_kind;
pub(crate) mod http_method;
pub(crate) mod lang;
pub(crate) mod lifecycle_status;
pub(crate) mod rel_path;
pub(crate) mod review_interval;
pub(crate) mod sample_size;
pub(crate) mod sandbox;
pub(crate) mod scope;
pub(crate) mod severity;
pub(crate) mod trust;
pub(crate) mod url;
