//! V3.5 Markdown presenter for `adoc diff` and `adoc review`.
//!
//! Produces GitHub-flavored Markdown that pastes cleanly into a PR review
//! comment. The shape is frozen at slice time:
//!
//! 1. **Required reviewers** header line (review only, non-empty).
//! 2. `## Diff: N created, M deleted, K changed` summary heading.
//! 3. One `<details>` per `ChangedObject` in envelope order — summary line
//!    starts with a status icon, body contains fenced diffs and per-field rows.
//! 4. `## Created` and `## Deleted` bullet sections, only when non-empty.
//! 5. `## Impact` section (review only, non-empty).
//! 6. `## Proof obligations` GitHub task-list checklist (review only,
//!    non-empty).
//!
//! Status icons:
//! - `❌` if any proof obligation targets the changed object's id.
//! - `⚠️` if head.status == "needs_review".
//! - `✅` if head.status == "verified".
//! - `📝` otherwise.
//!
//! `adoc diff --format markdown` passes an empty obligations slice, so a
//! body-changed verified claim renders `✅` from `diff` and `❌` from
//! `review` — predictable difference between the two commands.
//!
//! See V3-DESIGN.md §V3.5 and the golden fixtures under
//! `crates/adoc-cli/tests/fixtures/review_markdown/`.

use std::fmt::Write as FmtWrite;
use std::io;

use adoc_core::{
    ChangedObject, FieldChange, ImpactedObject, ObjectDiffEnvelope, ProofObligation, RelationKind,
    RequiredReviewer, ReviewEnvelope,
};

pub(crate) struct MarkdownReviewPresenter;

impl MarkdownReviewPresenter {
    pub(crate) fn write_diff(
        envelope: &ObjectDiffEnvelope,
        out: &mut dyn io::Write,
    ) -> io::Result<()> {
        let body = render_markdown(envelope, &[], &[], &[]);
        out.write_all(body.as_bytes())
    }

    pub(crate) fn write_review(
        envelope: &ReviewEnvelope,
        out: &mut dyn io::Write,
    ) -> io::Result<()> {
        let body = render_markdown(
            &envelope.diff,
            &envelope.required_reviewers,
            &envelope.impact,
            &envelope.proof_obligations,
        );
        out.write_all(body.as_bytes())
    }
}

fn render_markdown(
    diff: &ObjectDiffEnvelope,
    reviewers: &[RequiredReviewer],
    impact: &[ImpactedObject],
    obligations: &[ProofObligation],
) -> String {
    let mut output = String::new();
    if !reviewers.is_empty() {
        render_reviewers_header(&mut output, reviewers);
    }
    render_summary(&mut output, diff);
    render_changed(&mut output, diff, obligations);
    if diff.created_count() > 0 {
        render_id_list_section(&mut output, "Created", diff.created_ids());
    }
    if diff.deleted_count() > 0 {
        render_id_list_section(&mut output, "Deleted", diff.deleted_ids());
    }
    if !impact.is_empty() {
        render_impact(&mut output, impact);
    }
    if !obligations.is_empty() {
        render_obligations(&mut output, obligations);
    }
    output
}

fn render_reviewers_header(output: &mut String, reviewers: &[RequiredReviewer]) {
    let mentions: Vec<String> = reviewers.iter().map(|r| format!("@{}", r.owner)).collect();
    writeln!(output, "**Required reviewers:** {}", mentions.join(" ")).expect("write to String");
    writeln!(output).expect("write to String");
}

fn render_summary(output: &mut String, envelope: &ObjectDiffEnvelope) {
    writeln!(
        output,
        "## Diff: {} created, {} deleted, {} changed",
        envelope.created_count(),
        envelope.deleted_count(),
        envelope.changed_count()
    )
    .expect("write to String");
}

fn render_changed(
    output: &mut String,
    envelope: &ObjectDiffEnvelope,
    obligations: &[ProofObligation],
) {
    for entry in envelope.changed() {
        writeln!(output).expect("write to String");
        let icon = icon_for(entry, obligations);
        let labels = field_change_summary_labels(entry.field_changes());
        writeln!(
            output,
            "<details><summary>{icon} <code>{id}</code> — {labels}</summary>",
            id = entry.id,
        )
        .expect("write to String");
        writeln!(output).expect("write to String");
        render_field_changes(output, entry.field_changes());
        writeln!(output).expect("write to String");
        writeln!(output, "</details>").expect("write to String");
    }
}

