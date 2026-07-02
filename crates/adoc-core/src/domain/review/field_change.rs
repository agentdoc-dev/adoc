//! Typed projection over a [`super::object_change::ChangedObject`].
//!
//! `FieldChange` is the V3.2 vocabulary that explains *what* differs inside a
//! Knowledge Object whose `content_hash` flipped between two snapshots. It is
//! pure data — the projection logic lives in
//! `application::review::project_changed` and is decorated onto each
//! `Changed` entry by `application::review::diff_objects`.
//!
//! Sealed `#[non_exhaustive]` enum: V3.3 will add `ImpactsAdded` /
//! `ImpactsRemoved` and tolerant readers must already ignore unknown variants.
//!
//! See V3-DESIGN.md §V3.2 and ADR-0018.

use std::fmt;

use serde::Serialize;

/// One typed difference between the base and head sides of a `Changed`
/// Object Change. Variants are scoped to the V3.2 vocabulary; the enum is
/// `#[non_exhaustive]` so future slices (V3.3 `Impacts*`) can extend without
/// bumping the wire schema.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FieldChange {
    Body {
        before: String,
        after: String,
    },
    Status {
        before: Option<String>,
        after: Option<String>,
    },
    /// V5.1: the `severity` of a severity-bearing kind (`constraint`) changed.
    /// Stored in the graph node's `status` slot but projected distinctly so the
    /// shared `Severity` value object reads as severity, not lifecycle status.
    Severity {
        before: Option<String>,
        after: Option<String>,
    },
    Owner {
        before: Option<String>,
        after: Option<String>,
    },
    VerifiedAt {
        before: Option<String>,
        after: Option<String>,
    },
    EvidenceAdded {
        field: String,
        value: String,
    },
    EvidenceRemoved {
        field: String,
        value: String,
    },
    RelationAdded {
        kind: RelationKind,
        target: String,
    },
    RelationRemoved {
        kind: RelationKind,
        target: String,
    },
    /// V3.3: a new path appeared in the `impacts:` list on the head side.
    ImpactsAdded {
        path: String,
    },
    /// V3.3: a path that was present on the base side disappeared on head.
    ImpactsRemoved {
        path: String,
    },
    /// V5.4: the `effective_at` date on a policy object changed.
    EffectiveAt {
        before: Option<String>,
        after: Option<String>,
    },
    /// V5.4: a new approver appeared in the `approved_by` list on a policy.
    ApprovedByAdded {
        value: String,
    },
    /// V5.4: an approver that was present in `approved_by` was removed.
    ApprovedByRemoved {
        value: String,
    },
    /// V5.5: the `trust` level on an `agent_instruction` object changed.
    Trust {
        before: Option<String>,
        after: Option<String>,
    },
    /// V5.5: the `scope` on an `agent_instruction` object changed.
    Scope {
        before: Option<String>,
        after: Option<String>,
    },
    /// V5.5: a new action appeared in the `allowed_actions` list on an
    /// `agent_instruction`.
    AllowedActionsAdded {
        value: String,
    },
    /// V5.5: an action that was present in `allowed_actions` was removed.
    AllowedActionsRemoved {
        value: String,
    },
    /// V5.5: a new action appeared in the `forbidden_actions` list on an
    /// `agent_instruction`.
    ForbiddenActionsAdded {
        value: String,
    },
    /// V5.5: an action that was present in `forbidden_actions` was removed.
    ForbiddenActionsRemoved {
        value: String,
    },
    /// V5.6: a new claim ID appeared in the `claims` list on a `contradiction`.
    ContradictionClaimsAdded {
        value: String,
    },
    /// V5.6: a claim ID that was present in `claims` was removed from a
    /// `contradiction`.
    ContradictionClaimsRemoved {
        value: String,
    },
    /// V6.5.1: the `method` on an `api` object changed.
    ApiMethod {
        before: Option<String>,
        after: Option<String>,
    },
    /// V6.5.1: the `path` on an `api` object changed.
    ApiPath {
        before: Option<String>,
        after: Option<String>,
    },
}

