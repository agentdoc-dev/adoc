//! V3.3 source-path impact analysis.
//!
//! Given a mechanical [`ObjectDiff`] and the set of changed files (from
//! `ChangedFilesProvider`), [`compute_impact`] flags every verified Knowledge
//! Object whose declared `impacts:` list intersects the changed-file set.
//!
//! Pure domain projection — no I/O. Knowledge Object scope only. See
//! V3-DESIGN.md §V3.3 and ADR-0019.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::domain::graph::GraphKnowledgeObjectNode;
use crate::domain::value_objects::rel_path::RelPath;

use super::object_diff::ObjectDiff;

const CLAIM_KIND: &str = "claim";
const DECISION_KIND: &str = "decision";
const API_KIND: &str = "api";
const VERIFIED_STATUS: &str = "verified";
const ACCEPTED_STATUS: &str = "accepted";
const SOURCE_KIND: &str = "source";
const SOURCE_PATH_FIELD: &str = "path";
/// Inline evidence kinds whose value is a repo-relative path (V6.3).
const PATH_EVIDENCE_KINDS: [&str; 2] = ["source_code", "test"];

/// One entry of the V3.3 impact list. Carries the Knowledge Object's id
/// plus the subset of its `impacts:` paths that are in the changed-file set.
///
/// Sorted ascending by `id` in the containing list; `paths` is itself
/// sorted ascending.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactedObject {
    pub id: String,
    pub paths: Vec<String>,
}

/// Flag verified claims / accepted decisions whose `impacts` list intersects
/// `changed_files`.
///
/// Considers the **head** side of `ObjectDiff::changed` entries and the
/// records in `ObjectDiff::created`. Deleted entries never appear in the
/// impact list — proof obligations for deletions are V3.4 territory.
///
/// The returned vector is sorted by Object ID, with each entry's `paths`
/// sorted ascending. `compute_impact(g, [])` is empty.
pub fn compute_impact(diff: &ObjectDiff, changed_files: &[RelPath]) -> Vec<ImpactedObject> {
    if changed_files.is_empty() {
        return Vec::new();
    }
    let changed: BTreeSet<&str> = changed_files.iter().map(RelPath::as_str).collect();

    let mut impacted: Vec<ImpactedObject> = Vec::new();
    for node in diff
        .created
        .iter()
        .chain(diff.changed.iter().map(|c| &c.head))
    {
        if let Some(entry) = impact_entry_for(node, &changed) {
            impacted.push(entry);
        }
    }
    impacted.sort_by(|a, b| a.id.cmp(&b.id));
    impacted
}

fn impact_entry_for(
    node: &GraphKnowledgeObjectNode,
    changed: &BTreeSet<&str>,
) -> Option<ImpactedObject> {
    if !is_verified_subject(node) || node.impacts.is_empty() {
        return None;
    }
    let mut hits: BTreeSet<&str> = BTreeSet::new();
    for declared in &node.impacts {
        if changed.contains(declared.as_str()) {
            hits.insert(declared.as_str());
        }
    }
    if hits.is_empty() {
        return None;
    }
    Some(ImpactedObject {
        id: node.id.clone(),
        paths: hits.into_iter().map(str::to_string).collect(),
    })
}

/// Why a changed path implicates a Knowledge Object (V6.3 `adoc.impacted.v0`
/// reason kind). Declaration order is the sort order: `impacts_path` before
/// `evidence_path`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImpactReasonKind {
    ImpactsPath,
    EvidencePath,
}

/// One (path, kind, via) reason hit produced by [`impacted_objects`].
///
/// Derived `Ord` is `(matched_path, kind, via_source_object)` by field order,
/// so collecting hits into a `BTreeSet` yields the deterministic,
/// deduplicated reason list the V6.3 envelope requires.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ImpactReasonHit {
    pub(crate) matched_path: String,
    pub(crate) kind: ImpactReasonKind,
    pub(crate) via_source_object: Option<String>,
}

/// One impacted verified subject with all its reasons, sorted per
/// [`ImpactReasonHit`]'s `Ord`. The containing list is sorted by node id.
#[derive(Debug)]
pub(crate) struct ImpactedHit<'a> {
    pub(crate) node: &'a GraphKnowledgeObjectNode,
    pub(crate) reasons: Vec<ImpactReasonHit>,
}

