//! V6.4 TB5 — Expanded Pilot full-loop proof, extended by V6.5.5.
//!
//! Drives the complete agent editing loop against a tempdir **copy** of
//! `examples/expanded-pilot` (the in-repo pilot is never touched):
//! `adoc impacted-by` flags a claim → a patch proposes a body update →
//! `adoc patch --apply` rewrites exactly the body span (byte-exact golden
//! comparison) → post-check clean → `adoc stale` / `adoc contradictions`
//! outputs unchanged → re-applying the same patch refuses
//! (`patch.source_drift` before a rebuild, `patch.base_hash_mismatch` after)
//! and writes nothing.
//!
//! V6.5.5 extends the loop with a second apply against a new-kind object:
//! an `update_fields` patch marks the pilot's overdue task `done`. Beyond
//! its own byte-exact golden, this apply pins an exact warning-count
//! **transition** across the apply step — the documented 6-warning budget
//! is body-edit-invariant but not status-apply-invariant: `task.overdue`
//! flips off when the task leaves `open`, so the post-check drops 6 → 5.

use std::fs;
use std::path::PathBuf;

mod support;

use support::{TestWorkspace, adoc_command, copy_tree, stderr, stdout};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cli crate has workspace parent")
        .parent()
        .expect("workspace has repo root")
        .to_path_buf()
}

const TARGET: &str = "billing.credits.consume";
const EVIDENCE_PATH: &str = "apps/backend/src/features/credits/consume.use-case.ts";
const NEW_BODY: &str = "Credit consumption is settled ledger-first by the use-case implementation; every movement is recorded against the audit ledger.";
const TASK_TARGET: &str = "billing.update-support-runbook";

fn pilot_workspace(name: &str) -> TestWorkspace {
    let workspace = TestWorkspace::new(name);
    copy_tree(
        &repo_root().join("examples/expanded-pilot"),
        &workspace.root,
    );
    workspace
}

fn run_json(workspace: &TestWorkspace, args: &[&str]) -> (Option<i32>, serde_json::Value) {
    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(args)
        .output()
        .expect("adoc runs");
    let value = serde_json::from_str(&stdout(&output)).unwrap_or_else(|error| {
        panic!(
            "stdout is JSON for {args:?}: {error}\nstdout:\n{}\nstderr:\n{}",
            stdout(&output),
            stderr(&output)
        )
    });
    (output.status.code(), value)
}

fn build(workspace: &TestWorkspace) {
    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "--no-embeddings"])
        .output()
        .expect("adoc build runs");
    assert!(
        output.status.success(),
        "pilot build passes\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

fn content_hash(workspace: &TestWorkspace, id: &str) -> String {
    let graph: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(workspace.root.join("dist/docs.graph.json")).expect("graph readable"),
    )
    .expect("graph parses");
    graph["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .find(|node| node["type"] == "knowledge_object" && node["id"] == id)
        .and_then(|node| node["content_hash"].as_str())
        .expect("target content_hash")
        .to_string()
}

fn write_patch(workspace: &TestWorkspace, base_hash: &str) {
    workspace.write(
        "patch.json",
        &serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "replace_body",
            "target": TARGET,
            "base_hash": base_hash,
            "changes": { "body": NEW_BODY },
            "reason": "V6.4 TB5 loop proof: re-verify after consume.use-case.ts changed.",
            "proposer": { "type": "agent", "id": "tb5-loop" }
        })
        .to_string(),
    );
}