impl FieldChange {
    /// Short headline label used by the presenters' per-object summary line
    /// (e.g. "body changed, status changed, owner changed"). Lives in the
    /// domain so the three V3 presenters share one source of truth — adding
    /// a future variant requires updating this one match (the
    /// `#[non_exhaustive]` fallback keeps presenters rendering until the
    /// label is filled in).
    pub fn summary_label(&self) -> &'static str {
        // Exhaustive match in-crate: `#[non_exhaustive]` only requires
        // wildcard arms for downstream crates, so a future variant added
        // here trips a compile error and prompts a label update. The
        // wildcard fallback lives in cross-crate presenter callers.
        match self {
            FieldChange::Body { .. } => "body changed",
            FieldChange::Status { .. } => "status changed",
            FieldChange::Severity { .. } => "severity changed",
            FieldChange::Owner { .. } => "owner changed",
            FieldChange::VerifiedAt { .. } => "verified_at changed",
            FieldChange::EvidenceAdded { .. } => "evidence added",
            FieldChange::EvidenceRemoved { .. } => "evidence removed",
            FieldChange::RelationAdded { .. } => "relation added",
            FieldChange::RelationRemoved { .. } => "relation removed",
            FieldChange::ImpactsAdded { .. } => "impacts added",
            FieldChange::ImpactsRemoved { .. } => "impacts removed",
            FieldChange::EffectiveAt { .. } => "effective_at changed",
            FieldChange::ApprovedByAdded { .. } => "approved_by added",
            FieldChange::ApprovedByRemoved { .. } => "approved_by removed",
            FieldChange::Trust { .. } => "trust changed",
            FieldChange::Scope { .. } => "scope changed",
            FieldChange::AllowedActionsAdded { .. } => "allowed_actions added",
            FieldChange::AllowedActionsRemoved { .. } => "allowed_actions removed",
            FieldChange::ForbiddenActionsAdded { .. } => "forbidden_actions added",
            FieldChange::ForbiddenActionsRemoved { .. } => "forbidden_actions removed",
            FieldChange::ContradictionClaimsAdded { .. } => "contradiction_claims added",
            FieldChange::ContradictionClaimsRemoved { .. } => "contradiction_claims removed",
            FieldChange::ApiMethod { .. } => "method changed",
            FieldChange::ApiPath { .. } => "path changed",
        }
    }
}

/// Default plain-text rendering, suitable for terminal output and any caller
/// that wants the canonical "label: before → after" form. Markdown and other
/// richer presenters compose around this — they style the prefix differently
/// or fence the body diff — but the labels and value-rendering stay
/// single-sourced here.
impl fmt::Display for FieldChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Exhaustive in-crate match; see `summary_label` for the rationale.
        match self {
            // Body diffs are rendered separately by markdown (fenced) and by
            // the terminal (label-only); Display keeps the terminal form.
            FieldChange::Body { .. } => f.write_str("body: changed"),
            FieldChange::Status { before, after } => write!(
                f,
                "status: {} → {}",
                optional(before.as_deref()),
                optional(after.as_deref())
            ),
            FieldChange::Severity { before, after } => write!(
                f,
                "severity: {} → {}",
                optional(before.as_deref()),
                optional(after.as_deref())
            ),
            FieldChange::Owner { before, after } => write!(
                f,
                "owner: {} → {}",
                optional(before.as_deref()),
                optional(after.as_deref())
            ),
            FieldChange::VerifiedAt { before, after } => write!(
                f,
                "verified_at: {} → {}",
                optional(before.as_deref()),
                optional(after.as_deref())
            ),
            FieldChange::EvidenceAdded { field, .. } => write!(f, "evidence: +{field}"),
            FieldChange::EvidenceRemoved { field, .. } => write!(f, "evidence: -{field}"),
            FieldChange::RelationAdded { kind, target } => {
                write!(f, "{}: +{target}", kind.as_str())
            }
            FieldChange::RelationRemoved { kind, target } => {
                write!(f, "{}: -{target}", kind.as_str())
            }
            FieldChange::ImpactsAdded { path } => write!(f, "impacts: +{path}"),
            FieldChange::ImpactsRemoved { path } => write!(f, "impacts: -{path}"),
            FieldChange::EffectiveAt { before, after } => write!(
                f,
                "effective_at: {} → {}",
                optional(before.as_deref()),
                optional(after.as_deref())
            ),
            FieldChange::ApprovedByAdded { value } => write!(f, "approved_by: +{value}"),
            FieldChange::ApprovedByRemoved { value } => write!(f, "approved_by: -{value}"),
            FieldChange::Trust { before, after } => write!(
                f,
                "trust: {} → {}",
                optional(before.as_deref()),
                optional(after.as_deref())
            ),
            FieldChange::Scope { before, after } => write!(
                f,
                "scope: {} → {}",
                optional(before.as_deref()),
                optional(after.as_deref())
            ),
            FieldChange::AllowedActionsAdded { value } => write!(f, "allowed_actions: +{value}"),
            FieldChange::AllowedActionsRemoved { value } => {
                write!(f, "allowed_actions: -{value}")
            }
            FieldChange::ForbiddenActionsAdded { value } => {
                write!(f, "forbidden_actions: +{value}")
            }
            FieldChange::ForbiddenActionsRemoved { value } => {
                write!(f, "forbidden_actions: -{value}")
            }
            FieldChange::ContradictionClaimsAdded { value } => {
                write!(f, "contradiction_claims: +{value}")
            }
            FieldChange::ContradictionClaimsRemoved { value } => {
                write!(f, "contradiction_claims: -{value}")
            }
            FieldChange::ApiMethod { before, after } => write!(
                f,
                "method: {} → {}",
                optional(before.as_deref()),
                optional(after.as_deref())
            ),
            FieldChange::ApiPath { before, after } => write!(
                f,
                "path: {} → {}",
                optional(before.as_deref()),
                optional(after.as_deref())
            ),
        }
    }
}

