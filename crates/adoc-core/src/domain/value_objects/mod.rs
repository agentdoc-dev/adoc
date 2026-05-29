//! Cross-aggregate value objects. Each module owns one type with constructor-asserted
//! invariants; aggregates store these types directly rather than re-validating strings.
//!
//! Added in V3.3 to give `impacts:` a typed home; future value objects (e.g. owner
//! handles, version strings) belong here too once they outgrow their original
//! aggregate.

pub(crate) mod lang;
pub(crate) mod rel_path;
pub(crate) mod sandbox;
pub(crate) mod severity;
