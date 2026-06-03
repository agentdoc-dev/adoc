//! V3.2 field-level projection over an [`ObjectChange::Changed`] entry.
//!
//! Pure domain projection — no I/O, no allocation outside `Vec<FieldChange>`
//! and the strings the projection owns. Promoted out of `application::review`
//! in Step E1 so the projection lives with the `FieldChange` vocabulary it
//! produces (`domain/review/field_change.rs`).
//!
//! `application::review::diff_objects` decorates each `Changed` entry by
//! calling [`project_changed`] inline; the obligation trigger table in
//! `domain::review::obligation_rules` then reads the decorated
//! `field_changes` slice off the entry.
//!
//! See V3-DESIGN.md §V3.2 for the variant list and §"Boundary Invariants"
//! for the set-diff (not list-diff) semantics on relations and impacts.

use std::collections::BTreeSet;

use crate::domain::knowledge_object::BlockKind;
use crate::domain::knowledge_object::metadata::KnowledgeObjectMetadata;

const EFFECTIVE_AT_FIELD: &str = "effective_at";
const SCOPE_FIELD: &str = "scope";

use super::field_change::{FieldChange, RelationKind};
use super::object_change::ChangedObject;

/// Project the differences between `c.base` and `c.head` into the V3.2
/// `FieldChange` vocabulary.
///
/// `Created` and `Deleted` entries project to the empty vector — the full
/// before/after record already lives in the diff envelope; we only enumerate
/// field-level *deltas* on `Changed` rows.
pub(crate) fn project_changed(c: &ChangedObject) -> Vec<FieldChange> {
    let mut out = Vec::new();
    let base = &c.base;
    let head = &c.head;
    let base_meta = KnowledgeObjectMetadata::from_node(base);
    let head_meta = KnowledgeObjectMetadata::from_node(head);

    if base.body != head.body {
        out.push(FieldChange::Body {
            before: base.body.clone(),
            after: head.body.clone(),
        });
    }

    if base.status != head.status {
        let before = base.status.clone();
        let after = head.status.clone();
        // Some kinds repurpose the graph node's `status` slot for a typed
        // discriminant rather than a lifecycle status: `constraint` stores its
        // shared `Severity`, and `agent_instruction` stores its `Trust`. Project
        // the delta under the matching variant so the change is not mislabelled
        // as a lifecycle Status change (and not double-counted below).
        out.push(if head.kind.as_str() == BlockKind::Constraint.as_str() {
            FieldChange::Severity { before, after }
        } else if head.kind.as_str() == BlockKind::AgentInstruction.as_str() {
            FieldChange::Trust { before, after }
        } else {
            FieldChange::Status { before, after }
        });
    }

    if base_meta.owner != head_meta.owner {
        out.push(FieldChange::Owner {
            before: base_meta.owner.map(str::to_string),
            after: head_meta.owner.map(str::to_string),
        });
    }

    if base_meta.verified_at != head_meta.verified_at {
        out.push(FieldChange::VerifiedAt {
            before: base_meta.verified_at.map(str::to_string),
            after: head_meta.verified_at.map(str::to_string),
        });
    }

    // Strict presence/absence on evidence kinds. A value-only change to
    // an evidence entry emits nothing — consumers see the diff in the full
    // before/after records, and V3.4's "Evidence removal → re-evidence"
    // obligation rule must not fire on edits that only update the value.
    // We build per-kind presence maps and diff them by kind string.
    {
        use std::collections::BTreeMap;
        let base_ev: BTreeMap<&str, &str> = base_meta
            .evidence
            .iter()
            .filter_map(|(k, v)| v.map(|val| (*k, val)))
            .collect();
        let head_ev: BTreeMap<&str, &str> = head_meta
            .evidence
            .iter()
            .filter_map(|(k, v)| v.map(|val| (*k, val)))
            .collect();
        // Kinds in head but not in base → added.
        for (kind, value) in &head_ev {
            if !base_ev.contains_key(kind) {
                out.push(FieldChange::EvidenceAdded {
                    field: (*kind).to_string(),
                    value: (*value).to_string(),
                });
            }
        }
        // Kinds in base but not in head → removed.
        for (kind, value) in &base_ev {
            if !head_ev.contains_key(kind) {
                out.push(FieldChange::EvidenceRemoved {
                    field: (*kind).to_string(),
                    value: (*value).to_string(),
                });
            }
        }
    }

    project_relation(
        &mut out,
        RelationKind::DependsOn,
        &base.relations.depends_on,
        &head.relations.depends_on,
    );
    project_relation(
        &mut out,
        RelationKind::Supersedes,
        &base.relations.supersedes,
        &head.relations.supersedes,
    );
    project_relation(
        &mut out,
        RelationKind::RelatedTo,
        &base.relations.related_to,
        &head.relations.related_to,
    );

    project_impacts(&mut out, &base.impacts, &head.impacts);

    let base_effective = base.fields.get(EFFECTIVE_AT_FIELD).map(String::as_str);
    let head_effective = head.fields.get(EFFECTIVE_AT_FIELD).map(String::as_str);
    if base_effective != head_effective {
        out.push(FieldChange::EffectiveAt {
            before: base_effective.map(str::to_string),
            after: head_effective.map(str::to_string),
        });
    }

    project_approved_by(&mut out, &base.approved_by, &head.approved_by);

    // V5.5: agent_instruction scope scalar diff and action-set diffs. `scope`
    // lives in the graph fields map (unlike `trust`, which rides the `status`
    // slot and is projected as `FieldChange::Trust` above).
    let base_scope = base.fields.get(SCOPE_FIELD).map(String::as_str);
    let head_scope = head.fields.get(SCOPE_FIELD).map(String::as_str);
    if base_scope != head_scope {
        out.push(FieldChange::Scope {
            before: base_scope.map(str::to_string),
            after: head_scope.map(str::to_string),
        });
    }

    project_action_list(
        &mut out,
        &base.allowed_actions,
        &head.allowed_actions,
        |value| FieldChange::AllowedActionsAdded { value },
        |value| FieldChange::AllowedActionsRemoved { value },
    );
    project_action_list(
        &mut out,
        &base.forbidden_actions,
        &head.forbidden_actions,
        |value| FieldChange::ForbiddenActionsAdded { value },
        |value| FieldChange::ForbiddenActionsRemoved { value },
    );

    project_contradiction_claims(
        &mut out,
        &base.contradiction_claims,
        &head.contradiction_claims,
    );

    out
}

