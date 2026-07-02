//! Lifecycle-signal read queries over a loaded graph artifact (V6.1–V6.3).
//!
//! `adoc stale`, `adoc contradictions`, and `adoc impacted-by` are read-only
//! queries over `dist/docs.graph.json`. Signals are **re-derived at read
//! time** from authored fields — an artifact built last week must not report
//! stale-as-of-build-time — so this module never trusts the persisted
//! `effective_status` projection. The stale query is clock-dependent and
//! carries `evaluated_at`; the contradictions and impacted queries are pure
//! functions of the artifact bytes (plus, for impacted, the changed-path set)
//! and deliberately carry no evaluation date. See ADR-0038 and
//! docs/V6-DESIGN.md.

use std::collections::BTreeSet;

use chrono::NaiveDate;
use serde::Serialize;

use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::graph::GraphKnowledgeObjectNode;
use crate::domain::obligation::ProofObligation;
use crate::domain::ports::changed_files::ChangedFilesError;
use crate::domain::review::impact::{ImpactReasonKind, impacted_objects};
use crate::domain::review::obligation_rules::obligation_for_impacted_id;
use crate::domain::value_objects::rel_path::RelPath;
use crate::domain::value_objects::review_interval::ReviewInterval;
use crate::domain::value_objects::severity::Severity;
use crate::infrastructure::artifact::graph_json::{
    derive_effective_status_from_fields, unresolved_contradiction_claim_index,
};

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

pub const CONTRADICTIONS_SCHEMA_VERSION: &str = "adoc.contradictions.v0";

/// Maximum `summary` length in characters (not bytes — multibyte-safe).
const SUMMARY_MAX_CHARS: usize = 120;

/// One contradiction object in the `adoc.contradictions.v0` listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContradictionRecord {
    pub id: String,
    pub severity: String,
    /// Echo of the authored status: `unresolved` by default; `resolved` /
    /// `dismissed` appear only under `--all`.
    pub status: String,
    pub claims: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    pub source_path: String,
    /// First non-empty body line, char-truncated to 120 with `…`.
    pub summary: String,
}

/// One contradicted claim in the `adoc.contradictions.v0` listing: a claim
/// implicated by at least one unresolved contradiction, or one whose authored
/// status is `contradicted`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContradictedClaimRecord {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authored_status: Option<String>,
    /// Contradiction-axis derivation only: `"contradicted"` when implicated by
    /// an unresolved contradiction, otherwise an echo of the authored status.
    /// The expiry axis is `adoc stale`'s job — the two commands answer
    /// different questions and this one is deliberately clock-free.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_status: Option<String>,
    /// `"contradiction:<id>"` where `<id>` is the lexicographically smallest
    /// implicating contradiction — identical to the build-time projection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_reason: Option<String>,
    /// All implicating unresolved contradiction ids, sorted ascending. Empty
    /// only for a claim whose authored status is `contradicted` while no
    /// unresolved contradiction references it.
    pub contradiction_ids: Vec<String>,
}

/// The `adoc.contradictions.v0` wire envelope. A pure function of the artifact
/// bytes: no `evaluated_at`, byte-identical on any day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContradictionsEnvelope {
    pub schema_version: &'static str,
    pub contradictions: Vec<ContradictionRecord>,
    pub contradicted_claims: Vec<ContradictedClaimRecord>,
    pub diagnostics: Vec<Diagnostic>,
}

impl ContradictionsEnvelope {
    pub fn new(
        contradictions: Vec<ContradictionRecord>,
        contradicted_claims: Vec<ContradictedClaimRecord>,
        diagnostics: Vec<Diagnostic>,
    ) -> Self {
        Self {
            schema_version: CONTRADICTIONS_SCHEMA_VERSION,
            contradictions,
            contradicted_claims,
            diagnostics,
        }
    }
}