fn optional(value: Option<&str>) -> &str {
    value.unwrap_or("(none)")
}

/// Discriminator for the three V0 relation slots on a Knowledge Object.
/// Mirrors the field names on `crate::domain::graph::GraphRelations`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationKind {
    DependsOn,
    Supersedes,
    RelatedTo,
}

impl RelationKind {
    /// Canonical wire/serde name for this kind. Matches the serde rename and
    /// the presenters' formerly-duplicated `relation_kind_label` helpers.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DependsOn => "depends_on",
            Self::Supersedes => "supersedes",
            Self::RelatedTo => "related_to",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn body_variant_serializes_with_snake_case_tag_and_before_after() {
        let change = FieldChange::Body {
            before: "old".to_string(),
            after: "new".to_string(),
        };

        let value = serde_json::to_value(&change).expect("FieldChange serializes");

        assert_eq!(
            value,
            json!({ "type": "body", "before": "old", "after": "new" })
        );
    }

    #[test]
    fn status_variant_carries_optional_before_after() {
        let change = FieldChange::Status {
            before: Some("draft".to_string()),
            after: Some("verified".to_string()),
        };

        let value = serde_json::to_value(&change).expect("FieldChange serializes");

        assert_eq!(value["type"], "status");
        assert_eq!(value["before"], "draft");
        assert_eq!(value["after"], "verified");
    }

    #[test]
    fn severity_variant_serializes_with_snake_case_tag_and_optional_sides() {
        let change = FieldChange::Severity {
            before: Some("high".to_string()),
            after: Some("critical".to_string()),
        };

        let value = serde_json::to_value(&change).expect("FieldChange serializes");

        assert_eq!(value["type"], "severity");
        assert_eq!(value["before"], "high");
        assert_eq!(value["after"], "critical");
    }

    #[test]
    fn owner_variant_serializes_with_owner_tag_and_optional_sides() {
        let change = FieldChange::Owner {
            before: None,
            after: Some("team-billing".to_string()),
        };

        let value = serde_json::to_value(&change).expect("FieldChange serializes");

        assert_eq!(value["type"], "owner");
        assert!(value["before"].is_null());
        assert_eq!(value["after"], "team-billing");
    }

    #[test]
    fn verified_at_variant_serializes_with_snake_case_tag() {
        let change = FieldChange::VerifiedAt {
            before: Some("2026-05-05".to_string()),
            after: None,
        };

        let value = serde_json::to_value(&change).expect("FieldChange serializes");

        assert_eq!(value["type"], "verified_at");
        assert_eq!(value["before"], "2026-05-05");
        assert!(value["after"].is_null());
    }

    #[test]
    fn evidence_added_variant_carries_field_and_value() {
        let change = FieldChange::EvidenceAdded {
            field: "source".to_string(),
            value: "ledger".to_string(),
        };

        let value = serde_json::to_value(&change).expect("FieldChange serializes");

        assert_eq!(
            value,
            json!({ "type": "evidence_added", "field": "source", "value": "ledger" })
        );
    }

