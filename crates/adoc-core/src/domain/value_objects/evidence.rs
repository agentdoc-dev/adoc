//! Inline evidence value object — the typed replacement for the V0 flat
//! `source`/`test`/`reviewed_by` fields.
//!
//! A V5.8 `Evidence` may be an `Inline` entry (a typed [`EvidenceKind`]
//! paired with a non-empty [`EvidenceValue`] string) or an `ObjectRef`
//! entry pointing to a `source` Knowledge Object by [`ObjectId`].

use crate::domain::identity::ObjectId;
use crate::domain::value_objects::evidence_kind::EvidenceKind;
use crate::domain::values::NonEmptyText;

/// A single piece of evidence attached to a `Claim` or `Verification`.
///
/// Two variants exist:
/// - `Inline` — an evidence kind + inline text value pair (V5.8 TB1).
/// - `ObjectRef` — a reference to a `source` Knowledge Object by ID (V5.8 TB2).
///
/// Because this type is `pub(crate)`, external callers cannot construct it
/// and are not broken by the addition of new variants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Evidence {
    Inline {
        kind: EvidenceKind,
        value: EvidenceValue,
    },
    /// A reference to a `source` Knowledge Object by ID.
    ///
    /// Added in V5.8 TB2 to support `evidence_ref:` on claims. The target
    /// must resolve to an existing `source` object at workspace-validation
    /// time.
    ObjectRef(ObjectId),
}

impl Evidence {
    /// Construct an `Inline` evidence entry from a typed kind and a raw value
    /// string. Returns `None` when `value` is empty or whitespace-only.
    pub(crate) fn inline(kind: EvidenceKind, value: &str) -> Option<Self> {
        EvidenceValue::try_new(value).map(|v| Self::Inline { kind, value: v })
    }

    /// Construct an `ObjectRef` evidence entry pointing to the given object.
    pub(crate) fn object_ref(id: ObjectId) -> Self {
        Self::ObjectRef(id)
    }

    /// Map a V0 field name to a typed kind and construct an `Inline` entry.
    ///
    /// Mappings:
    /// - `"source"` → [`EvidenceKind::SourceCode`]
    /// - `"test"` → [`EvidenceKind::Test`]
    /// - `"reviewed_by"` → [`EvidenceKind::HumanReview`]
    /// - `"human_review"` → [`EvidenceKind::HumanReview`]
    /// - `"external_url"` → [`EvidenceKind::ExternalUrl`] (V5.10 TB5: Low-tier
    ///   inline evidence; warns `claim.evidence_quality_low` on verified claims
    ///   that rely solely on this kind)
    ///
    /// Returns `None` for unknown field names or when `value` is empty.
    pub(crate) fn from_field(field_name: &str, value: &str) -> Option<Self> {
        let kind = match field_name {
            "source" => EvidenceKind::SourceCode,
            "test" => EvidenceKind::Test,
            "reviewed_by" | "human_review" => EvidenceKind::HumanReview,
            "external_url" => EvidenceKind::ExternalUrl,
            _ => return None,
        };
        Self::inline(kind, value)
    }

    /// The typed evidence kind for `Inline` entries; `None` for `ObjectRef`.
    ///
    /// Call sites that only handle `Inline` evidence (e.g. the verification
    /// graph projection) match on this returning `Some(kind)` and skip `None`.
    pub(crate) fn kind(&self) -> Option<EvidenceKind> {
        match self {
            Self::Inline { kind, .. } => Some(*kind),
            Self::ObjectRef(_) => None,
        }
    }

    /// The object ID that this entry points to, or `None` for `Inline`.
    pub(crate) fn target_id(&self) -> Option<&ObjectId> {
        match self {
            Self::ObjectRef(id) => Some(id),
            Self::Inline { .. } => None,
        }
    }

    /// The inline value string for `Inline` entries; `None` for `ObjectRef`.
    pub(crate) fn value(&self) -> Option<&EvidenceValue> {
        match self {
            Self::Inline { value, .. } => Some(value),
            Self::ObjectRef(_) => None,
        }
    }
}

/// A non-empty evidence text value.
///
/// Constructed only via [`EvidenceValue::try_new`]; whitespace-only input
/// returns `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EvidenceValue(String);

impl EvidenceValue {
    /// Construct an `EvidenceValue` from a raw string. Trims ASCII edges
    /// (via `NonEmptyText`); returns `None` when the result is empty.
    pub(crate) fn try_new(value: &str) -> Option<Self> {
        NonEmptyText::try_new(value).map(|v| Self(v.as_str().to_string()))
    }

    /// The trimmed evidence value string.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // EvidenceValue
    // ------------------------------------------------------------------

