use std::fmt::Write as FmtWrite;
use std::io;
use std::path::PathBuf;

use adoc_core::{Diagnostic, DiagnosticCode, PatchCheckResult, PatchInput, Severity, check_patch};

use crate::error::CliError;
use crate::presentation::style::key::cyan_key;
use crate::presentation::style::kv::faint_label;
use crate::presentation::{ResolvedFormat, json as json_presentation};

use super::{
    discover_project_config_if, eprint_diagnostics, exit_code_for_diagnostics, report,
    resolve_graph_artifact_path_with_config,
};

pub(crate) struct PatchCommandInput {
    pub(crate) patch_path: PathBuf,
    pub(crate) artifact: Option<PathBuf>,
}

pub(crate) fn patch(input: PatchCommandInput, resolved: ResolvedFormat) -> i32 {
    let config = match discover_project_config_if(input.artifact.is_none()) {
        Ok(config) => config,
        Err(error) => return report(error),
    };
    let graph_artifact = resolve_graph_artifact_path_with_config(input.artifact, config.as_ref());
    let result = check_patch(PatchInput {
        graph_artifact_path: graph_artifact,
        patch_path: input.patch_path,
    });
    let exit_code = patch_exit_code(&result);

    match resolved {
        ResolvedFormat::Json => write_patch_json(&result, exit_code),
        ResolvedFormat::Plain => write_patch_text(&result, false, exit_code),
        ResolvedFormat::Styled => write_patch_text(&result, true, exit_code),
    }
}

fn write_patch_json(result: &PatchCheckResult, exit_code: i32) -> i32 {
    json_presentation::write_json(result, &mut io::stdout()).map_or_else(
        |source| report(CliError::RetrievalIo { source }),
        |()| exit_code,
    )
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

fn patch_exit_code(result: &PatchCheckResult) -> i32 {
    if result.valid {
        0
    } else {
        exit_code_for_diagnostics(&result.diagnostics, patch_diagnostic_exit_code).max(1)
    }
}

fn patch_diagnostic_exit_code(diagnostic: &Diagnostic) -> Option<i32> {
    match (diagnostic.code, diagnostic.severity) {
        (DiagnosticCode::PatchBaseHashMismatch, _) => Some(4),
        (DiagnosticCode::GraphObjectNotFound, _) => Some(3),
        (
            DiagnosticCode::IoArtifactMissing
            | DiagnosticCode::IoArtifactUnreadable
            | DiagnosticCode::IoArtifactMalformed
            | DiagnosticCode::SchemaUnsupportedVersion
            | DiagnosticCode::IdDuplicateInArtifact,
            _,
        ) => Some(2),
        (
            DiagnosticCode::PatchInvalidDocument
            | DiagnosticCode::PatchValidationFailed
            | DiagnosticCode::PatchTargetAlreadyExists
            | DiagnosticCode::PatchPlacementInvalid
            | DiagnosticCode::IdInvalid,
            _,
        ) => Some(1),
        (_, Severity::Error) => Some(1),
        _ => None,
    }
}