    #[test]
    fn evidence_removed_variant_serializes_with_evidence_removed_tag() {
        let change = FieldChange::EvidenceRemoved {
            field: "test".to_string(),
            value: "integration".to_string(),
        };

        let value = serde_json::to_value(&change).expect("FieldChange serializes");

        assert_eq!(value["type"], "evidence_removed");
        assert_eq!(value["field"], "test");
        assert_eq!(value["value"], "integration");
    }

    #[test]
    fn relation_added_variant_serializes_kind_in_snake_case() {
        let change = FieldChange::RelationAdded {
            kind: RelationKind::DependsOn,
            target: "billing.payments".to_string(),
        };

        let value = serde_json::to_value(&change).expect("FieldChange serializes");

        assert_eq!(
            value,
            json!({
                "type": "relation_added",
                "kind": "depends_on",
                "target": "billing.payments",
            })
        );
    }

    #[test]
    fn relation_removed_variant_carries_kind_and_target() {
        let change = FieldChange::RelationRemoved {
            kind: RelationKind::Supersedes,
            target: "billing.legacy-credits".to_string(),
        };

        let value = serde_json::to_value(&change).expect("FieldChange serializes");

        assert_eq!(value["type"], "relation_removed");
        assert_eq!(value["kind"], "supersedes");
        assert_eq!(value["target"], "billing.legacy-credits");
    }

    #[test]
    fn relation_kind_related_to_serializes_in_snake_case() {
        let value = serde_json::to_value(RelationKind::RelatedTo).expect("RelationKind serializes");

        assert_eq!(value, json!("related_to"));
    }

    #[test]
    fn impacts_added_variant_serializes_with_snake_case_tag() {
        let change = FieldChange::ImpactsAdded {
            path: "crates/billing/src/refund.rs".to_string(),
        };

        let value = serde_json::to_value(&change).expect("FieldChange serializes");

        assert_eq!(
            value,
            json!({ "type": "impacts_added", "path": "crates/billing/src/refund.rs" })
        );
    }

    #[test]
    fn impacts_removed_variant_serializes_with_snake_case_tag() {
        let change = FieldChange::ImpactsRemoved {
            path: "src/old.rs".to_string(),
        };

        let value = serde_json::to_value(&change).expect("FieldChange serializes");

        assert_eq!(
            value,
            json!({ "type": "impacts_removed", "path": "src/old.rs" })
        );
    }

    #[test]
    fn summary_label_covers_every_v3_variant() {
        let cases = [
            (
                FieldChange::Body {
                    before: String::new(),
                    after: String::new(),
                },
                "body changed",
            ),
            (
                FieldChange::Status {
                    before: None,
                    after: None,
                },
                "status changed",
            ),
            (
                FieldChange::Owner {
                    before: None,
                    after: None,
                },
                "owner changed",
            ),
            (
                FieldChange::VerifiedAt {
                    before: None,
                    after: None,
                },
                "verified_at changed",
            ),
            (
                FieldChange::EvidenceAdded {
                    field: String::new(),
                    value: String::new(),
                },
                "evidence added",
            ),
            (
                FieldChange::EvidenceRemoved {
                    field: String::new(),
                    value: String::new(),
                },
                "evidence removed",
            ),
            (
                FieldChange::RelationAdded {
                    kind: RelationKind::DependsOn,
                    target: String::new(),
                },
                "relation added",
            ),
            (
                FieldChange::RelationRemoved {
                    kind: RelationKind::DependsOn,
                    target: String::new(),
                },
                "relation removed",
            ),
            (
                FieldChange::ImpactsAdded {
                    path: String::new(),
                },
                "impacts added",
            ),
            (
                FieldChange::ImpactsRemoved {
                    path: String::new(),
                },
                "impacts removed",
            ),
        ];
        for (change, expected) in cases {
            assert_eq!(change.summary_label(), expected);
        }
    }

