use adoc_core::{
    Diagnostic, DiagnosticCode, GitRef, ObjectDiffEnvelope, PatchApplyInput as CorePatchApplyInput,
    PatchApplyResult, PatchCheckResult, PatchInput, ReviewError, ReviewInput as CoreReviewInput,
    Severity, SnapshotSelector, apply_patch as core_apply_patch, check_patch as core_check_patch,
    diff_objects, load_review_from_git, load_review_with_changed_files_from_git,
    parse_patch_from_path, parse_patch_from_value, patch_apply_refusal, review_with_patch,
};

use super::shared::{
    discover_project_config_if, exit_code_for_diagnostics, resolve_docs_path_with_config,
    resolve_graph_artifact_for_read, resolve_graph_artifact_path_with_config,
};
use super::{
    DiffInput, DiffOutcome, PatchApplyInput, PatchApplyOutcome, PatchApplySource, PatchCheckInput,
    PatchCheckOutcome, ReviewInput, ReviewOutcome, ReviewPatchSource,
};
use crate::{LocalContext, LocalError, PathPolicy};

pub(super) fn diff_with_context<P>(
    context: &LocalContext<P>,
    input: DiffInput,
) -> Result<DiffOutcome, LocalError>
where
    P: PathPolicy,
{
    let project_root = context.config_start().to_path_buf();
    let review_input = CoreReviewInput {
        project_root,
        base: SnapshotSelector::GitRef(GitRef::new(input.base_ref)),
        head: snapshot_selector_from_head_ref(input.head_ref),
    };
    let load =
        load_review_from_git(review_input).map_err(|source| LocalError::Review { source })?;
    let diff = diff_objects(&load.session);
    let envelope = ObjectDiffEnvelope::from_diff(diff, load.diagnostics);
    Ok(DiffOutcome {
        envelope,
        exit_code: 0,
    })
}

pub(super) fn review_with_context<P>(
    context: &LocalContext<P>,
    input: ReviewInput,
) -> Result<ReviewOutcome, LocalError>
where
    P: PathPolicy,
{
    let project_root = context.config_start().to_path_buf();
    // V3.7: parse the patch source before snapshotting so a malformed patch
    // doesn't pay the cost of a worktree checkout. Path-policy resolution
    // happens here at the orchestration boundary; adoc-core never sees a
    // raw filesystem path.
    let patch_document = match input.patch {
        Some(ReviewPatchSource::Path(path)) => {
            let resolved = context.path_policy().resolve_read_path(&path)?;
            Some(
                parse_patch_from_path(&resolved).map_err(|source| LocalError::Review {
                    source: ReviewError::PatchParse { source },
                })?,
            )
        }
        Some(ReviewPatchSource::Inline(value)) => Some(parse_patch_from_value(value).map_err(
            |source| LocalError::Review {
                source: ReviewError::PatchParse { source },
            },
        )?),
        None => None,
    };

    let review_input = CoreReviewInput {
        project_root,
        base: SnapshotSelector::GitRef(GitRef::new(input.base_ref)),
        head: snapshot_selector_from_head_ref(input.head_ref),
    };
    let load = load_review_with_changed_files_from_git(review_input)
        .map_err(|source| LocalError::Review { source })?;
    let envelope = review_with_patch(&load.session, load.diagnostics, patch_document.as_ref());
    Ok(ReviewOutcome {
        envelope,
        exit_code: 0,
    })
}

fn snapshot_selector_from_head_ref(head_ref: Option<String>) -> SnapshotSelector {
    match head_ref {
        Some(spec) => SnapshotSelector::GitRef(GitRef::new(spec)),
        None => SnapshotSelector::Workdir,
    }
}

pub(super) fn patch_check_with_context<P>(
    context: &LocalContext<P>,
    input: PatchCheckInput,
) -> Result<PatchCheckOutcome, LocalError>
where
    P: PathPolicy,
{
    let patch_path = context.path_policy().resolve_read_path(&input.patch_path)?;
    let graph_artifact = resolve_graph_artifact_for_read(context, input.artifact.as_deref())?;
    let result = core_check_patch(PatchInput {
        graph_artifact_path: graph_artifact,
        patch_path,
    });
    let exit_code = patch_exit_code(&result);

    Ok(PatchCheckOutcome { result, exit_code })
}

/// V6.4 — patch apply orchestration. The docs root resolves through the
/// identical chain `check_with_context` uses: `content_hash` payloads embed
/// source paths, so the apply-time recompile reproduces artifact hashes only
/// when the docs root is spelled byte-identically to the one `adoc build`
/// used. A parse failure becomes a refusal envelope (exit 1), not a process
/// error.
pub(super) fn patch_apply_with_context<P>(
    context: &LocalContext<P>,
    input: PatchApplyInput,
) -> Result<PatchApplyOutcome, LocalError>
where
    P: PathPolicy,
{
    let patch = match input.patch {
        PatchApplySource::Path(path) => {
            let resolved = context.path_policy().resolve_read_path(&path)?;
            parse_patch_from_path(&resolved)
        }
        PatchApplySource::Inline(value) => parse_patch_from_value(value),
    };
    let patch = match patch {
        Ok(patch) => patch,
        Err(error) => {
            let result = patch_apply_refusal(error.diagnostics().to_vec(), &input.interface);
            return Ok(PatchApplyOutcome {
                result,
                exit_code: 1,
            });
        }
    };

    let artifact = input
        .artifact
        .as_deref()
        .map(|path| context.path_policy().resolve_read_path(path))
        .transpose()?;
    let config = discover_project_config_if(true, context.config_start())?;
    let graph_artifact = resolve_graph_artifact_path_with_config(artifact, config.as_ref());
    let graph_artifact = context.path_policy().resolve_read_path(&graph_artifact)?;
    let docs_root = resolve_docs_path_with_config(None, config.as_ref())?;
    let docs_root = context.path_policy().resolve_read_path(&docs_root)?;
    let project_root = context
        .path_policy()
        .resolve_write_path(context.config_start())?;

    let result = core_apply_patch(
        CorePatchApplyInput {
            graph_artifact_path: graph_artifact,
            docs_root,
            project_root,
            interface: input.interface,
        },
        patch,
    );
    let exit_code = patch_apply_exit_code(&result);

    Ok(PatchApplyOutcome { result, exit_code })
}

fn patch_exit_code(result: &PatchCheckResult) -> i32 {
    if result.valid {
        0
    } else {
        exit_code_for_diagnostics(&result.diagnostics, patch_diagnostic_exit_code).max(1)
    }
}

/// V6.4 apply exit codes (ADR-0036), deliberately distinct from the check's
/// 0–4 map: `0` applied and post-check clean; `1` refused, nothing written
/// (including a stale `base_hash`); `2` applied but the post-check reports
/// new errors — agents must treat `2` as "stop and surface to a human".
fn patch_apply_exit_code(result: &PatchApplyResult) -> i32 {
    if !result.applied {
        1
    } else if result.post_check.error_count > 0 {
        2
    } else {
        0
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
            | DiagnosticCode::IdDuplicateInArtifact
            | DiagnosticCode::IdInvalid,
            _,
        ) => Some(2),
        (
            DiagnosticCode::PatchInvalidDocument
            | DiagnosticCode::PatchValidationFailed
            | DiagnosticCode::PatchTargetAlreadyExists
            | DiagnosticCode::PatchPlacementInvalid,
            _,
        ) => Some(1),
        (_, Severity::Error) => Some(1),
        _ => None,
    }
}
