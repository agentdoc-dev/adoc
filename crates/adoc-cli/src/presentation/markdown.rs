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
    ChangedObject, Diagnostic, FieldChange, ImpactReasonKind, ImpactedEnvelope, ImpactedObject,
    ObjectDiffEnvelope, ProofObligation, RequiredReviewer, ReviewEnvelope,
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

    /// V6.3 `adoc impacted-by --format markdown`: the PR-comment shape —
    /// `## Impacted by` header with code-quoted changed paths, one bullet per
    /// impacted object with its reasons, then the same proof-obligations
    /// task list as `adoc review`.
    pub(crate) fn write_impacted(
        envelope: &ImpactedEnvelope,
        out: &mut dyn io::Write,
    ) -> io::Result<()> {
        let mut body = String::new();
        render_impacted_by(&mut body, envelope);
        if !envelope.proof_obligations.is_empty() {
            render_obligations(&mut body, &envelope.proof_obligations);
        }
        out.write_all(body.as_bytes())
    }

    /// V6.3 `adoc impacted-by --format markdown` refusal path: one blockquote
    /// per diagnostic, so a PR-comment consumer pasting stdout never posts an
    /// empty comment (JSON gets the envelope; plain/styled use stderr only).
    pub(crate) fn write_impacted_error(
        diagnostics: &[Diagnostic],
        out: &mut dyn io::Write,
    ) -> io::Result<()> {
        let mut body = String::new();
        for diagnostic in diagnostics {
            // Diagnostic messages can be multi-line (git stderr); every line
            // gets the `> ` prefix so strict renderers keep the blockquote.
            let message = diagnostic.message.replace('\n', "\n> ");
            writeln!(
                body,
                "> ⚠️ adoc impacted-by failed: `{}` — {message}",
                diagnostic.code
            )
            .expect("write to String");
        }
        out.write_all(body.as_bytes())
    }
}

fn render_impacted_by(output: &mut String, envelope: &ImpactedEnvelope) {
    // An empty changed set (e.g. `--ref` with no diff) gets an explicit
    // marker so a PR-comment reader can tell "ref had no diff" apart from
    // a header that simply lost its path list.
    if envelope.changed_paths.is_empty() {
        writeln!(output, "## Impacted by: _(no changed paths)_").expect("write to String");
    } else {
        let paths: Vec<String> = envelope
            .changed_paths
            .iter()
            .map(|path| format!("`{path}`"))
            .collect();
        writeln!(output, "## Impacted by: {}", paths.join(", ")).expect("write to String");
    }
    if envelope.impacted.is_empty() {
        writeln!(output).expect("write to String");
        writeln!(output, "No impacted Knowledge Objects.").expect("write to String");
    }
    for record in &envelope.impacted {
        let reasons: Vec<String> = record
            .reasons
            .iter()
            .map(|reason| {
                let label = match reason.kind {
                    ImpactReasonKind::ImpactsPath => "impacts",
                    ImpactReasonKind::EvidencePath => "evidence",
                };
                match &reason.via_source_object {
                    Some(source) => {
                        format!("`{}` ({label} via `{source}`)", reason.matched_path)
                    }
                    None => format!("`{}` ({label})", reason.matched_path),
                }
            })
            .collect();
        writeln!(output, "- `{}` → {}", record.id, reasons.join(", ")).expect("write to String");
    }
}

