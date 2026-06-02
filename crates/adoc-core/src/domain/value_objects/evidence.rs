//! Inline evidence value object — the typed replacement for the V0 flat
//! `source`/`test`/`reviewed_by` fields.
//!
//! A V5.8 `Evidence` is always an `Inline` entry: a typed
//! [`EvidenceKind`] paired with a non-empty [`EvidenceValue`] string.
//! TB2 will add an `ObjectRef(ObjectId)` variant; this module is
//! intentionally kept crate-internal so the addition is non-breaking.

use crate::domain::value_objects::evidence_kind::EvidenceKind;
use crate::domain::values::NonEmptyText;

/// A single piece of evidence attached to a `Verification`.
///
/// Currently the only variant is `Inline` — an evidence kind + inline text
/// value pair. TB2 will add `ObjectRef(ObjectId)` inside the same crate;
/// because this type is `pub(crate)`, external callers cannot construct it
/// and therefore won't be broken by the addition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Evidence {
    Inline {
        kind: EvidenceKind,
        value: EvidenceValue,
    },
}

impl Evidence {
    /// Construct an `Inline` evidence entry from a typed kind and a raw value
    /// string. Returns `None` when `value` is empty or whitespace-only.
    pub(crate) fn inline(kind: EvidenceKind, value: &str) -> Option<Self> {
        EvidenceValue::try_new(value).map(|v| Self::Inline { kind, value: v })
    }

    /// Map a V0 field name to a typed kind and construct an `Inline` entry.
    ///
    /// Mappings:
    /// - `"source"` → [`EvidenceKind::SourceCode`]
    /// - `"test"` → [`EvidenceKind::Test`]
    /// - `"reviewed_by"` → [`EvidenceKind::HumanReview`]
    /// - `"human_review"` → [`EvidenceKind::HumanReview`]
    ///
    /// Returns `None` for unknown field names or when `value` is empty.
    pub(crate) fn from_field(field_name: &str, value: &str) -> Option<Self> {
        let kind = match field_name {
            "source" => EvidenceKind::SourceCode,
            "test" => EvidenceKind::Test,
            "reviewed_by" | "human_review" => EvidenceKind::HumanReview,
            _ => return None,
        };
        Self::inline(kind, value)
    }

    /// The typed evidence kind for this entry.
    pub(crate) fn kind(&self) -> EvidenceKind {
        match self {
            Self::Inline { kind, .. } => *kind,
        }
    }

    /// The inline value string for this entry.
    pub(crate) fn value(&self) -> &EvidenceValue {
        match self {
            Self::Inline { value, .. } => value,
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
        assert_eq!(ev.kind(), EvidenceKind::SourceCode);
        assert_eq!(ev.value().as_str(), "ledger");
    }

    #[test]
    fn evidence_inline_returns_none_for_empty_value() {
        assert!(Evidence::inline(EvidenceKind::Test, "").is_none());
    }

    // ------------------------------------------------------------------
    // Evidence::from_field — field-name mapping
    // ------------------------------------------------------------------

    #[test]
    fn from_field_source_maps_to_source_code() {
        let ev = Evidence::from_field("source", "payments ledger").expect("valid");
        assert_eq!(ev.kind(), EvidenceKind::SourceCode);
    }

    #[test]
    fn from_field_test_maps_to_test() {
        let ev = Evidence::from_field("test", "cargo test billing").expect("valid");
        assert_eq!(ev.kind(), EvidenceKind::Test);
    }

    #[test]
    fn from_field_reviewed_by_maps_to_human_review() {
        let ev = Evidence::from_field("reviewed_by", "qa-team").expect("valid");
        assert_eq!(ev.kind(), EvidenceKind::HumanReview);
    }

    #[test]
    fn from_field_human_review_also_maps_to_human_review() {
        let ev = Evidence::from_field("human_review", "ops-run").expect("valid");
        assert_eq!(ev.kind(), EvidenceKind::HumanReview);
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
                expected_kind,
                "field {field:?} should map to {expected_kind:?}"
            );
            // EvidenceKind round-trips through as_str → try_new
            let wire = ev.kind().as_str();
            assert_eq!(
                EvidenceKind::try_new(wire),
                Ok(expected_kind),
                "as_str round-trip failed for {wire:?}"
            );
        }
    }
}