/// Evaluate the contradictions query: unresolved contradictions (all statuses
/// with `include_all`) plus every contradicted claim, joined for the consumer.
pub(crate) fn evaluate_contradictions(
    session: &GraphSession,
    include_all: bool,
    diagnostics: Vec<Diagnostic>,
) -> ContradictionsEnvelope {
    let implicated = unresolved_contradiction_claim_index(session.objects());

    let mut contradictions: Vec<ContradictionRecord> = session
        .objects()
        .filter(|node| node.kind == "contradiction")
        .filter(|node| include_all || node.status.as_deref() == Some("unresolved"))
        .map(|node| ContradictionRecord {
            id: node.id.clone(),
            // ADR-0039: top-level `severity` is the sole carrier in v4; the
            // v3 fields["severity"] fallback is dead (v3 artifacts are
            // rejected at the version gate).
            severity: node.severity.clone().unwrap_or_default(),
            status: node.status.clone().unwrap_or_default(),
            claims: node.contradiction_claims.clone(),
            owner: node.fields.get("owner").cloned(),
            source_path: node.source_span.path.clone(),
            summary: body_summary(&node.body),
        })
        .collect();

    // Severity descending (critical first; unparseable last), then id.
    contradictions.sort_by(|a, b| {
        let a_severity = Severity::try_new(&a.severity).ok();
        let b_severity = Severity::try_new(&b.severity).ok();
        b_severity.cmp(&a_severity).then_with(|| a.id.cmp(&b.id))
    });

    let mut contradicted_claims: Vec<ContradictedClaimRecord> = session
        .objects()
        .filter(|node| node.kind == "claim")
        .filter_map(|node| {
            let contradiction_ids = implicated.get(&node.id).cloned().unwrap_or_default();
            let authored_contradicted = node.status.as_deref() == Some("contradicted");
            if contradiction_ids.is_empty() && !authored_contradicted {
                return None;
            }
            let (effective_status, effective_reason) = if contradiction_ids.is_empty() {
                // Orphaned authored status: echo, no derivation.
                (node.status.clone(), None)
            } else {
                (
                    Some("contradicted".to_string()),
                    Some(format!("contradiction:{}", contradiction_ids[0])),
                )
            };
            Some(ContradictedClaimRecord {
                id: node.id.clone(),
                authored_status: node.status.clone(),
                effective_status,
                effective_reason,
                contradiction_ids,
            })
        })
        .collect();

    contradicted_claims.sort_by(|a, b| a.id.cmp(&b.id));

    ContradictionsEnvelope::new(contradictions, contradicted_claims, diagnostics)
}

/// Empty envelope for the artifact-load-failure path.
pub(crate) fn empty_contradictions_envelope(
    diagnostics: Vec<Diagnostic>,
) -> ContradictionsEnvelope {
    ContradictionsEnvelope::new(Vec::new(), Vec::new(), diagnostics)
}

pub const IMPACTED_SCHEMA_VERSION: &str = "adoc.impacted.v0";

/// One reason a changed path implicates an object, in the
/// `adoc.impacted.v0` `reasons[]` list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImpactReason {
    /// `"impacts_path"` or `"evidence_path"`.
    pub kind: ImpactReasonKind,
    pub matched_path: String,
    /// The referenced `source` object's id when the evidence path was
    /// resolved through an `evidence_ref`; absent for declared `impacts:`
    /// paths and inline evidence values.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub via_source_object: Option<String>,
}

/// One impacted verified subject in the `adoc.impacted.v0` listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImpactedRecord {
    pub id: String,
    pub kind: String,
    /// Always present: only verified claims / accepted decisions appear.
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// Sorted `(matched_path, kind, via_source_object)`, deduplicated.
    pub reasons: Vec<ImpactReason>,
}

/// The `adoc.impacted.v0` wire envelope. Clock-free: a pure function of the
/// artifact bytes and the changed-path set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImpactedEnvelope {
    pub schema_version: &'static str,
    /// Sorted ascending, deduplicated — both input shapes normalize here.
    pub changed_paths: Vec<String>,
    /// Sorted by Object ID; one record per object regardless of reason count.
    pub impacted: Vec<ImpactedRecord>,
    /// One impact-review obligation per impacted record
    /// (`obligation_for_impacted_id`), sorted by object id.
    pub proof_obligations: Vec<ProofObligation>,
    pub diagnostics: Vec<Diagnostic>,
}