/// V6.3 inverse impact question over current knowledge: which verified
/// claims / accepted decisions are implicated by `changed_files`?
///
/// Sibling of [`compute_impact`] (ADR-0038): same verified-subject scope and
/// exact per-path matching, but projected over the full current object set
/// instead of an [`ObjectDiff`]. Two reason kinds:
///
/// - `impacts_path` — the object's declared `impacts:` contains a changed
///   path (shared with V3.3 via `impact_entry_for`).
/// - `evidence_path` — an inline `source_code`/`test` evidence value equals a
///   changed path, or a referenced `source` object's `path` field does (the
///   hit then carries `via_source_object`). Referenced sources are resolved
///   from the same `objects` slice — pure, no index required.
///
/// `impacted_objects(objects, [])` is empty.
pub(crate) fn impacted_objects<'a>(
    objects: &[&'a GraphKnowledgeObjectNode],
    changed_files: &[RelPath],
) -> Vec<ImpactedHit<'a>> {
    if changed_files.is_empty() {
        return Vec::new();
    }
    let changed: BTreeSet<&str> = changed_files.iter().map(RelPath::as_str).collect();

    // Pass 1: resolve referenced `source` objects to their declared path.
    // url-only sources have no `path` field and never match.
    let source_paths: BTreeMap<&str, &str> = objects
        .iter()
        .filter(|node| node.kind == SOURCE_KIND)
        .filter_map(|node| {
            node.fields
                .get(SOURCE_PATH_FIELD)
                .map(|path| (node.id.as_str(), path.as_str()))
        })
        .collect();

    // Pass 2: collect reason hits per verified subject.
    let mut impacted: Vec<ImpactedHit<'a>> = Vec::new();
    for node in objects {
        if !is_verified_subject(node) {
            continue;
        }
        let mut reasons: BTreeSet<ImpactReasonHit> = BTreeSet::new();

        if let Some(entry) = impact_entry_for(node, &changed) {
            for path in entry.paths {
                reasons.insert(ImpactReasonHit {
                    matched_path: path,
                    kind: ImpactReasonKind::ImpactsPath,
                    via_source_object: None,
                });
            }
        }

        for evidence in &node.evidence {
            match (&evidence.value, &evidence.reference) {
                (Some(value), None)
                    if PATH_EVIDENCE_KINDS.contains(&evidence.kind.as_str())
                        && changed.contains(value.as_str()) =>
                {
                    reasons.insert(ImpactReasonHit {
                        matched_path: value.clone(),
                        kind: ImpactReasonKind::EvidencePath,
                        via_source_object: None,
                    });
                }
                (None, Some(reference)) => {
                    if let Some(path) = source_paths.get(reference.as_str())
                        && changed.contains(path)
                    {
                        reasons.insert(ImpactReasonHit {
                            matched_path: (*path).to_string(),
                            kind: ImpactReasonKind::EvidencePath,
                            via_source_object: Some(reference.clone()),
                        });
                    }
                }
                _ => {}
            }
        }

        if !reasons.is_empty() {
            impacted.push(ImpactedHit {
                node,
                reasons: reasons.into_iter().collect(),
            });
        }
    }
    impacted.sort_by(|a, b| a.node.id.cmp(&b.node.id));
    impacted
}

