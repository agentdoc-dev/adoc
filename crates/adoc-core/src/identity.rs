use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ObjectId(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ObjectIdError;

impl ObjectId {
    /// Construct an Object ID that satisfies the AgentDoc grammar.
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, ObjectIdError> {
        let value = value.into();
        if is_valid_object_id(&value) {
            Ok(Self(value))
        } else {
            Err(ObjectIdError)
        }
    }

    /// Construct an Object ID without enforcing the segment-grammar invariant.
    ///
    /// Reserved for explicit, scoped exceptions where the caller has decided
    /// the surrounding context makes the value safe (e.g. the
    /// `PageId::UNTITLED_FALLBACK` sentinel that intentionally violates the
    /// "≥2 kebab-case segments" rule because no derivable identity exists).
    /// Add a comment at every call site explaining why validation is being
    /// bypassed.
    pub(crate) fn new_unchecked(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_valid_object_id(value: &str) -> bool {
    let mut segment_count = 0;
    for segment in value.split('.') {
        segment_count += 1;
        if !is_valid_segment(segment) {
            return false;
        }
    }
    segment_count >= 2
}

fn is_valid_segment(segment: &str) -> bool {
    !segment.is_empty()
        && !segment.starts_with('-')
        && !segment.ends_with('-')
        && segment.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
}

impl fmt::Display for ObjectId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PageId(ObjectId);

impl PageId {
    /// Sentinel used when a page has neither an `@doc()` annotation nor a
    /// derivable file-path identity.
    ///
    /// **Invariant exception:** Object IDs normally require at least two
    /// kebab-case segments (CONTEXT.md / docs/adr/0002 design contract).
    /// `"untitled"` is one segment by deliberate design — it's the placeholder
    /// the renderer needs to keep emitting `data-page-id` in the truly
    /// pathological case (empty workspace path with no front-matter). When
    /// `ObjectId::new` gains v0.x grammar validation, this sentinel must keep
    /// flowing through [`ObjectId::new_unchecked`], not `new`.
    pub(crate) const UNTITLED_FALLBACK: &'static str = "untitled";

    pub(crate) fn new(id: ObjectId) -> Self {
        Self(id)
    }

    pub(crate) fn from_string(value: impl Into<String>) -> Result<Self, ObjectIdError> {
        ObjectId::new(value).map(Self)
    }

    pub(crate) fn untitled_fallback() -> Self {
        // Intentionally bypasses Object ID grammar validation: see
        // UNTITLED_FALLBACK doc-comment for the why.
        Self(ObjectId::new_unchecked(Self::UNTITLED_FALLBACK))
    }

    pub(crate) fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for PageId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn untitled_fallback_keeps_single_segment_sentinel() {
        // Pin the wire-visible value so a future grammar-tightening change to
        // ObjectId::new can't accidentally break the fallback path's output.
        assert_eq!(PageId::untitled_fallback().as_str(), "untitled");
    }

    #[test]
    fn new_unchecked_accepts_single_segment_value() {
        // The blessed bypass must accept inputs that ObjectId::new will
        // (eventually) reject. Today both accept anything; the contract this
        // test pins is "new_unchecked is the path that survives v0.x".
        let id = ObjectId::new_unchecked("untitled");
        assert_eq!(id.as_str(), "untitled");
    }

    #[test]
    fn new_accepts_lowercase_dot_separated_kebab_segments() {
        let id = ObjectId::new("billing.credits.decrement-after-success").expect("valid Object ID");
        assert_eq!(id.as_str(), "billing.credits.decrement-after-success");
    }

    #[test]
    fn new_rejects_single_segment_value() {
        assert!(ObjectId::new("untitled").is_err());
    }

    #[test]
    fn new_rejects_uppercase_underscores_spaces_and_edge_hyphens() {
        for value in [
            "Billing.credits",
            "billing_credit.limit",
            "billing.credits limit",
            "billing.-credits",
            "billing.credits-",
        ] {
            assert!(ObjectId::new(value).is_err(), "{value} should be invalid");
        }
    }
}