fn project_impacts(out: &mut Vec<FieldChange>, base: &[String], head: &[String]) {
    let base_set: BTreeSet<&str> = base.iter().map(String::as_str).collect();
    let head_set: BTreeSet<&str> = head.iter().map(String::as_str).collect();
    for path in head_set.difference(&base_set) {
        out.push(FieldChange::ImpactsAdded {
            path: (*path).to_string(),
        });
    }
    for path in base_set.difference(&head_set) {
        out.push(FieldChange::ImpactsRemoved {
            path: (*path).to_string(),
        });
    }
}

fn project_contradiction_claims(out: &mut Vec<FieldChange>, base: &[String], head: &[String]) {
    let base_set: BTreeSet<&str> = base.iter().map(String::as_str).collect();
    let head_set: BTreeSet<&str> = head.iter().map(String::as_str).collect();
    for value in head_set.difference(&base_set) {
        out.push(FieldChange::ContradictionClaimsAdded {
            value: (*value).to_string(),
        });
    }
    for value in base_set.difference(&head_set) {
        out.push(FieldChange::ContradictionClaimsRemoved {
            value: (*value).to_string(),
        });
    }
}

fn project_approved_by(out: &mut Vec<FieldChange>, base: &[String], head: &[String]) {
    let base_set: BTreeSet<&str> = base.iter().map(String::as_str).collect();
    let head_set: BTreeSet<&str> = head.iter().map(String::as_str).collect();
    for value in head_set.difference(&base_set) {
        out.push(FieldChange::ApprovedByAdded {
            value: (*value).to_string(),
        });
    }
    for value in base_set.difference(&head_set) {
        out.push(FieldChange::ApprovedByRemoved {
            value: (*value).to_string(),
        });
    }
}

fn project_action_list(
    out: &mut Vec<FieldChange>,
    base: &[String],
    head: &[String],
    added_ctor: impl Fn(String) -> FieldChange,
    removed_ctor: impl Fn(String) -> FieldChange,
) {
    let base_set: BTreeSet<&str> = base.iter().map(String::as_str).collect();
    let head_set: BTreeSet<&str> = head.iter().map(String::as_str).collect();
    for value in head_set.difference(&base_set) {
        out.push(added_ctor((*value).to_string()));
    }
    for value in base_set.difference(&head_set) {
        out.push(removed_ctor((*value).to_string()));
    }
}