impl ImpactedEnvelope {
    pub fn new(
        changed_paths: Vec<String>,
        impacted: Vec<ImpactedRecord>,
        proof_obligations: Vec<ProofObligation>,
        diagnostics: Vec<Diagnostic>,
    ) -> Self {
        Self {
            schema_version: IMPACTED_SCHEMA_VERSION,
            changed_paths,
            impacted,
            proof_obligations,
            diagnostics,
        }
    }
}

/// Evaluate the V6.3 impacted query: which verified claims / accepted
/// decisions are implicated by `changed_files`, with one impact-review proof
/// obligation per impacted record.
pub(crate) fn evaluate_impacted(
    session: &GraphSession,
    changed_files: &[RelPath],
    diagnostics: Vec<Diagnostic>,
) -> ImpactedEnvelope {
    let objects: Vec<&GraphKnowledgeObjectNode> = session.objects().collect();
    let hits = impacted_objects(&objects, changed_files);

    let mut impacted = Vec::with_capacity(hits.len());
    let mut obligations = Vec::with_capacity(hits.len());
    for hit in hits {
        obligations.push(obligation_for_impacted_id(&hit.node.id));
        impacted.push(ImpactedRecord {
            id: hit.node.id.clone(),
            kind: hit.node.kind.clone(),
            status: hit.node.status.clone().unwrap_or_default(),
            owner: hit.node.fields.get("owner").cloned(),
            reasons: hit
                .reasons
                .into_iter()
                .map(|reason| ImpactReason {
                    kind: reason.kind,
                    matched_path: reason.matched_path,
                    via_source_object: reason.via_source_object,
                })
                .collect(),
        });
    }

    // One obligation per hit, ids unique, hits sorted by id (see
    // `impacted_objects`) — the sort is a ~free explicit invariant guard,
    // not a dedup/merge.
    obligations.sort_by(|a, b| a.object_id.cmp(&b.object_id));
    ImpactedEnvelope::new(
        changed_paths_strings(changed_files),
        impacted,
        obligations,
        diagnostics,
    )
}

/// Empty envelope for failure paths (invalid input, artifact load failure).
/// `changed_paths` echoes whatever was resolved before the failure.
pub(crate) fn empty_impacted_envelope(
    changed_paths: Vec<String>,
    diagnostics: Vec<Diagnostic>,
) -> ImpactedEnvelope {
    ImpactedEnvelope::new(changed_paths, Vec::new(), Vec::new(), diagnostics)
}

/// Sorted, deduplicated wire strings for the envelope's `changed_paths`.
pub(crate) fn changed_paths_strings(changed_files: &[RelPath]) -> Vec<String> {
    changed_files
        .iter()
        .map(RelPath::as_str)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(str::to_string)
        .collect()
}

/// Map a [`ChangedFilesError`] from the `--ref` git derivation to a
/// fix-oriented envelope diagnostic (ADR-0038 posture: a query emits its
/// envelope even when the question could not be derived).
pub(crate) fn changed_files_failure_diagnostic(
    error: &ChangedFilesError,
    base_ref: &str,
) -> Diagnostic {
    match error {
        ChangedFilesError::UnresolvableBase { spec, reason } => Diagnostic::error(
            DiagnosticCode::ImpactedRefUnresolvable,
            format!("could not resolve --ref `{spec}`: {reason}"),
        ),
        ChangedFilesError::ProviderUnavailable { reason } => Diagnostic::error(
            DiagnosticCode::ImpactedGitUnavailable,
            format!("git is unavailable for --ref `{base_ref}`: {reason}"),
        ),
        ChangedFilesError::Io(err) => Diagnostic::error(
            DiagnosticCode::ImpactedGitUnavailable,
            format!("git failed while deriving changed files for --ref `{base_ref}`: {err}"),
        ),
    }
}

/// Validate explicit positional changed paths. Every invalid value yields one
/// `impacted.invalid_path` diagnostic — all collected, not first-error.
pub(crate) fn validate_changed_paths(paths: &[String]) -> Result<Vec<RelPath>, Vec<Diagnostic>> {
    let mut valid = Vec::with_capacity(paths.len());
    let mut diagnostics = Vec::new();
    for path in paths {
        match RelPath::try_new(path) {
            Ok(rel_path) => valid.push(rel_path),
            Err(error) => diagnostics.push(Diagnostic::error(
                DiagnosticCode::ImpactedInvalidPath,
                format!("invalid changed path `{path}`: {error}"),
            )),
        }
    }
    if diagnostics.is_empty() {
        Ok(valid)
    } else {
        Err(diagnostics)
    }
}

