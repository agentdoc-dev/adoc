use std::fmt::Write as FmtWrite;
use std::io;
use std::io::Read;
use std::path::PathBuf;

use adoc_core::{PatchApplyResult, PatchCheckResult};
use adoc_local::{
    LocalContext, PatchApplyInput, PatchApplySource, PatchCheckInput, UnrestrictedPathPolicy,
};

use crate::error::CliError;
use crate::presentation::ResolvedFormat;
use crate::presentation::style::key::cyan_key;
use crate::presentation::style::kv::faint_label;

use super::{current_dir, eprint_diagnostics, report, write_json_or_report};

pub(crate) struct PatchCommandInput {
    pub(crate) check: Option<PathBuf>,
    pub(crate) apply: Option<String>,
    pub(crate) artifact: Option<PathBuf>,
}

pub(crate) fn patch(input: PatchCommandInput, resolved: ResolvedFormat) -> i32 {
    match (input.check, input.apply) {
        (Some(patch_path), None) => patch_check(patch_path, input.artifact, resolved),
        (None, Some(apply)) => patch_apply(apply, input.artifact, resolved),
        _ => unreachable!("clap enforces exactly one of --check/--apply"),
    }
}

fn patch_check(patch_path: PathBuf, artifact: Option<PathBuf>, resolved: ResolvedFormat) -> i32 {
    let config_start = match current_dir() {
        Ok(path) => path,
        Err(error) => return report(error),
    };
    let context = LocalContext::new(config_start, UnrestrictedPathPolicy);
    let outcome = match context.patch_check(PatchCheckInput {
        patch_path,
        artifact,
    }) {
        Ok(outcome) => outcome,
        Err(error) => return report(error.into()),
    };
    let result = outcome.result;
    let exit_code = outcome.exit_code;

    match resolved {
        ResolvedFormat::Json => write_json_or_report(&result, exit_code),
        ResolvedFormat::Plain => write_patch_text(&result, false, exit_code),
        ResolvedFormat::Styled => write_patch_text(&result, true, exit_code),
        ResolvedFormat::Markdown => {
            unreachable!("main.rs rejects markdown format for `adoc patch` before dispatch")
        }
    }
}

fn patch_apply(apply: String, artifact: Option<PathBuf>, resolved: ResolvedFormat) -> i32 {
    let source = if apply == "@-" {
        let mut buffer = String::new();
        if let Err(source) = io::stdin().read_to_string(&mut buffer) {
            return report(CliError::RetrievalIo { source });
        }
        match serde_json::from_str(&buffer) {
            Ok(value) => PatchApplySource::Inline(value),
            Err(error) => {
                eprintln!("error: stdin is not valid JSON: {error}");
                return 1;
            }
        }
    } else {
        PatchApplySource::Path(PathBuf::from(apply))
    };

    let config_start = match current_dir() {
        Ok(path) => path,
        Err(error) => return report(error),
    };
    let context = LocalContext::new(config_start, UnrestrictedPathPolicy);
    let outcome = match context.patch_apply(PatchApplyInput {
        patch: source,
        artifact,
        interface: "cli".to_string(),
    }) {
        Ok(outcome) => outcome,
        Err(error) => return report(error.into()),
    };
    let result = outcome.result;
    let exit_code = outcome.exit_code;

    match resolved {
        ResolvedFormat::Json => write_json_or_report(&result, exit_code),
        ResolvedFormat::Plain => write_apply_text(&result, false, exit_code),
        ResolvedFormat::Styled => write_apply_text(&result, true, exit_code),
        ResolvedFormat::Markdown => {
            unreachable!("main.rs rejects markdown format for `adoc patch` before dispatch")
        }
    }
}

fn write_apply_text(result: &PatchApplyResult, styled: bool, exit_code: i32) -> i32 {
    if !result.diagnostics.is_empty() {
        eprint_diagnostics(&result.diagnostics);
    }
    let mut output = String::new();
    render_apply_text(&mut output, result, styled);
    print!("{output}");
    exit_code
}

fn render_apply_text(output: &mut String, result: &PatchApplyResult, styled: bool) {
    let status = if result.applied { "applied" } else { "refused" };
    let label = |text: &str| {
        if styled {
            faint_label(text).to_string()
        } else {
            text.to_string()
        }
    };

    writeln!(output, "{} {status}", label("Status:")).expect("writing to String cannot fail");
    if let Some(target) = &result.target {
        writeln!(output, "{} {target}", label("Target:")).expect("writing to String cannot fail");
    }
    if !result.operation.is_empty() {
        if styled {
            writeln!(
                output,
                "{} {}",
                label("Operation:"),
                cyan_key(&result.operation)
            )
            .expect("writing to String cannot fail");
        } else {
            writeln!(output, "Operation: {}", result.operation)
                .expect("writing to String cannot fail");
        }
    }

    writeln!(output, "{}", label("Written Files:")).expect("writing to String cannot fail");
    if result.written_files.is_empty() {
        writeln!(output, "(none)").expect("writing to String cannot fail");
    } else {
        for file in &result.written_files {
            writeln!(
                output,
                "- {} ({} -> {})",
                file.path, file.before_file_hash, file.after_file_hash
            )
            .expect("writing to String cannot fail");
        }
    }

    if result.post_check.ran {
        writeln!(
            output,
            "{} {} errors, {} warnings",
            label("Post-check:"),
            result.post_check.error_count,
            result.post_check.warning_count
        )
        .expect("writing to String cannot fail");
    } else {
        writeln!(output, "{} not run", label("Post-check:"))
            .expect("writing to String cannot fail");
    }

    if result.artifacts_stale {
        writeln!(
            output,
            "{} stale — run `adoc build` to refresh dist artifacts",
            label("Artifacts:")
        )
        .expect("writing to String cannot fail");
    }
}

