//! V6.1 lifecycle-signal read queries over a loaded graph artifact.
//!
//! `adoc stale` (and, later, `adoc contradictions`) are read-only queries over
//! `dist/docs.graph.json`. Staleness and overdue-ness are **re-derived at read
//! time** from authored fields — an artifact built last week must not report
//! stale-as-of-build-time — so this module never trusts the persisted
//! `effective_status` projection. See ADR-0038 and docs/V6-DESIGN.md.

use chrono::NaiveDate;
use serde::Serialize;

use crate::domain::diagnostic::Diagnostic;
use crate::domain::graph::GraphKnowledgeObjectNode;
use crate::domain::value_objects::review_interval::ReviewInterval;
use crate::infrastructure::artifact::graph_json::derive_effective_status_from_fields;

use super::graph::GraphSession;
use super::local_today;

pub const STALE_SCHEMA_VERSION: &str = "adoc.stale.v0";

/// Why a Knowledge Object appears in the `adoc stale` listing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StaleCategory {
    /// `expires_at` is strictly before the evaluation date (any status).
    Stale,
    /// An `active` policy whose `effective_at + review_interval` is strictly
    /// before the evaluation date.
    ReviewOverdue,
    /// A verified object whose `expires_at` falls within the `--within`
    /// horizon (evaluation date inclusive).
    ExpiringSoon,
}

impl StaleCategory {
    /// Fixed ordinal used only as the final sort tiebreak so that one object
    /// yielding records in several categories stays deterministic.
    fn ordinal(self) -> u8 {
        match self {
            Self::Stale => 0,
            Self::ReviewOverdue => 1,
            Self::ExpiringSoon => 2,
        }
    }
}

/// One `adoc.stale.v0` record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StaleRecord {
    pub id: String,
    pub kind: String,
    pub category: StaleCategory,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authored_status: Option<String>,
    /// Re-derived at read time; echoes the authored status when no derivation
    /// applies (e.g. a draft object listed only because its expiry passed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_status: Option<String>,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub days_overdue: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub days_remaining: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    pub source_path: String,
}

impl StaleRecord {
    /// Signed urgency for the "most-overdue first" sort: overdue records are
    /// positive (≥ 1 by the strict `<` derivations), expiring-soon records are
    /// zero or negative.
    fn urgency(&self) -> i64 {
        match self.category {
            StaleCategory::Stale | StaleCategory::ReviewOverdue => {
                i64::from(self.days_overdue.unwrap_or(0))
            }
            StaleCategory::ExpiringSoon => -i64::from(self.days_remaining.unwrap_or(0)),
        }
    }
}

/// The `adoc.stale.v0` wire envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StaleEnvelope {
    pub schema_version: &'static str,
    /// `%Y-%m-%d` evaluation date — staleness is re-derived against this date,
    /// not against the artifact's build date.
    pub evaluated_at: String,
    pub records: Vec<StaleRecord>,
    pub diagnostics: Vec<Diagnostic>,
}

impl StaleEnvelope {
    pub fn new(
        evaluated_at: NaiveDate,
        records: Vec<StaleRecord>,
        diagnostics: Vec<Diagnostic>,
    ) -> Self {
        Self {
            schema_version: STALE_SCHEMA_VERSION,
            evaluated_at: evaluated_at.format("%Y-%m-%d").to_string(),
            records,
            diagnostics,
        }
    }
}

/// Evaluate the stale query against today's date.
pub(crate) fn evaluate_stale_today(
    session: &GraphSession,
    within_days: Option<u32>,
    diagnostics: Vec<Diagnostic>,
) -> StaleEnvelope {
    let today = local_today();
    StaleEnvelope::new(
        today,
        evaluate_stale_for_date(session, within_days, today),
        diagnostics,
    )
}

/// Empty envelope for the artifact-load-failure path: `evaluated_at` is still
/// populated so consumers never special-case the field.
pub(crate) fn empty_stale_envelope_today(diagnostics: Vec<Diagnostic>) -> StaleEnvelope {
    StaleEnvelope::new(local_today(), Vec::new(), diagnostics)
}

