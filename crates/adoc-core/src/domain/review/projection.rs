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

use crate::domain::knowledge_object::metadata::KnowledgeObjectMetadata;

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
        out.push(FieldChange::Status {
            before: base.status.clone(),
            after: head.status.clone(),
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

    // Strict presence/absence on the V0 evidence keys. A value-only change to
    // an evidence field emits nothing — consumers see the diff in the full
    // before/after records, and V3.4's "Evidence removal → re-evidence"
    // obligation rule must not fire on edits that only update the value.
    for ((field, base_value), (_, head_value)) in
        base_meta.evidence.iter().zip(head_meta.evidence.iter())
    {
        match (base_value, head_value) {
            (None, Some(after)) => out.push(FieldChange::EvidenceAdded {
                field: field.as_str().to_string(),
                value: (*after).to_string(),
            }),
            (Some(before), None) => out.push(FieldChange::EvidenceRemoved {
                field: field.as_str().to_string(),
                value: (*before).to_string(),
            }),
            _ => {}
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
    use crate::domain::graph::{GraphKnowledgeObjectNode, GraphRelations, GraphSourceSpan};
    use crate::domain::knowledge_object::metadata::EvidenceField;
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
    fn evidence_added_when_source_appears_in_head() {
        let base = baseline("x");
        let mut head = baseline("x");
        head.fields
            .insert("source".to_string(), "ledger".to_string());

        assert_eq!(
            project_changed(&changed_from(base, head)),
            vec![FieldChange::EvidenceAdded {
                field: "source".to_string(),
                value: "ledger".to_string(),
            }]
        );
    }

    #[test]
    fn evidence_removed_when_test_disappears_in_head() {
        let mut base = baseline("x");
        base.fields
            .insert("test".to_string(), "integration".to_string());
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
        // Strict presence/absence semantics: source: A -> source: B is
        // not an EvidenceAdded/Removed and not an "EvidenceChanged"
        // (no such variant in V3.2). Consumers must read the full
        // before/after records if they care about value-only edits.
        let mut base = baseline("x");
        base.fields
            .insert("source".to_string(), "ledger-v1".to_string());
        let mut head = baseline("x");
        head.fields
            .insert("source".to_string(), "ledger-v2".to_string());

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
        // Order: body, status, owner, verified_at, evidence (source,
        // test, reviewed_by), relations (depends_on, supersedes,
        // related_to). Matches the documented visit order in
        // project_changed.
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
        head_fields.insert("source".to_string(), "ledger".to_string());
        let head = node(
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
                    field: "source".to_string(),
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
    fn evidence_field_canonical_order_matches_v0_wire_contract() {
        let wire: Vec<&'static str> = EvidenceField::ALL.iter().map(|f| f.as_str()).collect();
        assert_eq!(wire, vec!["source", "test", "reviewed_by"]);
    }

    #[test]
    fn diff_objects_decoration_step_populates_field_changes_on_each_changed_entry() {
        // Property test of the contract callers (application::review::diff_objects)
        // depend on: a freshly-computed ObjectDiff has empty field_changes
        // until decorated by project_changed.
        let mut base = baseline("old");
        base.content_hash = "sha256:base-hash".to_string();
        let mut head = baseline("new");
        head.content_hash = "sha256:head-hash".to_string();

        let diff = crate::domain::review::object_diff::ObjectDiff::compute(
            std::slice::from_ref(&base),
            std::slice::from_ref(&head),
        );

        // Un-decorated diff: decoration is the application layer's job.
        assert_eq!(diff.changed().len(), 1);
        assert!(diff.changed()[0].field_changes().is_empty());

        // Run the same decoration step diff_objects() performs.
        let mut decorated = diff;
        for entry in decorated.changed_mut() {
            entry.field_changes = project_changed(entry);
        }
        assert_eq!(
            decorated.changed()[0].field_changes(),
            &[FieldChange::Body {
                before: "old".to_string(),
                after: "new".to_string(),
            }]
        );
    }
}