    #[test]
    fn display_renders_each_variant_in_canonical_form() {
        assert_eq!(
            FieldChange::Body {
                before: "old".to_string(),
                after: "new".to_string(),
            }
            .to_string(),
            "body: changed"
        );
        assert_eq!(
            FieldChange::Status {
                before: Some("draft".to_string()),
                after: Some("verified".to_string()),
            }
            .to_string(),
            "status: draft → verified"
        );
        assert_eq!(
            FieldChange::Owner {
                before: Some("team-billing".to_string()),
                after: None,
            }
            .to_string(),
            "owner: team-billing → (none)"
        );
        assert_eq!(
            FieldChange::VerifiedAt {
                before: None,
                after: Some("2026-05-26".to_string()),
            }
            .to_string(),
            "verified_at: (none) → 2026-05-26"
        );
        assert_eq!(
            FieldChange::EvidenceAdded {
                field: "source".to_string(),
                value: "ledger".to_string(),
            }
            .to_string(),
            "evidence: +source"
        );
        assert_eq!(
            FieldChange::EvidenceRemoved {
                field: "test".to_string(),
                value: "integration".to_string(),
            }
            .to_string(),
            "evidence: -test"
        );
        assert_eq!(
            FieldChange::RelationAdded {
                kind: RelationKind::DependsOn,
                target: "billing.payments".to_string(),
            }
            .to_string(),
            "depends_on: +billing.payments"
        );
        assert_eq!(
            FieldChange::RelationRemoved {
                kind: RelationKind::Supersedes,
                target: "billing.legacy".to_string(),
            }
            .to_string(),
            "supersedes: -billing.legacy"
        );
        assert_eq!(
            FieldChange::ImpactsAdded {
                path: "src/foo.rs".to_string(),
            }
            .to_string(),
            "impacts: +src/foo.rs"
        );
        assert_eq!(
            FieldChange::ImpactsRemoved {
                path: "src/bar.rs".to_string(),
            }
            .to_string(),
            "impacts: -src/bar.rs"
        );
    }

    #[test]
    fn relation_kind_as_str_matches_serde_repr() {
        for (kind, expected) in [
            (RelationKind::DependsOn, "depends_on"),
            (RelationKind::Supersedes, "supersedes"),
            (RelationKind::RelatedTo, "related_to"),
        ] {
            assert_eq!(kind.as_str(), expected);
            assert_eq!(
                serde_json::to_value(kind).expect("RelationKind serializes"),
                json!(expected)
            );
        }
    }

    #[test]
    fn cloned_field_change_equals_original() {
        let original = FieldChange::Body {
            before: "a".to_string(),
            after: "b".to_string(),
        };
        let cloned = original.clone();

        assert_eq!(original, cloned);
    }

    #[test]
    fn effective_at_variant_serializes_with_snake_case_tag() {
        let change = FieldChange::EffectiveAt {
            before: Some("2026-01-01".to_string()),
            after: None,
        };

        let value = serde_json::to_value(&change).expect("FieldChange serializes");

        assert_eq!(value["type"], "effective_at");
        assert_eq!(value["before"], "2026-01-01");
        assert!(value["after"].is_null());
    }

    #[test]
    fn approved_by_added_variant_serializes_with_snake_case_tag() {
        let change = FieldChange::ApprovedByAdded {
            value: "security-lead".to_string(),
        };

        let value = serde_json::to_value(&change).expect("FieldChange serializes");

        assert_eq!(
            value,
            json!({ "type": "approved_by_added", "value": "security-lead" })
        );
    }

    #[test]
    fn approved_by_removed_variant_serializes_with_snake_case_tag() {
        let change = FieldChange::ApprovedByRemoved {
            value: "old-approver".to_string(),
        };

        let value = serde_json::to_value(&change).expect("FieldChange serializes");

        assert_eq!(
            value,
            json!({ "type": "approved_by_removed", "value": "old-approver" })
        );
    }

    #[test]
    fn summary_label_covers_v5_4_policy_variants() {
        let cases = [
            (
                FieldChange::EffectiveAt {
                    before: None,
                    after: None,
                },
                "effective_at changed",
            ),
            (
                FieldChange::ApprovedByAdded {
                    value: String::new(),
                },
                "approved_by added",
            ),
            (
                FieldChange::ApprovedByRemoved {
                    value: String::new(),
                },
                "approved_by removed",
            ),
        ];
        for (change, expected) in cases {
            assert_eq!(change.summary_label(), expected);
        }
    }

    #[test]
    fn display_renders_v5_4_policy_variants_in_canonical_form() {
        assert_eq!(
            FieldChange::EffectiveAt {
                before: None,
                after: Some("2026-06-01".to_string()),
            }
            .to_string(),
            "effective_at: (none) → 2026-06-01"
        );
        assert_eq!(
            FieldChange::EffectiveAt {
                before: Some("2026-01-01".to_string()),
                after: Some("2026-06-01".to_string()),
            }
            .to_string(),
            "effective_at: 2026-01-01 → 2026-06-01"
        );
        assert_eq!(
            FieldChange::ApprovedByAdded {
                value: "security-lead".to_string(),
            }
            .to_string(),
            "approved_by: +security-lead"
        );
        assert_eq!(
            FieldChange::ApprovedByRemoved {
                value: "old-approver".to_string(),
            }
            .to_string(),
            "approved_by: -old-approver"
        );
    }