fn is_verified_subject(node: &GraphKnowledgeObjectNode) -> bool {
    let Some(status) = node.status.as_deref() else {
        return false;
    };
    match node.kind.as_str() {
        CLAIM_KIND => status == VERIFIED_STATUS,
        DECISION_KIND => status == ACCEPTED_STATUS,
        // V6.5.1 (within the ADR-0038 reason set): a verified api naturally
        // declares its schema/source files via `impacts:`.
        API_KIND => status == VERIFIED_STATUS,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::graph::GraphKnowledgeObjectNode;
    use crate::domain::review::object_change::ChangedObject;
    use crate::domain::review::object_diff::test_support::test_node;

    fn rel(s: &str) -> RelPath {
        RelPath::try_new(s).expect("valid test path")
    }

    fn verified_claim_with_impacts(id: &str, paths: &[&str]) -> GraphKnowledgeObjectNode {
        let mut node = test_node(id, "sha256:dummy");
        node.status = Some(VERIFIED_STATUS.to_string());
        node.impacts = paths.iter().map(|p| (*p).to_string()).collect();
        node
    }

    fn diff_with_created(nodes: Vec<GraphKnowledgeObjectNode>) -> ObjectDiff {
        ObjectDiff::compute(&[], &nodes)
    }

    fn diff_with_changed(
        base_head_pairs: Vec<(GraphKnowledgeObjectNode, GraphKnowledgeObjectNode)>,
    ) -> ObjectDiff {
        // Hand-build a diff via Changed entries — distinct content_hash forces them
        // into `changed[]` when fed to `compute`.
        let base: Vec<GraphKnowledgeObjectNode> =
            base_head_pairs.iter().map(|(b, _)| b.clone()).collect();
        let head: Vec<GraphKnowledgeObjectNode> =
            base_head_pairs.iter().map(|(_, h)| h.clone()).collect();
        ObjectDiff::compute(&base, &head)
    }

    // The "first failing test" listed in V3-DESIGN.md §Test Pyramid for V3.3.
    #[test]
    fn compute_impact_flags_verified_claim_when_impacts_path_in_changed_set() {
        let diff = diff_with_created(vec![verified_claim_with_impacts(
            "billing.refunds",
            &["crates/billing/src/refund.rs"],
        )]);

        let impacted = compute_impact(&diff, &[rel("crates/billing/src/refund.rs")]);

        assert_eq!(
            impacted,
            vec![ImpactedObject {
                id: "billing.refunds".to_string(),
                paths: vec!["crates/billing/src/refund.rs".to_string()],
            }]
        );
    }

    #[test]
    fn compute_impact_returns_empty_when_no_files_changed() {
        let diff = diff_with_created(vec![verified_claim_with_impacts(
            "billing.refunds",
            &["a.rs"],
        )]);

        assert!(compute_impact(&diff, &[]).is_empty());
    }

    #[test]
    fn compute_impact_returns_empty_when_no_overlap() {
        let diff = diff_with_created(vec![verified_claim_with_impacts(
            "billing.refunds",
            &["a.rs"],
        )]);

        assert!(compute_impact(&diff, &[rel("b.rs")]).is_empty());
    }

    #[test]
    fn compute_impact_sorts_overlapping_paths_ascending() {
        let diff = diff_with_created(vec![verified_claim_with_impacts(
            "billing.refunds",
            &["z.rs", "a.rs", "m.rs"],
        )]);

        let impacted = compute_impact(
            &diff,
            &[rel("m.rs"), rel("z.rs"), rel("a.rs"), rel("other.rs")],
        );

        assert_eq!(impacted.len(), 1);
        assert_eq!(impacted[0].paths, vec!["a.rs", "m.rs", "z.rs"]);
    }

    #[test]
    fn compute_impact_sorts_results_by_object_id() {
        let diff = diff_with_created(vec![
            verified_claim_with_impacts("zz.late", &["a.rs"]),
            verified_claim_with_impacts("aa.early", &["a.rs"]),
        ]);

        let impacted = compute_impact(&diff, &[rel("a.rs")]);

        let ids: Vec<&str> = impacted.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, vec!["aa.early", "zz.late"]);
    }

    #[test]
    fn compute_impact_ignores_non_verified_claim_even_when_impacts_match() {
        let mut node = test_node("billing.refunds", "sha256:n");
        node.status = Some("draft".to_string()); // not verified
        node.impacts = vec!["a.rs".to_string()];

        let diff = diff_with_created(vec![node]);

        assert!(compute_impact(&diff, &[rel("a.rs")]).is_empty());
    }

    #[test]
    fn compute_impact_includes_accepted_decision_intersection() {
        let mut node = test_node("billing.policy", "sha256:n");
        node.kind = "decision".to_string();
        node.status = Some(ACCEPTED_STATUS.to_string());
        node.impacts = vec!["a.rs".to_string()];

        let diff = diff_with_created(vec![node]);

        let impacted = compute_impact(&diff, &[rel("a.rs")]);
        assert_eq!(impacted.len(), 1);
        assert_eq!(impacted[0].id, "billing.policy");
    }

    #[test]
    fn compute_impact_ignores_proposed_decision() {
        let mut node = test_node("billing.policy", "sha256:n");
        node.kind = "decision".to_string();
        node.status = Some("proposed".to_string());
        node.impacts = vec!["a.rs".to_string()];

        let diff = diff_with_created(vec![node]);

        assert!(compute_impact(&diff, &[rel("a.rs")]).is_empty());
    }

    #[test]
    fn compute_impact_uses_head_side_of_changed_entries() {
        // base side has no impacts; head side declares impacts.
        let mut base = test_node("billing.refunds", "sha256:base");
        base.status = Some(VERIFIED_STATUS.to_string());
        let mut head = test_node("billing.refunds", "sha256:head");
        head.status = Some(VERIFIED_STATUS.to_string());
        head.impacts = vec!["a.rs".to_string()];

        let diff = diff_with_changed(vec![(base, head)]);

        let impacted = compute_impact(&diff, &[rel("a.rs")]);
        assert_eq!(impacted.len(), 1);
        assert_eq!(impacted[0].id, "billing.refunds");
    }

    #[test]
    fn compute_impact_ignores_deleted_objects() {
        // Deleted-only side: base has the verified claim, head omits it.
        let mut base = test_node("billing.refunds", "sha256:base");
        base.status = Some(VERIFIED_STATUS.to_string());
        base.impacts = vec!["a.rs".to_string()];

        let diff = ObjectDiff::compute(&[base], &[]);

        assert!(compute_impact(&diff, &[rel("a.rs")]).is_empty());
    }

    // --- V6.3 `impacted_objects` (inverse question over current knowledge) ---

    use crate::domain::graph::GraphEvidence;

    fn source_node(id: &str, path: Option<&str>) -> GraphKnowledgeObjectNode {
        let mut node = test_node(id, "sha256:src");
        node.kind = "source".to_string();
        node.status = None;
        if let Some(p) = path {
            node.fields.insert("path".to_string(), p.to_string());
        }
        node
    }

    fn hit(path: &str, kind: ImpactReasonKind, via: Option<&str>) -> ImpactReasonHit {
        ImpactReasonHit {
            matched_path: path.to_string(),
            kind,
            via_source_object: via.map(str::to_string),
        }
    }

    #[test]
    fn impacted_objects_flags_verified_claim_on_impacts_path() {
        let claim =
            verified_claim_with_impacts("billing.refunds", &["crates/billing/src/refund.rs"]);

        let impacted = impacted_objects(&[&claim], &[rel("crates/billing/src/refund.rs")]);

        assert_eq!(impacted.len(), 1);
        assert_eq!(impacted[0].node.id, "billing.refunds");
        assert_eq!(
            impacted[0].reasons,
            vec![hit(
                "crates/billing/src/refund.rs",
                ImpactReasonKind::ImpactsPath,
                None
            )]
        );
    }

    #[test]
    fn impacted_objects_empty_changed_set_returns_empty() {
        let claim = verified_claim_with_impacts("billing.refunds", &["a.rs"]);

        assert!(impacted_objects(&[&claim], &[]).is_empty());
    }

    #[test]
    fn impacted_objects_flags_inline_source_code_and_test_evidence_paths() {
        let mut claim = test_node("billing.consume", "sha256:n");
        claim.status = Some(VERIFIED_STATUS.to_string());
        claim.evidence = vec![
            GraphEvidence::inline("source_code", "src/credits.rs"),
            GraphEvidence::inline("test", "tests/credits.rs"),
            // Non-path-bearing kinds and non-path values must not match.
            GraphEvidence::inline("human_review", "src/credits.rs"),
            GraphEvidence::inline("test", "cargo test credits"),
        ];

        let impacted =
            impacted_objects(&[&claim], &[rel("src/credits.rs"), rel("tests/credits.rs")]);

        assert_eq!(impacted.len(), 1);
        assert_eq!(
            impacted[0].reasons,
            vec![
                hit("src/credits.rs", ImpactReasonKind::EvidencePath, None),
                hit("tests/credits.rs", ImpactReasonKind::EvidencePath, None),
            ]
        );
    }

    #[test]
    fn impacted_objects_resolves_object_ref_evidence_via_source_path() {
        let mut claim = test_node("billing.consume", "sha256:n");
        claim.status = Some(VERIFIED_STATUS.to_string());
        claim.evidence = vec![
            GraphEvidence::object_ref("source_code", "billing.consume-use-case"),
            // Dangling reference: target absent from the object set.
            GraphEvidence::object_ref("source_code", "billing.missing-source"),
            // url-only source object: no path field, never matches.
            GraphEvidence::object_ref("source_code", "billing.url-source"),
        ];
        let source = source_node("billing.consume-use-case", Some("src/consume.use-case.ts"));
        let url_source = source_node("billing.url-source", None);

        let impacted = impacted_objects(
            &[&claim, &source, &url_source],
            &[rel("src/consume.use-case.ts")],
        );

        assert_eq!(impacted.len(), 1);
        assert_eq!(impacted[0].node.id, "billing.consume");
        assert_eq!(
            impacted[0].reasons,
            vec![hit(
                "src/consume.use-case.ts",
                ImpactReasonKind::EvidencePath,
                Some("billing.consume-use-case")
            )]
        );
    }

    #[test]
    fn impacted_objects_excludes_non_verified_subjects() {
        let mut draft_claim = verified_claim_with_impacts("billing.draft", &["a.rs"]);
        draft_claim.status = Some("draft".to_string());

        let mut proposed_decision = verified_claim_with_impacts("billing.proposed", &["a.rs"]);
        proposed_decision.kind = "decision".to_string();
        proposed_decision.status = Some("proposed".to_string());

        let mut constraint = verified_claim_with_impacts("security.constraint", &["a.rs"]);
        constraint.kind = "constraint".to_string();

        let mut draft_with_evidence = test_node("billing.draft-evidence", "sha256:n");
        draft_with_evidence.evidence = vec![GraphEvidence::inline("source_code", "a.rs")];

        let impacted = impacted_objects(
            &[
                &draft_claim,
                &proposed_decision,
                &constraint,
                &draft_with_evidence,
            ],
            &[rel("a.rs")],
        );

        assert!(impacted.is_empty());
    }

    #[test]
    fn impacted_objects_same_path_in_impacts_and_evidence_yields_two_reasons_one_record() {
        let mut claim = verified_claim_with_impacts("billing.refunds", &["src/refund.rs"]);
        claim.evidence = vec![
            GraphEvidence::inline("source_code", "src/refund.rs"),
            // Exact duplicate inline entry dedups to one reason.
            GraphEvidence::inline("source_code", "src/refund.rs"),
        ];

        let impacted = impacted_objects(&[&claim], &[rel("src/refund.rs")]);

        assert_eq!(impacted.len(), 1);
        assert_eq!(
            impacted[0].reasons,
            vec![
                hit("src/refund.rs", ImpactReasonKind::ImpactsPath, None),
                hit("src/refund.rs", ImpactReasonKind::EvidencePath, None),
            ]
        );
    }

    #[test]
    fn impacted_objects_sorts_records_by_id_and_reasons_by_path_then_kind() {
        let late = verified_claim_with_impacts("zz.late", &["a.rs"]);
        let mut early = verified_claim_with_impacts("aa.early", &["b.rs", "a.rs"]);
        early.evidence = vec![GraphEvidence::inline("test", "a.rs")];

        let impacted = impacted_objects(&[&late, &early], &[rel("a.rs"), rel("b.rs")]);

        let ids: Vec<&str> = impacted.iter().map(|h| h.node.id.as_str()).collect();
        assert_eq!(ids, vec!["aa.early", "zz.late"]);
        assert_eq!(
            impacted[0].reasons,
            vec![
                hit("a.rs", ImpactReasonKind::ImpactsPath, None),
                hit("a.rs", ImpactReasonKind::EvidencePath, None),
                hit("b.rs", ImpactReasonKind::ImpactsPath, None),
            ]
        );
    }

    #[test]
    fn impacted_objects_includes_accepted_decision_via_evidence() {
        let mut decision = test_node("billing.use-ledger", "sha256:n");
        decision.kind = "decision".to_string();
        decision.status = Some(ACCEPTED_STATUS.to_string());
        decision.evidence = vec![GraphEvidence::object_ref("source_code", "billing.src")];
        let source = source_node("billing.src", Some("src/ledger.rs"));

        let impacted = impacted_objects(&[&decision, &source], &[rel("src/ledger.rs")]);

        assert_eq!(impacted.len(), 1);
        assert_eq!(impacted[0].node.id, "billing.use-ledger");
    }

    #[test]
    fn changed_object_referenced_so_dead_code_lint_is_satisfied() {
        // Suppress unused import warning for ChangedObject in this test module
        // (it is read transitively through ObjectDiff::changed but the lint
        // sometimes flags the direct `use` line).
        let _ = std::mem::size_of::<ChangedObject>();
    }
}