    #[test]
    fn evidence_value_try_new_rejects_empty_string() {
        assert!(EvidenceValue::try_new("").is_none());
    }

    #[test]
    fn evidence_value_try_new_rejects_whitespace_only() {
        assert!(EvidenceValue::try_new("   \t  ").is_none());
    }

    #[test]
    fn evidence_value_try_new_trims_and_accepts() {
        let val = EvidenceValue::try_new("  billing ledger  ").expect("valid value");
        assert_eq!(val.as_str(), "billing ledger");
    }

    // ------------------------------------------------------------------
    // Evidence::inline
    // ------------------------------------------------------------------

    #[test]
    fn evidence_inline_constructs_and_exposes_kind_and_value() {
        let ev = Evidence::inline(EvidenceKind::SourceCode, "ledger").expect("valid");
        assert_eq!(ev.kind(), Some(EvidenceKind::SourceCode));
        assert_eq!(ev.value().expect("has value").as_str(), "ledger");
        assert!(ev.target_id().is_none());
    }

    #[test]
    fn evidence_inline_returns_none_for_empty_value() {
        assert!(Evidence::inline(EvidenceKind::Test, "").is_none());
    }

    // ------------------------------------------------------------------
    // Evidence::ObjectRef
    // ------------------------------------------------------------------

    #[test]
    fn evidence_object_ref_constructs_and_exposes_target_id() {
        use crate::domain::identity::ObjectId;
        let id = ObjectId::new("billing.consume-use-case").expect("valid id");
        let ev = Evidence::object_ref(id.clone());
        assert_eq!(ev.target_id(), Some(&id));
        assert!(ev.kind().is_none());
        assert!(ev.value().is_none());
    }

    #[test]
    fn evidence_object_ref_matches_correctly() {
        use crate::domain::identity::ObjectId;
        let id = ObjectId::new("auth.source-ref").expect("valid id");
        let ev = Evidence::object_ref(id.clone());
        match &ev {
            Evidence::ObjectRef(inner) => assert_eq!(inner, &id),
            Evidence::Inline { .. } => panic!("expected ObjectRef"),
        }
    }

    // ------------------------------------------------------------------
    // Evidence::from_field — field-name mapping
    // ------------------------------------------------------------------

    #[test]
    fn from_field_source_maps_to_source_code() {
        let ev = Evidence::from_field("source", "payments ledger").expect("valid");
        assert_eq!(ev.kind(), Some(EvidenceKind::SourceCode));
    }

    #[test]
    fn from_field_test_maps_to_test() {
        let ev = Evidence::from_field("test", "cargo test billing").expect("valid");
        assert_eq!(ev.kind(), Some(EvidenceKind::Test));
    }

    #[test]
    fn from_field_reviewed_by_maps_to_human_review() {
        let ev = Evidence::from_field("reviewed_by", "qa-team").expect("valid");
        assert_eq!(ev.kind(), Some(EvidenceKind::HumanReview));
    }

    #[test]
    fn from_field_human_review_also_maps_to_human_review() {
        let ev = Evidence::from_field("human_review", "ops-run").expect("valid");
        assert_eq!(ev.kind(), Some(EvidenceKind::HumanReview));
    }

    #[test]
    fn from_field_reviewed_by_and_human_review_collapse_to_same_kind() {
        let a = Evidence::from_field("reviewed_by", "qa").expect("valid");
        let b = Evidence::from_field("human_review", "qa").expect("valid");
        assert_eq!(a.kind(), b.kind());
    }

    #[test]
    fn from_field_unknown_key_returns_none() {
        assert!(Evidence::from_field("not_a_field", "anything").is_none());
    }

    #[test]
    fn from_field_returns_none_for_empty_value() {
        assert!(Evidence::from_field("source", "").is_none());
    }

    // ------------------------------------------------------------------
    // Display / as_str round-trip (via EvidenceKind)
    // ------------------------------------------------------------------

    #[test]
    fn evidence_kind_round_trip_for_all_v0_mappings() {
        for (field, expected_kind) in [
            ("source", EvidenceKind::SourceCode),
            ("test", EvidenceKind::Test),
            ("reviewed_by", EvidenceKind::HumanReview),
            ("human_review", EvidenceKind::HumanReview),
        ] {
            let ev = Evidence::from_field(field, "value").expect("valid");
            assert_eq!(
                ev.kind(),
                Some(expected_kind),
                "field {field:?} should map to {expected_kind:?}"
            );
            // EvidenceKind round-trips through as_str → try_new
            let wire = ev.kind().expect("inline has kind").as_str();
            assert_eq!(
                EvidenceKind::try_new(wire),
                Ok(expected_kind),
                "as_str round-trip failed for {wire:?}"
            );
        }
    }
}