    // ── V5.5 agent_instruction variants ───────────────────────────────────

    #[test]
    fn trust_variant_serializes_with_snake_case_tag() {
        let change = FieldChange::Trust {
            before: Some("team".to_string()),
            after: Some("authoritative".to_string()),
        };

        let value = serde_json::to_value(&change).expect("FieldChange serializes");

        assert_eq!(value["type"], "trust");
        assert_eq!(value["before"], "team");
        assert_eq!(value["after"], "authoritative");
    }

    #[test]
    fn scope_variant_serializes_with_snake_case_tag() {
        let change = FieldChange::Scope {
            before: Some("docs/auth/*".to_string()),
            after: Some("docs/**".to_string()),
        };

        let value = serde_json::to_value(&change).expect("FieldChange serializes");

        assert_eq!(value["type"], "scope");
        assert_eq!(value["before"], "docs/auth/*");
        assert_eq!(value["after"], "docs/**");
    }

    #[test]
    fn allowed_actions_added_variant_serializes_with_snake_case_tag() {
        let change = FieldChange::AllowedActionsAdded {
            value: "summarize".to_string(),
        };

        let value = serde_json::to_value(&change).expect("FieldChange serializes");

        assert_eq!(
            value,
            json!({ "type": "allowed_actions_added", "value": "summarize" })
        );
    }

    #[test]
    fn allowed_actions_removed_variant_serializes_with_snake_case_tag() {
        let change = FieldChange::AllowedActionsRemoved {
            value: "cite".to_string(),
        };

        let value = serde_json::to_value(&change).expect("FieldChange serializes");

        assert_eq!(
            value,
            json!({ "type": "allowed_actions_removed", "value": "cite" })
        );
    }

    #[test]
    fn forbidden_actions_added_variant_serializes_with_snake_case_tag() {
        let change = FieldChange::ForbiddenActionsAdded {
            value: "execute_shell".to_string(),
        };

        let value = serde_json::to_value(&change).expect("FieldChange serializes");

        assert_eq!(
            value,
            json!({ "type": "forbidden_actions_added", "value": "execute_shell" })
        );
    }

    #[test]
    fn forbidden_actions_removed_variant_serializes_with_snake_case_tag() {
        let change = FieldChange::ForbiddenActionsRemoved {
            value: "access_secrets".to_string(),
        };

        let value = serde_json::to_value(&change).expect("FieldChange serializes");

        assert_eq!(
            value,
            json!({ "type": "forbidden_actions_removed", "value": "access_secrets" })
        );
    }

    #[test]
    fn summary_label_covers_v5_5_agent_instruction_variants() {
        let cases = [
            (
                FieldChange::Trust {
                    before: None,
                    after: None,
                },
                "trust changed",
            ),
            (
                FieldChange::Scope {
                    before: None,
                    after: None,
                },
                "scope changed",
            ),
            (
                FieldChange::AllowedActionsAdded {
                    value: String::new(),
                },
                "allowed_actions added",
            ),
            (
                FieldChange::AllowedActionsRemoved {
                    value: String::new(),
                },
                "allowed_actions removed",
            ),
            (
                FieldChange::ForbiddenActionsAdded {
                    value: String::new(),
                },
                "forbidden_actions added",
            ),
            (
                FieldChange::ForbiddenActionsRemoved {
                    value: String::new(),
                },
                "forbidden_actions removed",
            ),
        ];
        for (change, expected) in cases {
            assert_eq!(change.summary_label(), expected);
        }
    }

    // ── V5.6 contradiction variants ────────────────────────────────────────

    #[test]
    fn contradiction_claims_added_variant_serializes_with_snake_case_tag() {
        let change = FieldChange::ContradictionClaimsAdded {
            value: "auth.a".to_string(),
        };

        let value = serde_json::to_value(&change).expect("FieldChange serializes");

        assert_eq!(
            value,
            json!({ "type": "contradiction_claims_added", "value": "auth.a" })
        );
    }