#[test]
fn expanded_pilot_apply_loop() {
    let workspace = pilot_workspace("apply-loop");
    build(&workspace);

    // 1. The code change flags the claim with an impact-review obligation.
    let (code, impacted) = run_json(
        &workspace,
        &["impacted-by", EVIDENCE_PATH, "--format", "json"],
    );
    assert_eq!(code, Some(0));
    let impacted_ids: Vec<&str> = impacted["impacted"]
        .as_array()
        .expect("impacted array")
        .iter()
        .map(|record| record["id"].as_str().expect("id"))
        .collect();
    assert!(
        impacted_ids.contains(&TARGET),
        "impacted-by flags the claim: {impacted_ids:?}"
    );
    assert!(
        impacted["proof_obligations"]
            .as_array()
            .expect("obligations")
            .iter()
            .any(|obligation| obligation["object_id"] == TARGET),
        "the claim carries an impact-review obligation"
    );

    // 2. Baselines before the apply.
    let (_, stale_before) = run_json(&workspace, &["stale", "--format", "json"]);
    let (_, contradictions_before) = run_json(&workspace, &["contradictions", "--format", "json"]);

    // 3. Apply the proposed body update.
    write_patch(&workspace, &content_hash(&workspace, TARGET));
    let (code, envelope) = run_json(
        &workspace,
        &["patch", "--apply", "patch.json", "--format", "json"],
    );
    assert_eq!(code, Some(0), "envelope: {envelope}");
    assert_eq!(envelope["schema_version"], "adoc.patch.apply.v0");
    assert_eq!(envelope["applied"], true);
    assert_eq!(envelope["check"]["valid"], true);
    assert_eq!(envelope["post_check"]["ran"], true);
    assert_eq!(envelope["post_check"]["error_count"], 0);
    // The documented pilot diagnostic budget is body-edit-invariant.
    assert_eq!(envelope["post_check"]["warning_count"], 6);
    assert_eq!(envelope["artifacts_stale"], true);
    assert_eq!(envelope["trace"]["interface"], "cli");
    assert_eq!(envelope["trace"]["proposer"]["kind"], "agent");
    let written = envelope["written_files"].as_array().expect("written_files");
    assert_eq!(written.len(), 1);
    assert!(
        written[0]["path"]
            .as_str()
            .expect("path")
            .ends_with("billing/claims.adoc")
    );
    assert_ne!(
        written[0]["before_file_hash"],
        written[0]["after_file_hash"]
    );

    // 4. Byte-exact golden: only the body line differs, everything else in
    //    the tree is byte-identical to the in-repo pilot.
    let golden = fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/v6_4_apply_loop/billing-claims.after.adoc"),
    )
    .expect("golden readable");
    assert_eq!(
        fs::read(workspace.root.join("billing/claims.adoc")).expect("rewritten file readable"),
        golden,
        "rewritten billing/claims.adoc must match the golden byte-for-byte"
    );
    assert_tree_pristine_except(&workspace, &["billing/claims.adoc"]);

    // 5. External post-check: the budget holds (0 errors gate exit 0).
    let check = adoc_command()
        .current_dir(&workspace.root)
        .args(["check"])
        .output()
        .expect("adoc check runs");
    assert!(
        check.status.success(),
        "post-apply check passes\nstderr:\n{}",
        stderr(&check)
    );

    // 6. Re-applying against the now-stale artifact refuses on source drift
    //    and writes nothing.
    let (code, drift) = run_json(
        &workspace,
        &["patch", "--apply", "patch.json", "--format", "json"],
    );
    assert_eq!(code, Some(1));
    assert_eq!(drift["applied"], false);
    assert_eq!(drift["written_files"].as_array().map(Vec::len), Some(0));
    assert!(
        drift["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .any(|diagnostic| diagnostic["code"] == "patch.source_drift"),
        "pre-rebuild re-apply refuses on source drift: {drift}"
    );

    // 7. Rebuild: lifecycle-signal findings are unchanged by the body edit.
    //    Diagnostic *spans* below the edited block legitimately shift by the
    //    body-length delta, so envelopes compare on records and diagnostic
    //    identities, not raw bytes.
    build(&workspace);
    let (_, stale_after) = run_json(&workspace, &["stale", "--format", "json"]);
    let (_, contradictions_after) = run_json(&workspace, &["contradictions", "--format", "json"]);
    assert_eq!(
        stale_after["records"], stale_before["records"],
        "adoc stale records unchanged by the apply"
    );
    assert_eq!(
        diagnostic_identities(&stale_after),
        diagnostic_identities(&stale_before),
        "adoc stale diagnostics unchanged by the apply (modulo spans)"
    );
    assert_eq!(
        contradictions_after["contradictions"], contradictions_before["contradictions"],
        "adoc contradictions findings unchanged by the apply"
    );
    assert_eq!(
        contradictions_after["contradicted_claims"], contradictions_before["contradicted_claims"],
        "contradicted claims unchanged by the apply"
    );

    // 8. Re-applying against the fresh artifact refuses on the stale
    //    base_hash and writes nothing (slice acceptance, ROADMAP-V6 §V6.4).
    let (code, stale_hash) = run_json(
        &workspace,
        &["patch", "--apply", "patch.json", "--format", "json"],
    );
    assert_eq!(code, Some(1));
    assert_eq!(stale_hash["applied"], false);
    assert_eq!(
        stale_hash["written_files"].as_array().map(Vec::len),
        Some(0)
    );
    assert!(
        stale_hash["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .any(|diagnostic| diagnostic["code"] == "patch.base_hash_mismatch"),
        "post-rebuild re-apply refuses on base_hash: {stale_hash}"
    );
    assert_eq!(
        fs::read(workspace.root.join("billing/claims.adoc")).expect("file readable"),
        golden,
        "refusals never double-write"
    );

    // 9. V6.5.5: a second apply against a new-kind object — mark the overdue
    //    task `done` via update_fields. The apply flips `task.overdue` off,
    //    so the post-check pins the exact 6 → 5 warning-count transition
    //    (the pristine in-repo pilot keeps the 6-warning budget).
    workspace.write(
        "task-patch.json",
        &serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "update_fields",
            "target": TASK_TARGET,
            "base_hash": content_hash(&workspace, TASK_TARGET),
            "changes": { "fields": { "status": "done" } },
            "reason": "V6.5.5 loop proof: the support runbook update shipped.",
            "proposer": { "type": "agent", "id": "tb5-loop" }
        })
        .to_string(),
    );
    let (code, task_envelope) = run_json(
        &workspace,
        &["patch", "--apply", "task-patch.json", "--format", "json"],
    );
    assert_eq!(code, Some(0), "envelope: {task_envelope}");
    assert_eq!(task_envelope["applied"], true);
    assert_eq!(task_envelope["post_check"]["error_count"], 0);
    assert_eq!(
        task_envelope["post_check"]["warning_count"], 5,
        "marking the task done must drop the budget from 6 to 5 warnings: {task_envelope}"
    );
    let task_written = task_envelope["written_files"]
        .as_array()
        .expect("written_files");
    assert_eq!(task_written.len(), 1);
    assert!(
        task_written[0]["path"]
            .as_str()
            .expect("path")
            .ends_with("billing/tasks.adoc")
    );

    // 10. Byte-exact golden for the task apply; nothing else moved beyond
    //     the two applied files.
    let task_golden = fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/v6_4_apply_loop/billing-tasks.after.adoc"),
    )
    .expect("task golden readable");
    assert_eq!(
        fs::read(workspace.root.join("billing/tasks.adoc")).expect("rewritten file readable"),
        task_golden,
        "rewritten billing/tasks.adoc must match the golden byte-for-byte"
    );
    assert_tree_pristine_except(&workspace, &["billing/claims.adoc", "billing/tasks.adoc"]);

    // 11. External post-check: exit stays 0 on the reduced budget.
    let check = adoc_command()
        .current_dir(&workspace.root)
        .args(["check"])
        .output()
        .expect("adoc check runs");
    assert!(
        check.status.success(),
        "post-task-apply check passes\nstderr:\n{}",
        stderr(&check)
    );
}