/// Pure evaluation against an explicit date (unit-test entry point).
pub(crate) fn evaluate_stale_for_date(
    session: &GraphSession,
    within_days: Option<u32>,
    today: NaiveDate,
) -> Vec<StaleRecord> {
    let mut records = Vec::new();

    for node in session.objects() {
        records.extend(expiry_records(node, within_days, today));
        records.extend(review_overdue_record(node, today));
    }

    records.sort_by(|a, b| {
        b.urgency()
            .cmp(&a.urgency())
            .then_with(|| a.id.cmp(&b.id))
            .then_with(|| a.category.ordinal().cmp(&b.category.ordinal()))
    });
    records
}

/// `stale` / `expiring_soon` records driven by the `expires_at` field.
fn expiry_records(
    node: &GraphKnowledgeObjectNode,
    within_days: Option<u32>,
    today: NaiveDate,
) -> Option<StaleRecord> {
    let expires_at_value = node.fields.get("expires_at")?;
    let expires_at = NaiveDate::parse_from_str(expires_at_value, "%Y-%m-%d").ok()?;

    if expires_at < today {
        // Listed for ANY authored status (the lifecycle.expired rule's
        // breadth); the re-derived effective_status stays verified-only.
        let days_overdue = days_between(expires_at, today);
        return Some(StaleRecord {
            id: node.id.clone(),
            kind: node.kind.clone(),
            category: StaleCategory::Stale,
            authored_status: node.status.clone(),
            effective_status: rederived_effective_status(node, today),
            reason: format!("expired:{expires_at}"),
            expires_at: Some(expires_at_value.clone()),
            days_overdue: Some(days_overdue),
            days_remaining: None,
            owner: node.fields.get("owner").cloned(),
            source_path: node.source_span.path.clone(),
        });
    }

    let horizon_days = within_days?;
    if node.status.as_deref() != Some("verified") {
        return None;
    }
    // A `None` horizon (date overflow) means "unbounded": everything counts.
    let within_horizon = today
        .checked_add_days(chrono::Days::new(u64::from(horizon_days)))
        .is_none_or(|horizon| expires_at <= horizon);
    if !within_horizon {
        return None;
    }

    let days_remaining = days_between(today, expires_at);
    Some(StaleRecord {
        id: node.id.clone(),
        kind: node.kind.clone(),
        category: StaleCategory::ExpiringSoon,
        authored_status: node.status.clone(),
        effective_status: rederived_effective_status(node, today),
        reason: format!("expires:{expires_at}"),
        expires_at: Some(expires_at_value.clone()),
        days_overdue: None,
        days_remaining: Some(days_remaining),
        owner: node.fields.get("owner").cloned(),
        source_path: node.source_span.path.clone(),
    })
}

/// `review_overdue` record for active policies, mirroring the compile-time
/// `PolicyReviewDrift` rule's arithmetic (strict `<`).
fn review_overdue_record(node: &GraphKnowledgeObjectNode, today: NaiveDate) -> Option<StaleRecord> {
    if node.kind != "policy" || node.status.as_deref() != Some("active") {
        return None;
    }

    let effective_at =
        NaiveDate::parse_from_str(node.fields.get("effective_at")?, "%Y-%m-%d").ok()?;
    let interval = ReviewInterval::try_new(node.fields.get("review_interval")?).ok()?;
    let next_review = effective_at + chrono::Duration::days(i64::from(interval.days()));

    if next_review >= today {
        return None;
    }

    Some(StaleRecord {
        id: node.id.clone(),
        kind: node.kind.clone(),
        category: StaleCategory::ReviewOverdue,
        authored_status: node.status.clone(),
        effective_status: rederived_effective_status(node, today),
        reason: format!("review_due:{next_review}"),
        expires_at: None,
        days_overdue: Some(days_between(next_review, today)),
        days_remaining: None,
        owner: node.fields.get("owner").cloned(),
        source_path: node.source_span.path.clone(),
    })
}

/// Read-time effective status: the V5.10 expiry derivation when it applies,
/// otherwise an echo of the authored status. Never trusts the persisted
/// projection on the node.
fn rederived_effective_status(node: &GraphKnowledgeObjectNode, today: NaiveDate) -> Option<String> {
    derive_effective_status_from_fields(
        node.status.as_deref(),
        node.fields.get("expires_at").map(String::as_str),
        today,
    )
    .map(|(status, _reason)| status)
    .or_else(|| node.status.clone())
}