/// First non-empty (ASCII-trimmed) body line, truncated to
/// [`SUMMARY_MAX_CHARS`] characters with a trailing `…` when cut.
fn body_summary(body: &str) -> String {
    let first_line = body
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("");
    if first_line.chars().count() <= SUMMARY_MAX_CHARS {
        first_line.to_string()
    } else {
        let mut truncated: String = first_line.chars().take(SUMMARY_MAX_CHARS - 1).collect();
        truncated.push('…');
        truncated
    }
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
            schema_version: "adoc.graph.v4".to_string(),
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

    fn contradiction_node(id: &str, severity: &str, status: &str, claims: &[&str]) -> GraphNode {
        contradiction_node_with_body(
            id,
            severity,
            status,
            claims,
            "Claims disagree about session storage.",
        )
    }

    fn contradiction_node_with_body(
        id: &str,
        severity: &str,
        status: &str,
        claims: &[&str],
        body: &str,
    ) -> GraphNode {
        let mut node = ko_node(
            id,
            "contradiction",
            Some(status),
            &[("owner", "platform-security")],
        );
        let GraphNode::KnowledgeObject(ko) = &mut node else {
            unreachable!("ko_node builds a knowledge object");
        };
        // ADR-0039: severity is a dedicated node field, not a fields entry.
        ko.severity = Some(severity.to_string());
        ko.contradiction_claims = claims.iter().map(|claim| (*claim).to_string()).collect();
        ko.body = body.to_string();
        node
    }

    /// Default listing is unresolved-only; `--all` adds resolved/dismissed with
    /// echoed statuses. Claims referenced only by a non-unresolved
    /// contradiction never enter `contradicted_claims`, under either mode.
    #[test]
    fn contradictions_default_lists_unresolved_only_and_all_includes_terminal_statuses() {
        let session = session_with(vec![
            contradiction_node(
                "conflict.open",
                "high",
                "unresolved",
                &["claim.a", "claim.b"],
            ),
            contradiction_node(
                "conflict.closed",
                "critical",
                "resolved",
                &["claim.c", "claim.d"],
            ),
            contradiction_node(
                "conflict.noise",
                "low",
                "dismissed",
                &["claim.c", "claim.d"],
            ),
            ko_node("claim.a", "claim", Some("verified"), &[]),
            ko_node("claim.b", "claim", Some("contradicted"), &[]),
            ko_node("claim.c", "claim", Some("verified"), &[]),
            ko_node("claim.d", "claim", Some("verified"), &[]),
        ]);

        let default_envelope = evaluate_contradictions(&session, false, Vec::new());
        let ids: Vec<&str> = default_envelope
            .contradictions
            .iter()
            .map(|record| record.id.as_str())
            .collect();
        assert_eq!(ids, vec!["conflict.open"]);
        assert_eq!(default_envelope.contradictions[0].status, "unresolved");
        assert_eq!(default_envelope.contradictions[0].severity, "high");
        assert_eq!(
            default_envelope.contradictions[0].claims,
            vec!["claim.a", "claim.b"]
        );
        assert_eq!(
            default_envelope.contradictions[0].owner.as_deref(),
            Some("platform-security")
        );
        assert_eq!(
            default_envelope.contradictions[0].source_path,
            "docs/team.adoc"
        );

        let all_envelope = evaluate_contradictions(&session, true, Vec::new());
        let all_ids: Vec<(&str, &str)> = all_envelope
            .contradictions
            .iter()
            .map(|record| (record.id.as_str(), record.status.as_str()))
            .collect();
        assert_eq!(
            all_ids,
            vec![
                ("conflict.closed", "resolved"), // critical sorts first
                ("conflict.open", "unresolved"),
                ("conflict.noise", "dismissed"),
            ]
        );

        // claim.c / claim.d are referenced only by resolved/dismissed
        // contradictions — never contradicted, under either mode.
        for envelope in [&default_envelope, &all_envelope] {
            let claim_ids: Vec<&str> = envelope
                .contradicted_claims
                .iter()
                .map(|record| record.id.as_str())
                .collect();
            assert_eq!(claim_ids, vec!["claim.a", "claim.b"]);
        }
    }

    /// Contradictions sort severity-descending (critical first), id ascending
    /// as tiebreak; unparseable severity sorts last and is echoed raw.
    #[test]
    fn contradictions_sort_by_severity_descending_then_id() {
        let session = session_with(vec![
            contradiction_node("conflict.medium", "medium", "unresolved", &["c.a", "c.b"]),
            contradiction_node("conflict.low", "low", "unresolved", &["c.a", "c.b"]),
            contradiction_node("conflict.garbage", "panic", "unresolved", &["c.a", "c.b"]),
            contradiction_node(
                "conflict.critical-z",
                "critical",
                "unresolved",
                &["c.a", "c.b"],
            ),
            contradiction_node(
                "conflict.critical-a",
                "critical",
                "unresolved",
                &["c.a", "c.b"],
            ),
            contradiction_node("conflict.high", "high", "unresolved", &["c.a", "c.b"]),
        ]);

        let envelope = evaluate_contradictions(&session, false, Vec::new());

        let ids: Vec<&str> = envelope
            .contradictions
            .iter()
            .map(|record| record.id.as_str())
            .collect();
        assert_eq!(
            ids,
            vec![
                "conflict.critical-a",
                "conflict.critical-z",
                "conflict.high",
                "conflict.medium",
                "conflict.low",
                "conflict.garbage",
            ]
        );
        assert_eq!(envelope.contradictions[5].severity, "panic");
    }

    /// ADR-0039: the top-level `severity` field is the sole carrier — a stray
    /// fields["severity"] entry (impossible on a v4 artifact) is ignored.
    #[test]
    fn contradiction_severity_reads_top_level_field_only() {
        let mut node = contradiction_node("conflict.dual", "critical", "unresolved", &["c.a"]);
        let GraphNode::KnowledgeObject(ko) = &mut node else {
            unreachable!();
        };
        ko.fields.insert("severity".to_string(), "low".to_string());

        let session = session_with(vec![node]);
        let envelope = evaluate_contradictions(&session, false, Vec::new());

        assert_eq!(envelope.contradictions[0].severity, "critical");
    }

    /// A claim implicated by an unresolved contradiction derives
    /// `effective_status: "contradicted"` with the build-format reason, even
    /// when its authored status is untouched.
    #[test]
    fn implicated_claim_derives_contradicted_with_reason() {
        let session = session_with(vec![
            contradiction_node(
                "conflict.one",
                "high",
                "unresolved",
                &["claim.csrf", "claim.mem"],
            ),
            ko_node("claim.csrf", "claim", Some("accepted"), &[]),
            ko_node("claim.mem", "claim", Some("contradicted"), &[]),
        ]);

        let envelope = evaluate_contradictions(&session, false, Vec::new());

        assert_eq!(envelope.contradicted_claims.len(), 2);
        let csrf = &envelope.contradicted_claims[0];
        assert_eq!(csrf.id, "claim.csrf");
        assert_eq!(csrf.authored_status.as_deref(), Some("accepted"));
        assert_eq!(csrf.effective_status.as_deref(), Some("contradicted"));
        assert_eq!(
            csrf.effective_reason.as_deref(),
            Some("contradiction:conflict.one")
        );
        assert_eq!(csrf.contradiction_ids, vec!["conflict.one"]);
    }

    /// Two unresolved contradictions on one claim: `contradiction_ids` sorted
    /// ascending, reason from the lexicographically smallest.
    #[test]
    fn multiple_contradictions_on_one_claim_sort_ids_and_use_smallest_for_reason() {
        let session = session_with(vec![
            contradiction_node(
                "conflict.zeta",
                "high",
                "unresolved",
                &["claim.x", "claim.y"],
            ),
            contradiction_node(
                "conflict.alpha",
                "low",
                "unresolved",
                &["claim.x", "claim.z"],
            ),
            ko_node("claim.x", "claim", Some("verified"), &[]),
            ko_node("claim.y", "claim", Some("verified"), &[]),
            ko_node("claim.z", "claim", Some("verified"), &[]),
        ]);

        let envelope = evaluate_contradictions(&session, false, Vec::new());

        let x = envelope
            .contradicted_claims
            .iter()
            .find(|record| record.id == "claim.x")
            .expect("claim.x is implicated twice");
        assert_eq!(x.contradiction_ids, vec!["conflict.alpha", "conflict.zeta"]);
        assert_eq!(
            x.effective_reason.as_deref(),
            Some("contradiction:conflict.alpha")
        );
    }

    /// A claim with authored status `contradicted` but no implicating
    /// unresolved contradiction is still listed — empty ids, echoed status,
    /// no derivation reason.
    #[test]
    fn orphaned_authored_contradicted_claim_is_listed_with_empty_ids() {
        let session = session_with(vec![ko_node(
            "claim.orphan",
            "claim",
            Some("contradicted"),
            &[],
        )]);

        let envelope = evaluate_contradictions(&session, false, Vec::new());

        assert_eq!(envelope.contradicted_claims.len(), 1);
        let orphan = &envelope.contradicted_claims[0];
        assert_eq!(orphan.authored_status.as_deref(), Some("contradicted"));
        assert_eq!(orphan.effective_status.as_deref(), Some("contradicted"));
        assert_eq!(orphan.effective_reason, None);
        assert!(orphan.contradiction_ids.is_empty());
    }

    /// The contradictions query is clock-free: an expired verified claim
    /// implicated by an unresolved contradiction reports `contradicted` on
    /// this axis (the expiry axis is `adoc stale`'s job; the build artifact's
    /// single effective_status slot keeps stale precedence).
    #[test]
    fn expired_verified_implicated_claim_still_reports_contradicted() {
        let session = session_with(vec![
            contradiction_node(
                "conflict.one",
                "high",
                "unresolved",
                &["claim.expired", "claim.b"],
            ),
            ko_node(
                "claim.expired",
                "claim",
                Some("verified"),
                &[("expires_at", "2000-01-01")],
            ),
            ko_node("claim.b", "claim", Some("verified"), &[]),
        ]);

        let envelope = evaluate_contradictions(&session, false, Vec::new());

        let expired = envelope
            .contradicted_claims
            .iter()
            .find(|record| record.id == "claim.expired")
            .expect("expired claim is listed");
        assert_eq!(expired.effective_status.as_deref(), Some("contradicted"));
    }

    /// Ids in `claims:` that name a non-claim object or nothing at all produce
    /// no `contradicted_claims` record (compile already diagnosed them).
    #[test]
    fn non_claim_and_dangling_claim_references_produce_no_records() {
        let session = session_with(vec![
            contradiction_node(
                "conflict.one",
                "high",
                "unresolved",
                &["decision.not-a-claim", "claim.gone"],
            ),
            ko_node("decision.not-a-claim", "decision", Some("accepted"), &[]),
        ]);

        let envelope = evaluate_contradictions(&session, false, Vec::new());

        assert_eq!(envelope.contradictions.len(), 1);
        assert!(
            envelope.contradicted_claims.is_empty(),
            "non-claim and dangling references must not yield records: {:#?}",
            envelope.contradicted_claims
        );
    }

    /// The envelope is artifact-pure: schema version constant, NO evaluated_at
    /// key, optional record fields omitted rather than null.
    #[test]
    fn contradictions_envelope_serializes_without_evaluated_at() {
        let session = session_with(vec![
            contradiction_node(
                "conflict.one",
                "high",
                "unresolved",
                &["claim.a", "claim.b"],
            ),
            ko_node("claim.a", "claim", None, &[]),
        ]);

        let envelope = evaluate_contradictions(&session, false, Vec::new());
        let value = serde_json::to_value(&envelope).expect("envelope serializes");
        let object = value.as_object().expect("envelope is an object");

        assert_eq!(value["schema_version"], "adoc.contradictions.v0");
        assert!(
            !object.contains_key("evaluated_at"),
            "contradictions is clock-free — no evaluated_at"
        );
        assert_eq!(value["diagnostics"], serde_json::json!([]));

        let claim = value["contradicted_claims"][0]
            .as_object()
            .expect("claim record is an object");
        assert!(!claim.contains_key("authored_status"));
        assert_eq!(claim["effective_status"], "contradicted");

        let empty = empty_contradictions_envelope(Vec::new());
        let empty_value = serde_json::to_value(&empty).expect("empty envelope serializes");
        assert_eq!(empty_value["contradictions"], serde_json::json!([]));
        assert_eq!(empty_value["contradicted_claims"], serde_json::json!([]));
    }

    // --- V6.3 `adoc impacted-by` ---

    fn rel(s: &str) -> RelPath {
        RelPath::try_new(s).expect("valid test path")
    }

    /// Full evaluation: impacts hit + evidence-ref hit, sorted records, one
    /// merged obligation per record, normalized changed_paths.
    #[test]
    fn evaluate_impacted_emits_records_and_obligations() {
        let mut claim = ko_node(
            "billing.refunds",
            "claim",
            Some("verified"),
            &[("owner", "team-billing")],
        );
        if let GraphNode::KnowledgeObject(node) = &mut claim {
            node.impacts = vec!["crates/billing/src/refund.rs".to_string()];
        }
        let mut decision = ko_node("billing.use-ledger", "decision", Some("accepted"), &[]);
        if let GraphNode::KnowledgeObject(node) = &mut decision {
            node.evidence = vec![crate::domain::graph::GraphEvidence::object_ref(
                "source_code",
                "billing.consume-use-case",
            )];
        }
        let source = ko_node(
            "billing.consume-use-case",
            "source",
            None,
            &[("path", "src/consume.use-case.ts")],
        );
        let draft = ko_node("billing.draft", "claim", Some("draft"), &[]);
        let session = session_with(vec![claim, decision, source, draft]);

        let changed = [
            rel("src/consume.use-case.ts"),
            rel("crates/billing/src/refund.rs"),
            rel("crates/billing/src/refund.rs"), // duplicate input normalizes away
            rel("unrelated.rs"),
        ];
        let envelope = evaluate_impacted(&session, &changed, Vec::new());

        assert_eq!(envelope.schema_version, "adoc.impacted.v0");
        assert_eq!(
            envelope.changed_paths,
            vec![
                "crates/billing/src/refund.rs",
                "src/consume.use-case.ts",
                "unrelated.rs",
            ],
            "changed_paths sorted ascending, deduplicated"
        );

        let ids: Vec<&str> = envelope
            .impacted
            .iter()
            .map(|record| record.id.as_str())
            .collect();
        assert_eq!(ids, vec!["billing.refunds", "billing.use-ledger"]);

        let refunds = &envelope.impacted[0];
        assert_eq!(refunds.kind, "claim");
        assert_eq!(refunds.status, "verified");
        assert_eq!(refunds.owner.as_deref(), Some("team-billing"));
        assert_eq!(
            refunds.reasons,
            vec![ImpactReason {
                kind: ImpactReasonKind::ImpactsPath,
                matched_path: "crates/billing/src/refund.rs".to_string(),
                via_source_object: None,
            }]
        );

        let ledger = &envelope.impacted[1];
        assert_eq!(ledger.kind, "decision");
        assert_eq!(ledger.status, "accepted");
        assert_eq!(ledger.owner, None);
        assert_eq!(
            ledger.reasons,
            vec![ImpactReason {
                kind: ImpactReasonKind::EvidencePath,
                matched_path: "src/consume.use-case.ts".to_string(),
                via_source_object: Some("billing.consume-use-case".to_string()),
            }]
        );

        assert_eq!(envelope.proof_obligations.len(), 2);
        let obligation = &envelope.proof_obligations[0];
        assert_eq!(obligation.object_id, "billing.refunds");
        assert_eq!(obligation.reason, "review impacted claim");
        assert_eq!(obligation.required_evidence, vec!["source_code"]);
        assert_eq!(
            envelope.proof_obligations[1].object_id,
            "billing.use-ledger"
        );
    }

    #[test]
    fn evaluate_impacted_empty_changed_set_yields_empty_listing() {
        let session = session_with(vec![ko_node(
            "billing.refunds",
            "claim",
            Some("verified"),
            &[],
        )]);

        let envelope = evaluate_impacted(&session, &[], Vec::new());

        assert!(envelope.changed_paths.is_empty());
        assert!(envelope.impacted.is_empty());
        assert!(envelope.proof_obligations.is_empty());
    }

    /// Wire shape: clock-free, optional fields omitted (not null).
    #[test]
    fn impacted_envelope_serialization_is_clock_free_and_omits_optionals() {
        let mut claim = ko_node("billing.refunds", "claim", Some("verified"), &[]);
        if let GraphNode::KnowledgeObject(node) = &mut claim {
            node.impacts = vec!["a.rs".to_string()];
        }
        let session = session_with(vec![claim]);

        let envelope = evaluate_impacted(&session, &[rel("a.rs")], Vec::new());
        let value = serde_json::to_value(&envelope).expect("envelope serializes");

        let object = value.as_object().expect("envelope is an object");
        assert!(
            !object.contains_key("evaluated_at"),
            "impacted is clock-free — no evaluated_at"
        );
        assert_eq!(value["schema_version"], "adoc.impacted.v0");

        let record = value["impacted"][0]
            .as_object()
            .expect("impacted record is an object");
        assert!(!record.contains_key("owner"), "owner omitted when absent");

        let reason = value["impacted"][0]["reasons"][0]
            .as_object()
            .expect("reason is an object");
        assert_eq!(reason["kind"], "impacts_path");
        assert!(
            !reason.contains_key("via_source_object"),
            "via_source_object omitted when absent"
        );

        let empty = empty_impacted_envelope(vec!["a.rs".to_string()], Vec::new());
        let empty_value = serde_json::to_value(&empty).expect("empty envelope serializes");
        assert_eq!(empty_value["changed_paths"], serde_json::json!(["a.rs"]));
        assert_eq!(empty_value["impacted"], serde_json::json!([]));
        assert_eq!(empty_value["proof_obligations"], serde_json::json!([]));
    }

    #[test]
    fn validate_changed_paths_collects_all_invalid_paths() {
        let valid = validate_changed_paths(&["src/a.rs".to_string(), "src/b.rs".to_string()])
            .expect("valid paths accepted");
        assert_eq!(valid.len(), 2);

        let diagnostics = validate_changed_paths(&[
            "/abs/path.rs".to_string(),
            "src/ok.rs".to_string(),
            "../escape.rs".to_string(),
        ])
        .expect_err("invalid paths rejected");
        assert_eq!(diagnostics.len(), 2, "all invalid paths collected");
        for diagnostic in &diagnostics {
            assert_eq!(diagnostic.code.as_str(), "impacted.invalid_path");
        }
    }

    #[test]
    fn changed_files_failure_diagnostics_split_user_vs_environment() {
        let unresolvable = changed_files_failure_diagnostic(
            &ChangedFilesError::UnresolvableBase {
                spec: "nope".to_string(),
                reason: "unknown revision".to_string(),
            },
            "nope",
        );
        assert_eq!(unresolvable.code.as_str(), "impacted.ref_unresolvable");
        assert!(unresolvable.message.contains("nope"));

        let unavailable = changed_files_failure_diagnostic(
            &ChangedFilesError::ProviderUnavailable {
                reason: "git not found".to_string(),
            },
            "main",
        );
        assert_eq!(unavailable.code.as_str(), "impacted.git_unavailable");

        let io = changed_files_failure_diagnostic(
            &ChangedFilesError::Io(std::io::Error::other("boom")),
            "main",
        );
        assert_eq!(io.code.as_str(), "impacted.git_unavailable");
    }

    /// Summary is the first non-empty line, char-truncated to 120 with `…`.
    #[test]
    fn body_summary_takes_first_non_empty_line_and_truncates_chars() {
        assert_eq!(body_summary("First line.\nSecond line."), "First line.");
        assert_eq!(body_summary("\n  \nActual start.\nMore."), "Actual start.");
        assert_eq!(body_summary(""), "");

        let long = "x".repeat(121);
        let summary = body_summary(&long);
        assert_eq!(summary.chars().count(), 120);
        assert!(summary.ends_with('…'));

        let exactly = "y".repeat(120);
        assert_eq!(body_summary(&exactly), exactly);

        // Multibyte: truncation must count chars, not bytes.
        let multibyte = "é".repeat(130);
        let multibyte_summary = body_summary(&multibyte);
        assert_eq!(multibyte_summary.chars().count(), 120);
        assert_eq!(multibyte_summary, format!("{}…", "é".repeat(119)));
    }
}