/// Diagnostic identity without source spans: `(code, object_id, message)`.
fn diagnostic_identities(envelope: &serde_json::Value) -> Vec<(String, String, String)> {
    envelope["diagnostics"]
        .as_array()
        .expect("diagnostics array")
        .iter()
        .map(|diagnostic| {
            (
                diagnostic["code"].as_str().unwrap_or_default().to_string(),
                diagnostic["object_id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                diagnostic["message"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
            )
        })
        .collect()
}

/// Every copied pilot source file except the `exempt` paths must stay
/// byte-identical to its `examples/expanded-pilot` original — the "git diff
/// shows only those hunks" guarantee without git.
fn assert_tree_pristine_except(workspace: &TestWorkspace, exempt: &[&str]) {
    let pilot = repo_root().join("examples/expanded-pilot");
    let mut pending = vec![pilot.clone()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).expect("pilot directory readable") {
            let entry = entry.expect("entry readable");
            let path = entry.path();
            if entry.file_type().expect("file type").is_dir() {
                pending.push(path);
                continue;
            }
            let relative = path
                .strip_prefix(&pilot)
                .expect("pilot-relative path")
                .to_string_lossy()
                .into_owned();
            if exempt.contains(&relative.as_str()) {
                continue;
            }
            assert_eq!(
                fs::read(workspace.root.join(&relative)).expect("copied file readable"),
                fs::read(&path).expect("original file readable"),
                "{relative} must stay byte-identical to the in-repo pilot"
            );
        }
    }
}