    #[test]
    fn contradiction_claims_removed_variant_serializes_with_snake_case_tag() {
        let change = FieldChange::ContradictionClaimsRemoved {
            value: "auth.b".to_string(),
        };

        let value = serde_json::to_value(&change).expect("FieldChange serializes");

        assert_eq!(
            value,
            json!({ "type": "contradiction_claims_removed", "value": "auth.b" })
        );
    }

    #[test]
    fn summary_label_covers_v5_6_contradiction_variants() {
        let cases = [
            (
                FieldChange::ContradictionClaimsAdded {
                    value: String::new(),
                },
                "contradiction_claims added",
            ),
            (
                FieldChange::ContradictionClaimsRemoved {
                    value: String::new(),
                },
                "contradiction_claims removed",
            ),
        ];
        for (change, expected) in cases {
            assert_eq!(change.summary_label(), expected);
        }
    }

    #[test]
    fn display_renders_v5_6_contradiction_variants_in_canonical_form() {
        assert_eq!(
            FieldChange::ContradictionClaimsAdded {
                value: "auth.a".to_string(),
            }
            .to_string(),
            "contradiction_claims: +auth.a"
        );
        assert_eq!(
            FieldChange::ContradictionClaimsRemoved {
                value: "auth.b".to_string(),
            }
            .to_string(),
            "contradiction_claims: -auth.b"
        );
    }

    // ── V6.5.1 api variants ────────────────────────────────────────────────

    #[test]
    fn api_method_variant_serializes_with_snake_case_tag() {
        let change = FieldChange::ApiMethod {
            before: Some("POST".to_string()),
            after: Some("PUT".to_string()),
        };

        let value = serde_json::to_value(&change).expect("FieldChange serializes");

        assert_eq!(value["type"], "api_method");
        assert_eq!(value["before"], "POST");
        assert_eq!(value["after"], "PUT");
    }

    #[test]
    fn api_path_variant_serializes_with_snake_case_tag() {
        let change = FieldChange::ApiPath {
            before: Some("/api/v1/consume".to_string()),
            after: Some("/api/v2/consume".to_string()),
        };

        let value = serde_json::to_value(&change).expect("FieldChange serializes");

        assert_eq!(value["type"], "api_path");
        assert_eq!(value["before"], "/api/v1/consume");
        assert_eq!(value["after"], "/api/v2/consume");
    }

    #[test]
    fn summary_label_and_display_cover_v6_5_api_variants() {
        assert_eq!(
            FieldChange::ApiMethod {
                before: None,
                after: None,
            }
            .summary_label(),
            "method changed"
        );
        assert_eq!(
            FieldChange::ApiPath {
                before: None,
                after: None,
            }
            .summary_label(),
            "path changed"
        );
        assert_eq!(
            FieldChange::ApiMethod {
                before: Some("POST".to_string()),
                after: Some("PUT".to_string()),
            }
            .to_string(),
            "method: POST → PUT"
        );
        assert_eq!(
            FieldChange::ApiPath {
                before: Some("/a".to_string()),
                after: Some("/b".to_string()),
            }
            .to_string(),
            "path: /a → /b"
        );
    }

    #[test]
    fn display_renders_v5_5_agent_instruction_variants_in_canonical_form() {
        assert_eq!(
            FieldChange::Trust {
                before: Some("team".to_string()),
                after: Some("authoritative".to_string()),
            }
            .to_string(),
            "trust: team → authoritative"
        );
        assert_eq!(
            FieldChange::Trust {
                before: None,
                after: Some("team".to_string()),
            }
            .to_string(),
            "trust: (none) → team"
        );
        assert_eq!(
            FieldChange::Scope {
                before: Some("docs/auth/*".to_string()),
                after: Some("docs/**".to_string()),
            }
            .to_string(),
            "scope: docs/auth/* → docs/**"
        );
        assert_eq!(
            FieldChange::AllowedActionsAdded {
                value: "summarize".to_string(),
            }
            .to_string(),
            "allowed_actions: +summarize"
        );
        assert_eq!(
            FieldChange::AllowedActionsRemoved {
                value: "cite".to_string(),
            }
            .to_string(),
            "allowed_actions: -cite"
        );
        assert_eq!(
            FieldChange::ForbiddenActionsAdded {
                value: "execute_shell".to_string(),
            }
            .to_string(),
            "forbidden_actions: +execute_shell"
        );
        assert_eq!(
            FieldChange::ForbiddenActionsRemoved {
                value: "access_secrets".to_string(),
            }
            .to_string(),
            "forbidden_actions: -access_secrets"
        );
    }
}