/// One section of the rendered Markdown output. Variants carry exactly the
/// data each section needs; the order they're emitted is the order of the
/// slice passed to `drive`. Section-omission ("don't print empty sections")
/// is a property of the enum, not control flow in `render_markdown`.
enum ReviewSection<'a> {
    ReviewersHeader(&'a [RequiredReviewer]),
    Summary {
        created: usize,
        deleted: usize,
        changed: usize,
    },
    ChangedDetails {
        entries: &'a [ChangedObject],
        obligations: &'a [ProofObligation],
    },
    CreatedIds(Vec<&'a str>),
    DeletedIds(Vec<&'a str>),
    Impact(&'a [ImpactedObject]),
    Obligations(&'a [ProofObligation]),
}

impl<'a> ReviewSection<'a> {
    fn is_empty(&self) -> bool {
        match self {
            Self::ReviewersHeader(reviewers) => reviewers.is_empty(),
            // Summary is the diff heading — always rendered, even when every
            // count is zero, so the reader can tell "no changes" from "tool
            // didn't run".
            Self::Summary { .. } => false,
            Self::ChangedDetails { entries, .. } => entries.is_empty(),
            Self::CreatedIds(ids) | Self::DeletedIds(ids) => ids.is_empty(),
            Self::Impact(impact) => impact.is_empty(),
            Self::Obligations(obligations) => obligations.is_empty(),
        }
    }

    fn render(&self, output: &mut String) {
        match self {
            Self::ReviewersHeader(reviewers) => render_reviewers_header(output, reviewers),
            Self::Summary {
                created,
                deleted,
                changed,
            } => render_summary(output, *created, *deleted, *changed),
            Self::ChangedDetails {
                entries,
                obligations,
            } => render_changed(output, entries, obligations),
            Self::CreatedIds(ids) => render_id_list_section(output, "Created", ids),
            Self::DeletedIds(ids) => render_id_list_section(output, "Deleted", ids),
            Self::Impact(impact) => render_impact(output, impact),
            Self::Obligations(obligations) => render_obligations(output, obligations),
        }
    }
}

fn render_markdown(
    diff: &ObjectDiffEnvelope,
    reviewers: &[RequiredReviewer],
    impact: &[ImpactedObject],
    obligations: &[ProofObligation],
) -> String {
    let created_ids: Vec<&str> = diff.created_ids().collect();
    let deleted_ids: Vec<&str> = diff.deleted_ids().collect();

    let sections = [
        ReviewSection::ReviewersHeader(reviewers),
        ReviewSection::Summary {
            created: diff.created_count(),
            deleted: diff.deleted_count(),
            changed: diff.changed_count(),
        },
        ReviewSection::ChangedDetails {
            entries: diff.changed(),
            obligations,
        },
        ReviewSection::CreatedIds(created_ids),
        ReviewSection::DeletedIds(deleted_ids),
        ReviewSection::Impact(impact),
        ReviewSection::Obligations(obligations),
    ];

    let mut output = String::new();
    drive(&mut output, &sections);
    output
}

fn drive(output: &mut String, sections: &[ReviewSection<'_>]) {
    for section in sections {
        if !section.is_empty() {
            section.render(output);
        }
    }
}

fn render_reviewers_header(output: &mut String, reviewers: &[RequiredReviewer]) {
    let mentions: Vec<String> = reviewers.iter().map(|r| format!("@{}", r.owner)).collect();
    writeln!(output, "**Required reviewers:** {}", mentions.join(" ")).expect("write to String");
    writeln!(output).expect("write to String");
}

fn render_summary(output: &mut String, created: usize, deleted: usize, changed: usize) {
    writeln!(
        output,
        "## Diff: {created} created, {deleted} deleted, {changed} changed",
    )
    .expect("write to String");
}

fn render_changed(output: &mut String, entries: &[ChangedObject], obligations: &[ProofObligation]) {
    for entry in entries {
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
        let label = change.summary_label();
        if !labels.contains(&label) {
            labels.push(label);
        }
    }
    if labels.is_empty() {
        return "no field changes detected".to_string();
    }
    labels.join(", ")
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
                writeln!(output, "+ {}: {target}", kind.as_str()).expect("write to String");
            }
            FieldChange::RelationRemoved { kind, target } => {
                writeln!(output, "- {}: {target}", kind.as_str()).expect("write to String");
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

fn render_id_list_section(output: &mut String, label: &str, ids: &[&str]) {
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

    #[test]
    fn empty_section_is_skipped_by_drive() {
        // Property test of the ReviewSection enum: an empty section never
        // renders. The "diff command never emits ❌" invariant from V3-DESIGN
        // §V3.5 reduces to "diff passes an empty obligations slice" — the
        // Obligations section then has is_empty() = true and is skipped.
        let mut output = String::new();
        let empty_obligations: &[ProofObligation] = &[];
        drive(
            &mut output,
            &[ReviewSection::Obligations(empty_obligations)],
        );
        assert_eq!(output, "");
    }

    #[test]
    fn impacted_by_header_marks_empty_changed_paths() {
        // Reachable via `--ref <ref>` with no diff: valid envelope, empty
        // changed_paths. The header must say so instead of rendering
        // `## Impacted by: ` with a trailing space.
        let envelope = ImpactedEnvelope::new(Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let mut output = String::new();
        render_impacted_by(&mut output, &envelope);
        assert_eq!(
            output,
            "## Impacted by: _(no changed paths)_\n\nNo impacted Knowledge Objects.\n"
        );
    }

    #[test]
    fn impacted_error_renders_one_blockquote_per_diagnostic() {
        use adoc_core::{DiagnosticCode, Severity};
        let diagnostics = vec![Diagnostic {
            code: DiagnosticCode::ImpactedRefUnresolvable,
            severity: Severity::Error,
            message: "ref `nope` did not resolve".to_string(),
            span: None,
            object_id: None,
            help: None,
        }];
        let mut out = Vec::new();
        MarkdownReviewPresenter::write_impacted_error(&diagnostics, &mut out)
            .expect("write to Vec");
        assert_eq!(
            String::from_utf8(out).expect("utf8"),
            "> ⚠️ adoc impacted-by failed: `impacted.ref_unresolvable` — ref `nope` did not resolve\n"
        );
    }

    #[test]
    fn impacted_error_quotes_every_line_of_a_multiline_message() {
        // Git stderr is multi-line (e.g. `fatal: ambiguous argument` plus a
        // hint). Every line must carry the `> ` prefix — GFM lazy
        // continuation would render unprefixed lines, but strict renderers
        // and markdown linters break the quote there.
        use adoc_core::{DiagnosticCode, Severity};
        let diagnostics = vec![Diagnostic {
            code: DiagnosticCode::ImpactedRefUnresolvable,
            severity: Severity::Error,
            message: "fatal: ambiguous argument 'nope'\nUse '--' to separate paths".to_string(),
            span: None,
            object_id: None,
            help: None,
        }];
        let mut out = Vec::new();
        MarkdownReviewPresenter::write_impacted_error(&diagnostics, &mut out)
            .expect("write to Vec");
        assert_eq!(
            String::from_utf8(out).expect("utf8"),
            "> ⚠️ adoc impacted-by failed: `impacted.ref_unresolvable` — fatal: ambiguous argument 'nope'\n> Use '--' to separate paths\n"
        );
    }

    #[test]
    fn summary_section_always_renders_even_with_all_zero_counts() {
        let mut output = String::new();
        drive(
            &mut output,
            &[ReviewSection::Summary {
                created: 0,
                deleted: 0,
                changed: 0,
            }],
        );
        assert_eq!(output, "## Diff: 0 created, 0 deleted, 0 changed\n");
    }
}