/// Whole days from `earlier` to `later` (`later` strictly after `earlier` at
/// every call site, so the result is ≥ 1 for overdue and ≥ 0 for remaining).
fn days_between(earlier: NaiveDate, later: NaiveDate) -> u32 {
    u32::try_from((later - earlier).num_days()).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::domain::graph::{
        GraphArtifactDocument, GraphIndex, GraphNode, GraphPageNode, GraphRelations,
        GraphSourceSpan,
    };

    fn fixed_today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 6, 1).expect("valid date")
    }

    fn ko_node(id: &str, kind: &str, status: Option<&str>, fields: &[(&str, &str)]) -> GraphNode {
        GraphNode::KnowledgeObject(GraphKnowledgeObjectNode {
            id: id.to_string(),
            kind: kind.to_string(),
            content_hash: format!("sha256:{id}"),
            status: status.map(str::to_string),
            severity: None,
            trust: None,
            body: "Body.".to_string(),
            page_id: "team.page".to_string(),
            source_span: GraphSourceSpan {
                path: "docs/team.adoc".to_string(),
                line: 1,
                column: 1,
            },
            fields: fields
                .iter()
                .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
                .collect::<BTreeMap<_, _>>(),
            relations: GraphRelations::default(),
            impacts: Vec::new(),
            approved_by: Vec::new(),
            allowed_actions: Vec::new(),
            forbidden_actions: Vec::new(),
            contradiction_claims: Vec::new(),
            evidence: Vec::new(),
            // Deliberately never persisted: read-time re-derivation must not
            // depend on the build-time projection.
            effective_status: None,
            effective_reason: None,
            evidence_quality: None,
        })
    }

    fn session_with(nodes: Vec<GraphNode>) -> GraphSession {
        let mut all_nodes = vec![GraphNode::Page(GraphPageNode {
            id: "team.page".to_string(),
            order: 0,
            title: None,
            source_path: "docs/team.adoc".to_string(),
        })];
        all_nodes.extend(nodes);
        let document = GraphArtifactDocument {
            schema_version: "adoc.graph.v3".to_string(),
            nodes: all_nodes,
            edges: Vec::new(),
            diagnostics: Vec::new(),
        };
        GraphSession::new(GraphIndex::from_document(document).expect("valid graph document"))
    }

    /// All three categories from one artifact pass, sorted most-overdue first
    /// then id, with read-time re-derivation (no persisted effective_status).
    #[test]
    fn evaluate_stale_emits_all_three_categories_sorted_most_overdue_first() {
        let session = session_with(vec![
            ko_node(
                "billing.legacy",
                "claim",
                Some("draft"),
                &[("expires_at", "2026-01-15"), ("owner", "team-billing")],
            ),
            ko_node(
                "security.retention",
                "claim",
                Some("verified"),
                &[("expires_at", "2024-01-01"), ("owner", "security-lead")],
            ),
            ko_node(
                "security.db-access",
                "policy",
                Some("active"),
                &[
                    ("effective_at", "2020-01-01"),
                    ("review_interval", "90d"),
                    ("owner", "security-lead"),
                ],
            ),
            ko_node(
                "billing.consume",
                "claim",
                Some("verified"),
                &[("expires_at", "2120-01-01")],
            ),
        ]);

        let records = evaluate_stale_for_date(&session, Some(36500), fixed_today());

        let ids: Vec<&str> = records.iter().map(|record| record.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "security.db-access", // review due 2020-03-31 — most overdue
                "security.retention", // expired 2024-01-01
                "billing.legacy",     // expired 2026-01-15
                "billing.consume",    // expiring 2120-01-01
            ],
            "records must sort most-overdue first, then id"
        );

        let policy = &records[0];
        assert_eq!(policy.category, StaleCategory::ReviewOverdue);
        assert_eq!(policy.reason, "review_due:2020-03-31");
        assert_eq!(policy.authored_status.as_deref(), Some("active"));
        assert_eq!(
            policy.effective_status.as_deref(),
            Some("active"),
            "no derivation applies to an active policy — echo authored status"
        );
        assert!(policy.days_overdue.expect("days_overdue") > 0);
        assert_eq!(policy.days_remaining, None);
        assert_eq!(policy.owner.as_deref(), Some("security-lead"));
        assert_eq!(policy.source_path, "docs/team.adoc");

        let verified_stale = &records[1];
        assert_eq!(verified_stale.category, StaleCategory::Stale);
        assert_eq!(verified_stale.reason, "expired:2024-01-01");
        assert_eq!(verified_stale.authored_status.as_deref(), Some("verified"));
        assert_eq!(
            verified_stale.effective_status.as_deref(),
            Some("stale"),
            "verified + expired must re-derive stale at read time even though \
             the node carries no persisted effective_status"
        );
        assert_eq!(verified_stale.expires_at.as_deref(), Some("2024-01-01"));

        let draft_stale = &records[2];
        assert_eq!(draft_stale.category, StaleCategory::Stale);
        assert_eq!(draft_stale.authored_status.as_deref(), Some("draft"));
        assert_eq!(
            draft_stale.effective_status.as_deref(),
            Some("draft"),
            "draft + expired is listed but derives no effective status — echo"
        );

        let expiring = &records[3];
        assert_eq!(expiring.category, StaleCategory::ExpiringSoon);
        assert_eq!(expiring.reason, "expires:2120-01-01");
        assert!(expiring.days_remaining.expect("days_remaining") > 0);
        assert_eq!(expiring.days_overdue, None);
        assert_eq!(expiring.owner, None);
    }

    /// Policies are exempt from the review check when fields are missing or
    /// unparseable, when the status is not `active`, or when the kind is not
    /// `policy` at all.
    #[test]
    fn review_overdue_is_gated_to_active_policies_with_valid_fields() {
        let session = session_with(vec![
            ko_node(
                "policy.no-interval",
                "policy",
                Some("active"),
                &[("effective_at", "2020-01-01")],
            ),
            ko_node(
                "policy.bad-interval",
                "policy",
                Some("active"),
                &[("effective_at", "2020-01-01"), ("review_interval", "soon")],
            ),
            ko_node(
                "policy.bad-date",
                "policy",
                Some("active"),
                &[("effective_at", "not-a-date"), ("review_interval", "90d")],
            ),
            ko_node(
                "policy.retired",
                "policy",
                Some("retired"),
                &[("effective_at", "2020-01-01"), ("review_interval", "90d")],
            ),
            ko_node(
                "claim.with-policy-fields",
                "claim",
                Some("verified"),
                &[("effective_at", "2020-01-01"), ("review_interval", "90d")],
            ),
        ]);

        let records = evaluate_stale_for_date(&session, None, fixed_today());

        assert!(
            records.is_empty(),
            "none of these may produce a review_overdue record: {records:#?}"
        );
    }

    /// `expires_at == today` is not stale (strict `<`) but IS expiring_soon
    /// with `--within 0d` and `days_remaining: 0`.
    #[test]
    fn expiry_on_evaluation_day_is_expiring_soon_not_stale() {
        let session = session_with(vec![ko_node(
            "claim.today",
            "claim",
            Some("verified"),
            &[("expires_at", "2026-06-01")],
        )]);

        let without_window = evaluate_stale_for_date(&session, None, fixed_today());
        assert!(
            without_window.is_empty(),
            "boundary-day expiry must not be stale and must not appear without --within"
        );

        let with_zero_window = evaluate_stale_for_date(&session, Some(0), fixed_today());
        assert_eq!(with_zero_window.len(), 1);
        assert_eq!(with_zero_window[0].category, StaleCategory::ExpiringSoon);
        assert_eq!(with_zero_window[0].days_remaining, Some(0));
    }

    /// Expiring-soon is gated to authored `verified`; future expiry outside the
    /// window is excluded; unparseable expiry dates are skipped entirely.
    #[test]
    fn expiring_soon_is_gated_to_verified_within_window() {
        let session = session_with(vec![
            ko_node(
                "claim.draft-future",
                "claim",
                Some("draft"),
                &[("expires_at", "2026-06-10")],
            ),
            ko_node(
                "claim.beyond-window",
                "claim",
                Some("verified"),
                &[("expires_at", "2026-07-15")],
            ),
            ko_node(
                "claim.garbage-date",
                "claim",
                Some("verified"),
                &[("expires_at", "soonish")],
            ),
            ko_node(
                "claim.inside-window",
                "claim",
                Some("verified"),
                &[("expires_at", "2026-06-10")],
            ),
        ]);

        let records = evaluate_stale_for_date(&session, Some(30), fixed_today());

        assert_eq!(
            records.len(),
            1,
            "only the verified in-window claim: {records:#?}"
        );
        assert_eq!(records[0].id, "claim.inside-window");
        assert_eq!(records[0].days_remaining, Some(9));
    }

    /// A huge `--within` that overflows the date horizon is treated as
    /// unbounded rather than panicking or silently excluding.
    #[test]
    fn within_horizon_overflow_is_unbounded() {
        let session = session_with(vec![ko_node(
            "claim.far-future",
            "claim",
            Some("verified"),
            &[("expires_at", "2120-01-01")],
        )]);

        let records = evaluate_stale_for_date(&session, Some(u32::MAX), fixed_today());

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].category, StaleCategory::ExpiringSoon);
    }

    /// An expired active policy with an overdue review yields TWO records,
    /// deterministically ordered by urgency, then id, then category ordinal.
    #[test]
    fn one_object_can_yield_stale_and_review_overdue_records() {
        let session = session_with(vec![ko_node(
            "policy.doubly-flagged",
            "policy",
            Some("active"),
            &[
                ("expires_at", "2026-05-31"),
                ("effective_at", "2026-04-01"),
                ("review_interval", "30d"),
            ],
        )]);

        let records = evaluate_stale_for_date(&session, None, fixed_today());

        assert_eq!(records.len(), 2, "expected two records: {records:#?}");
        // expired 2026-05-31 → 1 day overdue; review due 2026-05-01 → 31 days.
        assert_eq!(records[0].category, StaleCategory::ReviewOverdue);
        assert_eq!(records[0].days_overdue, Some(31));
        assert_eq!(records[1].category, StaleCategory::Stale);
        assert_eq!(records[1].days_overdue, Some(1));
    }

    /// Two objects expired on the same day tiebreak by id ascending.
    #[test]
    fn equal_urgency_tiebreaks_by_id() {
        let session = session_with(vec![
            ko_node(
                "claim.zeta",
                "claim",
                Some("draft"),
                &[("expires_at", "2026-01-15")],
            ),
            ko_node(
                "claim.alpha",
                "claim",
                Some("draft"),
                &[("expires_at", "2026-01-15")],
            ),
        ]);

        let records = evaluate_stale_for_date(&session, None, fixed_today());

        let ids: Vec<&str> = records.iter().map(|record| record.id.as_str()).collect();
        assert_eq!(ids, vec!["claim.alpha", "claim.zeta"]);
    }

    /// The envelope carries the schema version, the formatted evaluation date,
    /// and forwarded diagnostics.
    #[test]
    fn stale_envelope_serializes_schema_version_and_evaluated_at() {
        let envelope = StaleEnvelope::new(fixed_today(), Vec::new(), Vec::new());

        let value = serde_json::to_value(&envelope).expect("envelope serializes");

        assert_eq!(value["schema_version"], "adoc.stale.v0");
        assert_eq!(value["evaluated_at"], "2026-06-01");
        assert_eq!(value["records"], serde_json::json!([]));
        assert_eq!(value["diagnostics"], serde_json::json!([]));
    }

    /// Optional record fields are omitted from JSON, not serialized as null.
    #[test]
    fn stale_record_omits_absent_optional_fields() {
        let session = session_with(vec![ko_node(
            "claim.bare",
            "claim",
            None,
            &[("expires_at", "2026-01-15")],
        )]);

        let records = evaluate_stale_for_date(&session, None, fixed_today());
        let value = serde_json::to_value(&records[0]).expect("record serializes");
        let object = value.as_object().expect("record is an object");

        assert!(!object.contains_key("authored_status"));
        assert!(!object.contains_key("effective_status"));
        assert!(!object.contains_key("days_remaining"));
        assert!(!object.contains_key("owner"));
        assert_eq!(value["category"], "stale");
        assert_eq!(value["days_overdue"], 137);
    }
}