fn icon_for(entry: &ChangedObject, obligations: &[ProofObligation]) -> &'static str {
    icon_for_status(&entry.id, entry.head_status(), obligations)
}

/// Pure icon-decision rule, extracted so unit tests can exercise the four
/// branches without constructing a `ChangedObject` (whose constructor is
/// `pub(crate)` to adoc-core).
fn icon_for_status(
    id: &str,
    head_status: Option<&str>,
    obligations: &[ProofObligation],
) -> &'static str {
    if obligations.iter().any(|o| o.object_id == id) {
        return "❌";
    }
    match head_status {
        Some("needs_review") => "⚠️",
        Some("verified") => "✅",
        _ => "📝",
    }
}

fn field_change_summary_labels(changes: &[FieldChange]) -> String {
    let mut labels: Vec<&'static str> = Vec::new();
    for change in changes {
        let label = field_change_summary_label(change);
        if !labels.contains(&label) {
            labels.push(label);
        }
    }
    if labels.is_empty() {
        return "no field changes detected".to_string();
    }
    labels.join(", ")
}

fn field_change_summary_label(change: &FieldChange) -> &'static str {
    match change {
        FieldChange::Body { .. } => "body changed",
        FieldChange::Status { .. } => "status changed",
        FieldChange::Owner { .. } => "owner changed",
        FieldChange::VerifiedAt { .. } => "verified_at changed",
        FieldChange::EvidenceAdded { .. } => "evidence added",
        FieldChange::EvidenceRemoved { .. } => "evidence removed",
        FieldChange::RelationAdded { .. } => "relation added",
        FieldChange::RelationRemoved { .. } => "relation removed",
        FieldChange::ImpactsAdded { .. } => "impacts added",
        FieldChange::ImpactsRemoved { .. } => "impacts removed",
        // `FieldChange` is `#[non_exhaustive]`; future slices may add variants.
        _ => "field change",
    }
}

fn render_field_changes(output: &mut String, changes: &[FieldChange]) {
    for change in changes {
        match change {
            FieldChange::Body { before, after } => render_body_diff(output, before, after),
            FieldChange::Status { before, after } => {
                writeln!(
                    output,
                    "**status:** {} → {}",
                    optional(before.as_deref()),
                    optional(after.as_deref())
                )
                .expect("write to String");
            }
            FieldChange::Owner { before, after } => {
                writeln!(
                    output,
                    "**owner:** {} → {}",
                    optional(before.as_deref()),
                    optional(after.as_deref())
                )
                .expect("write to String");
            }
            FieldChange::VerifiedAt { before, after } => {
                writeln!(
                    output,
                    "**verified_at:** {} → {}",
                    optional(before.as_deref()),
                    optional(after.as_deref())
                )
                .expect("write to String");
            }
            FieldChange::EvidenceAdded { field, value } => {
                writeln!(output, "+ evidence.{field}: {value}").expect("write to String");
            }
            FieldChange::EvidenceRemoved { field, value } => {
                writeln!(output, "- evidence.{field}: {value}").expect("write to String");
            }
            FieldChange::RelationAdded { kind, target } => {
                writeln!(output, "+ {}: {target}", relation_kind_label(*kind))
                    .expect("write to String");
            }
            FieldChange::RelationRemoved { kind, target } => {
                writeln!(output, "- {}: {target}", relation_kind_label(*kind))
                    .expect("write to String");
            }
            FieldChange::ImpactsAdded { path } => {
                writeln!(output, "+ impacts: {path}").expect("write to String");
            }
            FieldChange::ImpactsRemoved { path } => {
                writeln!(output, "- impacts: {path}").expect("write to String");
            }
            // Same fallback rule as commands/diff.rs:150 — a future variant
            // surfaces with a stub line so the CLI keeps rendering when the
            // wire envelope ships ahead of the presenter.
            _ => {
                writeln!(
                    output,
                    "_field_change: (unsupported variant; upgrade the CLI)_"
                )
                .expect("write to String");
            }
        }
    }
}

fn render_body_diff(output: &mut String, before: &str, after: &str) {
    writeln!(output, "```diff").expect("write to String");
    for line in before.lines() {
        writeln!(output, "- {line}").expect("write to String");
    }
    for line in after.lines() {
        writeln!(output, "+ {line}").expect("write to String");
    }
    writeln!(output, "```").expect("write to String");
}