fn write_patch_text(result: &PatchCheckResult, styled: bool, exit_code: i32) -> i32 {
    if !result.diagnostics.is_empty() {
        eprint_diagnostics(&result.diagnostics);
    }
    let mut output = String::new();
    render_patch_text(&mut output, result, styled);
    print!("{output}");
    exit_code
}

fn render_patch_text(output: &mut String, result: &PatchCheckResult, styled: bool) {
    let status = if result.valid { "valid" } else { "invalid" };
    let accepted = if result.accepted_for_review {
        "accepted for review"
    } else {
        "not accepted for review"
    };

    if styled {
        writeln!(output, "{} {} ({accepted})", faint_label("Status:"), status)
            .expect("writing to String cannot fail");
        if let Some(target) = &result.target {
            writeln!(output, "{} {}", faint_label("Target:"), target)
                .expect("writing to String cannot fail");
        }
        if !result.operation.is_empty() {
            writeln!(
                output,
                "{} {}",
                faint_label("Operation:"),
                cyan_key(&result.operation)
            )
            .expect("writing to String cannot fail");
        }
    } else {
        writeln!(output, "Status: {status} ({accepted})").expect("writing to String cannot fail");
        if let Some(target) = &result.target {
            writeln!(output, "Target: {target}").expect("writing to String cannot fail");
        }
        if !result.operation.is_empty() {
            writeln!(output, "Operation: {}", result.operation)
                .expect("writing to String cannot fail");
        }
    }

    render_diffs(output, result, styled);
    render_relations(output, result, styled);
    render_proof_obligations(output, result, styled);
    render_follow_up(output, result, styled);
}

fn render_diffs(output: &mut String, result: &PatchCheckResult, styled: bool) {
    header(output, "Diffs:", styled);
    if result.diffs.is_empty() {
        writeln!(output, "(none)").expect("writing to String cannot fail");
        return;
    }
    for diff in &result.diffs {
        let old = diff
            .old
            .as_ref()
            .map(compact_json)
            .unwrap_or_else(|| "<none>".to_string());
        let new = diff
            .new
            .as_ref()
            .map(compact_json)
            .unwrap_or_else(|| "<none>".to_string());
        if styled {
            writeln!(output, "- {}: {old} -> {new}", cyan_key(&diff.field))
                .expect("writing to String cannot fail");
        } else {
            writeln!(output, "- {}: {old} -> {new}", diff.field)
                .expect("writing to String cannot fail");
        }
    }
}

fn render_relations(output: &mut String, result: &PatchCheckResult, styled: bool) {
    header(output, "Affected Relations:", styled);
    if result.affected_relations.is_empty() {
        writeln!(output, "(none)").expect("writing to String cannot fail");
        return;
    }
    for relation in &result.affected_relations {
        if styled {
            writeln!(
                output,
                "- {} {} --{}--> {}",
                relation.action,
                relation.source,
                cyan_key(relation.relation.as_str()),
                relation.target
            )
            .expect("writing to String cannot fail");
        } else {
            writeln!(
                output,
                "- {} {} --{}--> {}",
                relation.action, relation.source, relation.relation, relation.target
            )
            .expect("writing to String cannot fail");
        }
    }
}

fn render_proof_obligations(output: &mut String, result: &PatchCheckResult, styled: bool) {
    header(output, "Proof Obligations:", styled);
    if result.proof_obligations.is_empty() {
        writeln!(output, "(none)").expect("writing to String cannot fail");
        return;
    }
    for obligation in &result.proof_obligations {
        writeln!(output, "- {}: {}", obligation.object_id, obligation.reason)
            .expect("writing to String cannot fail");
        writeln!(
            output,
            "  evidence: {}",
            obligation.required_evidence.join(", ")
        )
        .expect("writing to String cannot fail");
    }
}

fn render_follow_up(output: &mut String, result: &PatchCheckResult, styled: bool) {
    header(output, "Required Follow-up:", styled);
    if result.required_follow_up.is_empty() {
        writeln!(output, "(none)").expect("writing to String cannot fail");
        return;
    }
    for item in &result.required_follow_up {
        writeln!(output, "- {item}").expect("writing to String cannot fail");
    }
}

fn header(output: &mut String, label: &str, styled: bool) {
    if styled {
        writeln!(output, "{}", faint_label(label)).expect("writing to String cannot fail");
    } else {
        writeln!(output, "{label}").expect("writing to String cannot fail");
    }
}

fn compact_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) => format!("{value:?}"),
        _ => serde_json::to_string(value).expect("JSON value serializes"),
    }
}