fn project_relation(
    out: &mut Vec<FieldChange>,
    kind: RelationKind,
    base: &[String],
    head: &[String],
) {
    let base_set: BTreeSet<&str> = base.iter().map(String::as_str).collect();
    let head_set: BTreeSet<&str> = head.iter().map(String::as_str).collect();
    for target in head_set.difference(&base_set) {
        out.push(FieldChange::RelationAdded {
            kind,
            target: (*target).to_string(),
        });
    }
    for target in base_set.difference(&head_set) {
        out.push(FieldChange::RelationRemoved {
            kind,
            target: (*target).to_string(),
        });
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::project_changed;
    use crate::domain::graph::{
        GraphEvidence, GraphKnowledgeObjectNode, GraphRelations, GraphSourceSpan,
    };
    use crate::domain::review::field_change::{FieldChange, RelationKind};
    use crate::domain::review::object_change::ChangedObject;

    fn node(
        id: &str,
        content_hash: &str,
        body: &str,
        status: Option<&str>,
        fields: BTreeMap<String, String>,
        relations: GraphRelations,
    ) -> GraphKnowledgeObjectNode {
        GraphKnowledgeObjectNode {
            id: id.to_string(),
            kind: "claim".to_string(),
            content_hash: content_hash.to_string(),
            status: status.map(str::to_string),
            body: body.to_string(),
            page_id: "team.billing".to_string(),
            source_span: GraphSourceSpan {
                path: "docs/billing.adoc".to_string(),
                line: 1,
                column: 1,
            },
            fields,
            relations,
            impacts: Vec::new(),
            approved_by: Vec::new(),
            allowed_actions: Vec::new(),
            forbidden_actions: Vec::new(),
            contradiction_claims: Vec::new(),
            evidence: Vec::new(),
            effective_status: None,
            effective_reason: None,
        }
    }

    fn baseline(body: &str) -> GraphKnowledgeObjectNode {
        node(
            "billing.credits",
            "sha256:base",
            body,
            Some("draft"),
            BTreeMap::new(),
            GraphRelations::default(),
        )
    }

    fn changed_from(
        base: GraphKnowledgeObjectNode,
        head: GraphKnowledgeObjectNode,
    ) -> ChangedObject {
        ChangedObject::new("billing.credits".to_string(), base, head)
    }

    #[test]
    fn identical_records_produce_empty_projection() {
        let base = baseline("Credits apply.");
        let head = baseline("Credits apply.");
        let c = changed_from(base, head);

        assert!(project_changed(&c).is_empty());
    }

    #[test]
    fn body_only_change_produces_exactly_one_body_field_change() {
        let base = baseline("Old.");
        let head = baseline("New.");
        let c = changed_from(base, head);

        assert_eq!(
            project_changed(&c),
            vec![FieldChange::Body {
                before: "Old.".to_string(),
                after: "New.".to_string(),
            }]
        );
    }

    #[test]
    fn status_change_emits_status_field_change_with_optional_sides() {
        let base = node(
            "billing.credits",
            "sha256:a",
            "x",
            Some("draft"),
            BTreeMap::new(),
            GraphRelations::default(),
        );
        let head = node(
            "billing.credits",
            "sha256:b",
            "x",
            Some("verified"),
            BTreeMap::new(),
            GraphRelations::default(),
        );

        assert_eq!(
            project_changed(&changed_from(base, head)),
            vec![FieldChange::Status {
                before: Some("draft".to_string()),
                after: Some("verified".to_string()),
            }]
        );
    }

    #[test]
    fn constraint_severity_change_emits_severity_field_change_not_status() {
        let constraint_node = |content_hash: &str, severity: &str| GraphKnowledgeObjectNode {
            id: "auth.session.no-local-storage".to_string(),
            kind: "constraint".to_string(),
            content_hash: content_hash.to_string(),
            status: Some(severity.to_string()),
            body: "Session tokens must not be stored in localStorage.".to_string(),
            page_id: "team.auth".to_string(),
            source_span: GraphSourceSpan {
                path: "docs/auth.adoc".to_string(),
                line: 1,
                column: 1,
            },
            fields: BTreeMap::new(),
            relations: GraphRelations::default(),
            impacts: Vec::new(),
            approved_by: Vec::new(),
            allowed_actions: Vec::new(),
            forbidden_actions: Vec::new(),
            contradiction_claims: Vec::new(),
            evidence: Vec::new(),
            effective_status: None,
            effective_reason: None,
        };

        let base = constraint_node("sha256:a", "high");
        let head = constraint_node("sha256:b", "critical");

        assert_eq!(
            project_changed(&ChangedObject::new(
                "auth.session.no-local-storage".to_string(),
                base,
                head,
            )),
            vec![FieldChange::Severity {
                before: Some("high".to_string()),
                after: Some("critical".to_string()),
            }]
        );
    }

    #[test]
    fn agent_instruction_trust_change_emits_only_trust_field_change_not_status() {
        // `trust` rides the `status` slot via the discriminant projection; a
        // trust change must surface as exactly one `Trust` variant — never a
        // mislabelled `Status` change, and never both.
        let agent_node = |content_hash: &str, trust: &str| GraphKnowledgeObjectNode {
            id: "auth.docs-answering-policy".to_string(),
            kind: "agent_instruction".to_string(),
            content_hash: content_hash.to_string(),
            status: Some(trust.to_string()),
            body: "Prefer verified claims over draft notes.".to_string(),
            page_id: "team.auth".to_string(),
            source_span: GraphSourceSpan {
                path: "docs/auth.adoc".to_string(),
                line: 1,
                column: 1,
            },
            fields: BTreeMap::new(),
            relations: GraphRelations::default(),
            impacts: Vec::new(),
            approved_by: Vec::new(),
            allowed_actions: vec!["summarize".to_string()],
            forbidden_actions: vec!["execute_shell".to_string()],
            contradiction_claims: Vec::new(),
            evidence: Vec::new(),
            effective_status: None,
            effective_reason: None,
        };

        let base = agent_node("sha256:a", "team");
        let head = agent_node("sha256:b", "authoritative");

        assert_eq!(
            project_changed(&ChangedObject::new(
                "auth.docs-answering-policy".to_string(),
                base,
                head,
            )),
            vec![FieldChange::Trust {
                before: Some("team".to_string()),
                after: Some("authoritative".to_string()),
            }]
        );
    }

    #[test]
    fn status_appearance_from_none_to_some_emits_status_field_change() {
        let base = node(
            "billing.credits",
            "sha256:a",
            "x",
            None,
            BTreeMap::new(),
            GraphRelations::default(),
        );
        let head = node(
            "billing.credits",
            "sha256:b",
            "x",
            Some("draft"),
            BTreeMap::new(),
            GraphRelations::default(),
        );

        assert_eq!(
            project_changed(&changed_from(base, head)),
            vec![FieldChange::Status {
                before: None,
                after: Some("draft".to_string()),
            }]
        );
    }

    #[test]
    fn owner_appearance_emits_owner_field_change_with_none_before() {
        let base = baseline("x");
        let mut head = baseline("x");
        head.fields
            .insert("owner".to_string(), "team-billing".to_string());

        assert_eq!(
            project_changed(&changed_from(base, head)),
            vec![FieldChange::Owner {
                before: None,
                after: Some("team-billing".to_string()),
            }]
        );
    }

    #[test]
    fn verified_at_removal_emits_verified_at_field_change_with_none_after() {
        let mut base = baseline("x");
        base.fields
            .insert("verified_at".to_string(), "2026-05-05".to_string());
        let head = baseline("x");

        assert_eq!(
            project_changed(&changed_from(base, head)),
            vec![FieldChange::VerifiedAt {
                before: Some("2026-05-05".to_string()),
                after: None,
            }]
        );
    }

    #[test]
    fn evidence_added_when_source_code_appears_in_head() {
        let base = baseline("x");
        let mut head = baseline("x");
        head.evidence
            .push(GraphEvidence::inline("source_code", "ledger"));

        assert_eq!(
            project_changed(&changed_from(base, head)),
            vec![FieldChange::EvidenceAdded {
                field: "source_code".to_string(),
                value: "ledger".to_string(),
            }]
        );
    }

    #[test]
    fn evidence_removed_when_test_disappears_in_head() {
        let mut base = baseline("x");
        base.evidence
            .push(GraphEvidence::inline("test", "integration"));
        let head = baseline("x");

        assert_eq!(
            project_changed(&changed_from(base, head)),
            vec![FieldChange::EvidenceRemoved {
                field: "test".to_string(),
                value: "integration".to_string(),
            }]
        );
    }

    #[test]
    fn evidence_value_only_change_emits_no_field_change() {
        // Strict presence/absence semantics: source_code: A -> source_code: B is
        // not an EvidenceAdded/Removed and not an "EvidenceChanged"
        // (no such variant in V3.2). Consumers must read the full
        // before/after records if they care about value-only edits.
        let mut base = baseline("x");
        base.evidence
            .push(GraphEvidence::inline("source_code", "ledger-v1"));
        let mut head = baseline("x");
        head.evidence
            .push(GraphEvidence::inline("source_code", "ledger-v2"));

        assert!(project_changed(&changed_from(base, head)).is_empty());
    }

    #[test]
    fn relation_added_for_new_depends_on_target() {
        let base = baseline("x");
        let mut head = baseline("x");
        head.relations.depends_on = vec!["billing.payments".to_string()];

        assert_eq!(
            project_changed(&changed_from(base, head)),
            vec![FieldChange::RelationAdded {
                kind: RelationKind::DependsOn,
                target: "billing.payments".to_string(),
            }]
        );
    }

    #[test]
    fn relation_removed_for_dropped_supersedes_target() {
        let mut base = baseline("x");
        base.relations.supersedes = vec!["billing.legacy-credits".to_string()];
        let head = baseline("x");

        assert_eq!(
            project_changed(&changed_from(base, head)),
            vec![FieldChange::RelationRemoved {
                kind: RelationKind::Supersedes,
                target: "billing.legacy-credits".to_string(),
            }]
        );
    }

    #[test]
    fn related_to_relation_uses_related_to_kind() {
        let base = baseline("x");
        let mut head = baseline("x");
        head.relations.related_to = vec!["billing.holds".to_string()];

        assert_eq!(
            project_changed(&changed_from(base, head)),
            vec![FieldChange::RelationAdded {
                kind: RelationKind::RelatedTo,
                target: "billing.holds".to_string(),
            }]
        );
    }

    #[test]
    fn relation_array_reorder_with_same_set_produces_empty_projection() {
        let mut base = baseline("x");
        base.relations.depends_on = vec!["b.b".to_string(), "a.a".to_string()];
        let mut head = baseline("x");
        head.relations.depends_on = vec!["a.a".to_string(), "b.b".to_string()];

        assert!(project_changed(&changed_from(base, head)).is_empty());
    }

    #[test]
    fn relation_duplicate_entries_collapse_via_set_semantics() {
        let mut base = baseline("x");
        base.relations.depends_on = vec!["a.a".to_string(), "a.a".to_string()];
        let mut head = baseline("x");
        head.relations.depends_on = vec!["a.a".to_string()];

        assert!(project_changed(&changed_from(base, head)).is_empty());
    }

    #[test]
    fn impacts_added_when_path_appears_in_head() {
        let base = baseline("x");
        let mut head = baseline("x");
        head.impacts = vec!["a.rs".to_string()];

        assert_eq!(
            project_changed(&changed_from(base, head)),
            vec![FieldChange::ImpactsAdded {
                path: "a.rs".to_string(),
            }]
        );
    }

    #[test]
    fn impacts_removed_when_path_disappears_in_head() {
        let mut base = baseline("x");
        base.impacts = vec!["a.rs".to_string()];
        let head = baseline("x");

        assert_eq!(
            project_changed(&changed_from(base, head)),
            vec![FieldChange::ImpactsRemoved {
                path: "a.rs".to_string(),
            }]
        );
    }

    #[test]
    fn impacts_set_reorder_with_same_set_produces_empty_projection() {
        let mut base = baseline("x");
        base.impacts = vec!["a.rs".to_string(), "b.rs".to_string()];
        let mut head = baseline("x");
        head.impacts = vec!["b.rs".to_string(), "a.rs".to_string()];

        assert!(project_changed(&changed_from(base, head)).is_empty());
    }

    #[test]
    fn impacts_added_and_removed_emit_sorted_per_kind() {
        let mut base = baseline("x");
        base.impacts = vec!["keep.rs".to_string(), "drop.rs".to_string()];
        let mut head = baseline("x");
        head.impacts = vec!["keep.rs".to_string(), "add.rs".to_string()];

        assert_eq!(
            project_changed(&changed_from(base, head)),
            vec![
                FieldChange::ImpactsAdded {
                    path: "add.rs".to_string(),
                },
                FieldChange::ImpactsRemoved {
                    path: "drop.rs".to_string(),
                },
            ]
        );
    }

    #[test]
    fn multiple_changes_appear_in_deterministic_visit_order() {
        // Order: body, status, owner, verified_at, evidence (by kind),
        // relations (depends_on, supersedes, related_to).
        let base = node(
            "billing.credits",
            "sha256:a",
            "old",
            Some("draft"),
            BTreeMap::new(),
            GraphRelations::default(),
        );
        let mut head_fields = BTreeMap::new();
        head_fields.insert("owner".to_string(), "team-billing".to_string());
        let mut head = node(
            "billing.credits",
            "sha256:b",
            "new",
            Some("verified"),
            head_fields,
            GraphRelations {
                depends_on: vec!["billing.payments".to_string()],
                ..GraphRelations::default()
            },
        );
        head.evidence
            .push(GraphEvidence::inline("source_code", "ledger"));

        let changes = project_changed(&changed_from(base, head));

        assert_eq!(
            changes,
            vec![
                FieldChange::Body {
                    before: "old".to_string(),
                    after: "new".to_string(),
                },
                FieldChange::Status {
                    before: Some("draft".to_string()),
                    after: Some("verified".to_string()),
                },
                FieldChange::Owner {
                    before: None,
                    after: Some("team-billing".to_string()),
                },
                FieldChange::EvidenceAdded {
                    field: "source_code".to_string(),
                    value: "ledger".to_string(),
                },
                FieldChange::RelationAdded {
                    kind: RelationKind::DependsOn,
                    target: "billing.payments".to_string(),
                },
            ]
        );
    }

    #[test]
    fn effective_at_change_emits_one_effective_at_field_change() {
        let mut base = baseline("x");
        base.fields
            .insert("effective_at".to_string(), "2026-01-01".to_string());
        let mut head = baseline("x");
        head.fields
            .insert("effective_at".to_string(), "2026-06-01".to_string());

        assert_eq!(
            project_changed(&changed_from(base, head)),
            vec![FieldChange::EffectiveAt {
                before: Some("2026-01-01".to_string()),
                after: Some("2026-06-01".to_string()),
            }]
        );
    }

    #[test]
    fn effective_at_removal_emits_effective_at_field_change_with_none_after() {
        let mut base = baseline("x");
        base.fields
            .insert("effective_at".to_string(), "2026-01-01".to_string());
        let head = baseline("x");

        assert_eq!(
            project_changed(&changed_from(base, head)),
            vec![FieldChange::EffectiveAt {
                before: Some("2026-01-01".to_string()),
                after: None,
            }]
        );
    }

    #[test]
    fn approved_by_added_when_approver_appears_in_head() {
        let base = baseline("x");
        let mut head = baseline("x");
        head.approved_by = vec!["security-lead".to_string()];

        assert_eq!(
            project_changed(&changed_from(base, head)),
            vec![FieldChange::ApprovedByAdded {
                value: "security-lead".to_string(),
            }]
        );
    }

    #[test]
    fn approved_by_removed_when_approver_disappears_in_head() {
        let mut base = baseline("x");
        base.approved_by = vec!["security-lead".to_string()];
        let head = baseline("x");

        assert_eq!(
            project_changed(&changed_from(base, head)),
            vec![FieldChange::ApprovedByRemoved {
                value: "security-lead".to_string(),
            }]
        );
    }

    #[test]
    fn approved_by_reorder_with_same_set_produces_empty_projection() {
        let mut base = baseline("x");
        base.approved_by = vec!["approver-b".to_string(), "approver-a".to_string()];
        let mut head = baseline("x");
        head.approved_by = vec!["approver-a".to_string(), "approver-b".to_string()];

        assert!(project_changed(&changed_from(base, head)).is_empty());
    }

    #[test]
    fn object_diff_compute_populates_field_changes_inline() {
        // Domain-internal invariant: `ObjectDiff::compute` self-decorates
        // each `Changed` entry with its V3.2 field-change projection so
        // downstream callers (V3.4 obligations, V3.5 presenters) read off a
        // ready-to-use aggregate. Pre-decoration entries are now an
        // unreachable internal state of `compute`.
        let mut base = baseline("old");
        base.content_hash = "sha256:base-hash".to_string();
        let mut head = baseline("new");
        head.content_hash = "sha256:head-hash".to_string();

        let diff = crate::domain::review::object_diff::ObjectDiff::compute(
            std::slice::from_ref(&base),
            std::slice::from_ref(&head),
        );

        assert_eq!(diff.changed().len(), 1);
        assert_eq!(
            diff.changed()[0].field_changes(),
            &[FieldChange::Body {
                before: "old".to_string(),
                after: "new".to_string(),
            }]
        );
    }
}