fn render_id_list_section<'a, I: Iterator<Item = &'a str>>(
    output: &mut String,
    label: &str,
    ids: I,
) {
    writeln!(output).expect("write to String");
    writeln!(output, "## {label}").expect("write to String");
    for id in ids {
        writeln!(output, "- `{id}`").expect("write to String");
    }
}

fn render_impact(output: &mut String, impact: &[ImpactedObject]) {
    writeln!(output).expect("write to String");
    writeln!(output, "## Impact").expect("write to String");
    for entry in impact {
        let paths: Vec<String> = entry.paths.iter().map(|p| format!("`{p}`")).collect();
        writeln!(output, "- `{}` → {}", entry.id, paths.join(", ")).expect("write to String");
    }
}

fn render_obligations(output: &mut String, obligations: &[ProofObligation]) {
    writeln!(output).expect("write to String");
    writeln!(output, "## Proof obligations").expect("write to String");
    for obligation in obligations {
        if obligation.required_evidence.is_empty() {
            writeln!(
                output,
                "- [ ] `{}`: {}",
                obligation.object_id, obligation.reason
            )
            .expect("write to String");
        } else {
            writeln!(
                output,
                "- [ ] `{}`: {} (evidence: {})",
                obligation.object_id,
                obligation.reason,
                obligation.required_evidence.join(", ")
            )
            .expect("write to String");
        }
    }
}

fn optional(value: Option<&str>) -> &str {
    value.unwrap_or("(none)")
}

fn relation_kind_label(kind: RelationKind) -> &'static str {
    match kind {
        RelationKind::DependsOn => "depends_on",
        RelationKind::Supersedes => "supersedes",
        RelationKind::RelatedTo => "related_to",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obligation(object_id: &str) -> ProofObligation {
        ProofObligation {
            object_id: object_id.to_string(),
            reason: "re-verify body".to_string(),
            required_evidence: vec!["source".to_string()],
        }
    }

    #[test]
    fn icon_falls_back_to_cross_when_obligation_targets_object() {
        let obligations = vec![obligation("billing.refunds")];
        let icon = icon_for_status("billing.refunds", Some("verified"), &obligations);
        assert_eq!(icon, "❌");
    }

    #[test]
    fn icon_warns_when_head_status_is_needs_review_and_no_obligation() {
        let icon = icon_for_status("billing.refunds", Some("needs_review"), &[]);
        assert_eq!(icon, "⚠️");
    }

    #[test]
    fn icon_checkmarks_verified_head_status_without_obligation() {
        let icon = icon_for_status("billing.refunds", Some("verified"), &[]);
        assert_eq!(icon, "✅");
    }

    #[test]
    fn icon_falls_through_to_memo_for_other_statuses() {
        assert_eq!(icon_for_status("x.y", Some("draft"), &[]), "📝");
        assert_eq!(icon_for_status("x.y", None, &[]), "📝");
    }

    #[test]
    fn icon_obligation_takes_precedence_over_needs_review() {
        let obligations = vec![obligation("billing.refunds")];
        let icon = icon_for_status("billing.refunds", Some("needs_review"), &obligations);
        assert_eq!(icon, "❌");
    }

    #[test]
    fn icon_unrelated_obligation_does_not_change_verified_icon() {
        let obligations = vec![obligation("other.id")];
        let icon = icon_for_status("billing.refunds", Some("verified"), &obligations);
        assert_eq!(icon, "✅");
    }

    #[test]
    fn field_change_summary_labels_dedupes_repeats() {
        let changes = vec![
            FieldChange::EvidenceAdded {
                field: "source".to_string(),
                value: "ledger".to_string(),
            },
            FieldChange::EvidenceAdded {
                field: "test".to_string(),
                value: "integration".to_string(),
            },
        ];
        assert_eq!(field_change_summary_labels(&changes), "evidence added");
    }

    #[test]
    fn field_change_summary_labels_joins_distinct_with_comma() {
        let changes = vec![
            FieldChange::Body {
                before: "a".to_string(),
                after: "b".to_string(),
            },
            FieldChange::Status {
                before: Some("verified".to_string()),
                after: Some("needs_review".to_string()),
            },
        ];
        assert_eq!(
            field_change_summary_labels(&changes),
            "body changed, status changed"
        );
    }

    #[test]
    fn body_diff_emits_minus_then_plus_lines_inside_diff_fence() {
        let mut output = String::new();
        render_body_diff(&mut output, "old line 1\nold line 2", "new line 1");
        assert_eq!(
            output,
            "```diff\n- old line 1\n- old line 2\n+ new line 1\n```\n"
        );
    }
}
