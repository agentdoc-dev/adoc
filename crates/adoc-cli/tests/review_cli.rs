mod support;

use std::process::Command;

use support::{TestWorkspace, adoc_command, assert_markdown_matches_golden, stderr, stdout};

// V3.3 / V3.4 base fixture plus a second verified claim `billing.holds-policy`
// whose head delta exercises every other `FieldChange` variant — status,
// owner, verified_at, evidence add/remove, relation add/remove, and impacts
// add/remove. `billing.legacy-credits` exists only in base (Deleted variant)
// and `billing.holds` only in head (Created variant), so the V3.5 golden also
// covers the `## Created` and `## Deleted` markdown sections. The supersedes
// target `billing.legacy-holds` is a draft claim that lives in both refs so
// reference validation stays clean.
const BASE_BILLING_ADOC: &str = concat!(
    "# Billing @doc(team.billing)\n",
    "\n",
    "::claim billing.refunds\n",
    "status: verified\n",
    "owner: team-billing\n",
    "verified_at: 2026-05-05\n",
    "source: ledger\n",
    "test: integration\n",
    "reviewed_by: team-billing\n",
    "impacts: crates/billing/src/refund.rs\n",
    "--\n",
    "Refunds process within 24 hours.\n",
    "::\n",
    "\n",
    "::claim billing.holds-policy\n",
    "status: verified\n",
    "owner: team-billing\n",
    "verified_at: 2026-05-05\n",
    "source: holds-spec\n",
    "test: integration\n",
    "supersedes: billing.legacy-holds\n",
    "impacts: crates/billing/src/holds.rs\n",
    "--\n",
    "Holds expire after 7 days.\n",
    "::\n",
    "\n",
    "::claim billing.legacy-holds\n",
    "status: draft\n",
    "--\n",
    "Legacy hold semantics.\n",
    "::\n",
    "\n",
    "::claim billing.legacy-credits\n",
    "status: draft\n",
    "--\n",
    "Legacy credit semantics.\n",
    "::\n",
);

const HEAD_BILLING_ADOC: &str = concat!(
    "# Billing @doc(team.billing)\n",
    "\n",
    "::claim billing.refunds\n",
    "status: verified\n",
    "owner: team-billing\n",
    "verified_at: 2026-05-05\n",
    "source: ledger\n",
    "test: integration\n",
    "reviewed_by: team-billing\n",
    "impacts: crates/billing/src/refund.rs\n",
    "--\n",
    "Refunds process within 12 hours.\n",
    "::\n",
    "\n",
    "::claim billing.holds-policy\n",
    "status: needs_review\n",
    "owner: team-payments\n",
    "verified_at: 2026-05-10\n",
    "source: holds-spec\n",
    "reviewed_by: team-payments\n",
    "depends_on: billing.refunds\n",
    "impacts: crates/billing/src/holds-v2.rs\n",
    "--\n",
    "Holds expire after 7 days.\n",
    "::\n",
    "\n",
    "::claim billing.legacy-holds\n",
    "status: draft\n",
    "--\n",
    "Legacy hold semantics.\n",
    "::\n",
    "\n",
    "::claim billing.holds\n",
    "status: draft\n",
    "--\n",
    "Holds extend refund window automatically.\n",
    "::\n",
);

const BASE_REFUND_SRC: &str = "// initial stub\n";
const HEAD_REFUND_SRC: &str = "// updated implementation\n";

fn run_git(workspace: &TestWorkspace, args: &[&str]) {
    let mut command = Command::new("git");
    command.args(args).current_dir(&workspace.root);
    for var in [
        "GIT_DIR",
        "GIT_INDEX_FILE",
        "GIT_WORK_TREE",
        "GIT_NAMESPACE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_COMMON_DIR",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_PREFIX",
    ] {
        command.env_remove(var);
    }
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("git {args:?} should spawn: {error}"));
    assert!(
        output.status.success(),
        "git {args:?} should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Build a 2-commit billing-pilot fixture for V3.3 acceptance:
/// - base on `main`: `docs/billing.adoc` declares a verified
///   `billing.refunds` claim with `impacts: crates/billing/src/refund.rs`
///   and an evidence stub at that file.
/// - head on `feature`: edits `crates/billing/src/refund.rs` and updates the
///   claim's body so its `content_hash` flips.
fn build_billing_pilot_with_impacts(name: &str) -> TestWorkspace {
    let workspace = TestWorkspace::new(name);
    run_git(&workspace, &["init", "--initial-branch=main"]);
    run_git(&workspace, &["config", "user.email", "test@adoc.dev"]);
    run_git(&workspace, &["config", "user.name", "adoc tests"]);
    run_git(&workspace, &["config", "commit.gpgsign", "false"]);

    workspace.write("docs/billing.adoc", BASE_BILLING_ADOC);
    workspace.write("crates/billing/src/refund.rs", BASE_REFUND_SRC);
    run_git(&workspace, &["add", "-A"]);
    run_git(&workspace, &["commit", "-m", "base"]);

    run_git(&workspace, &["checkout", "-b", "feature"]);
    workspace.write("docs/billing.adoc", HEAD_BILLING_ADOC);
    workspace.write("crates/billing/src/refund.rs", HEAD_REFUND_SRC);
    run_git(&workspace, &["add", "-A"]);
    run_git(&workspace, &["commit", "-m", "head"]);

    workspace
}

#[test]
fn review_main_json_envelope_flags_billing_refunds() {
    let workspace = build_billing_pilot_with_impacts("review-json");

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["review", "main", "--format", "json"])
        .output()
        .expect("adoc review runs");

    assert!(
        output.status.success(),
        "expected adoc review to exit zero\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );

    let value: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("adoc review --format json must emit a JSON envelope on stdout");

    assert_eq!(value["schema_version"], "adoc.review.v0");
    assert_eq!(value["diff"]["schema_version"], "adoc.diff.v0");

    let impact = value["impact"].as_array().expect("impact array");
    assert_eq!(
        impact.len(),
        1,
        "expected exactly one impacted object, got: {impact:#?}"
    );
    assert_eq!(impact[0]["id"], "billing.refunds");
    assert_eq!(impact[0]["paths"][0], "crates/billing/src/refund.rs");

    let reviewers = value["required_reviewers"]
        .as_array()
        .expect("required_reviewers array");
    assert_eq!(
        reviewers.len(),
        1,
        "expected one required reviewer, got: {reviewers:#?}"
    );
    assert_eq!(reviewers[0]["owner"], "team-billing");
    let object_ids = reviewers[0]["object_ids"]
        .as_array()
        .expect("object_ids array");
    assert!(
        object_ids.iter().any(|id| id == "billing.refunds"),
        "team-billing must own billing.refunds; got: {object_ids:#?}"
    );
}

#[test]
fn review_main_plain_lists_impact_and_reviewer_sections() {
    let workspace = build_billing_pilot_with_impacts("review-plain");

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["review", "main", "--format", "plain"])
        .output()
        .expect("adoc review runs");

    assert!(
        output.status.success(),
        "expected adoc review to exit zero\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let stdout = stdout(&output);
    assert!(stdout.contains("Impact:"));
    assert!(stdout.contains("billing.refunds (crates/billing/src/refund.rs)"));
    assert!(stdout.contains("Required reviewers:"));
    assert!(stdout.contains("team-billing: billing.refunds"));
}

#[test]
fn review_main_json_envelope_includes_proof_obligations() {
    let workspace = build_billing_pilot_with_impacts("review-obligations-json");

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["review", "main", "--format", "json"])
        .output()
        .expect("adoc review runs");

    assert!(
        output.status.success(),
        "expected adoc review to exit zero\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );

    let value: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("adoc review --format json must emit a JSON envelope on stdout");

    let obligations = value["proof_obligations"]
        .as_array()
        .expect("proof_obligations array");
    // V3.5 fixture extension: billing.holds-policy transitions verified →
    // needs_review (one stale-verified obligation) on top of billing.refunds'
    // body change (re-verify body) and the impact-review obligation.
    assert_eq!(
        obligations.len(),
        3,
        "expected stale-verified + re-verify-body + impact-review, got: {obligations:#?}"
    );

    // Output sorts by (object_id, reason). billing.holds-policy precedes
    // billing.refunds alphabetically; within billing.refunds, "re-verify body"
    // precedes "review impacted claim".
    assert_eq!(obligations[0]["object_id"], "billing.holds-policy");
    assert_eq!(obligations[0]["reason"], "stale verified claim");
    let stale_evidence: Vec<&str> = obligations[0]["required_evidence"]
        .as_array()
        .expect("required_evidence array")
        .iter()
        .map(|v| v.as_str().expect("evidence is string"))
        .collect();
    assert!(
        stale_evidence.is_empty(),
        "stale-verified obligation should carry no required evidence; got: {stale_evidence:?}"
    );

    assert_eq!(obligations[1]["object_id"], "billing.refunds");
    assert_eq!(obligations[1]["reason"], "re-verify body");
    let re_verify_evidence: Vec<&str> = obligations[1]["required_evidence"]
        .as_array()
        .expect("required_evidence array")
        .iter()
        .map(|v| v.as_str().expect("evidence is string"))
        .collect();
    assert_eq!(
        re_verify_evidence,
        // V5.8: EvidenceKind strings.
        vec!["source_code", "test", "human_review"],
        "V3.4 acceptance: body change on verified claim with three evidence fields"
    );

    assert_eq!(obligations[2]["object_id"], "billing.refunds");
    assert_eq!(obligations[2]["reason"], "review impacted claim");
    let impact_evidence: Vec<&str> = obligations[2]["required_evidence"]
        .as_array()
        .expect("required_evidence array")
        .iter()
        .map(|v| v.as_str().expect("evidence is string"))
        .collect();
    // V5.8: source evidence is "source_code".
    assert_eq!(impact_evidence, vec!["source_code"]);
}

/// Base page: two claims and an unresolved contradiction referencing both.
const BASE_CONTRADICTION_ADOC: &str = concat!(
    "# Auth Contradictions @doc(auth.contradictions)\n",
    "\n",
    "::claim auth.memory-only\n",
    "status: plain\n",
    "--\n",
    "Session tokens must be stored in memory only.\n",
    "::\n",
    "\n",
    "::claim auth.local-storage-ok\n",
    "status: plain\n",
    "--\n",
    "Session tokens may be stored in localStorage for convenience.\n",
    "::\n",
    "\n",
    "::contradiction auth.session.conflict\n",
    "severity: high\n",
    "status: unresolved\n",
    "owner: platform-auth\n",
    "claims: [auth.local-storage-ok, auth.memory-only]\n",
    "--\n",
    "The two claims disagree about session token storage.\n",
    "::\n",
);

/// Head page: same objects; only the contradiction's body is edited.
const HEAD_CONTRADICTION_ADOC: &str = concat!(
    "# Auth Contradictions @doc(auth.contradictions)\n",
    "\n",
    "::claim auth.memory-only\n",
    "status: plain\n",
    "--\n",
    "Session tokens must be stored in memory only.\n",
    "::\n",
    "\n",
    "::claim auth.local-storage-ok\n",
    "status: plain\n",
    "--\n",
    "Session tokens may be stored in localStorage for convenience.\n",
    "::\n",
    "\n",
    "::contradiction auth.session.conflict\n",
    "severity: high\n",
    "status: unresolved\n",
    "owner: platform-auth\n",
    "claims: [auth.local-storage-ok, auth.memory-only]\n",
    "--\n",
    "The two claims still disagree about session token storage; scope widened.\n",
    "::\n",
);

/// V5 audit remediation: any field change on an unresolved contradiction
/// emits one owner re-assert obligation (once per changed object).
#[test]
fn review_main_json_emits_owner_reassert_for_changed_unresolved_contradiction() {
    let workspace = TestWorkspace::new("review-contradiction-reassert");
    run_git(&workspace, &["init", "--initial-branch=main"]);
    run_git(&workspace, &["config", "user.email", "test@adoc.dev"]);
    run_git(&workspace, &["config", "user.name", "adoc tests"]);
    run_git(&workspace, &["config", "commit.gpgsign", "false"]);

    workspace.write("docs/contradictions.adoc", BASE_CONTRADICTION_ADOC);
    run_git(&workspace, &["add", "-A"]);
    run_git(&workspace, &["commit", "-m", "base"]);

    run_git(&workspace, &["checkout", "-b", "feature"]);
    workspace.write("docs/contradictions.adoc", HEAD_CONTRADICTION_ADOC);
    run_git(&workspace, &["add", "-A"]);
    run_git(&workspace, &["commit", "-m", "head"]);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["review", "main", "--format", "json"])
        .output()
        .expect("adoc review runs");

    assert!(
        output.status.success(),
        "expected adoc review to exit zero\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );

    let value: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("adoc review --format json must emit a JSON envelope on stdout");

    let obligations = value["proof_obligations"]
        .as_array()
        .expect("proof_obligations array");
    let reassert: Vec<&serde_json::Value> = obligations
        .iter()
        .filter(|o| o["reason"] == "owner re-assert (unresolved contradiction changed)")
        .collect();
    assert_eq!(
        reassert.len(),
        1,
        "expected exactly one owner re-assert obligation, got: {obligations:#?}"
    );
    assert_eq!(reassert[0]["object_id"], "auth.session.conflict");
    let required: Vec<&str> = reassert[0]["required_evidence"]
        .as_array()
        .expect("required_evidence array")
        .iter()
        .map(|v| v.as_str().expect("evidence is string"))
        .collect();
    assert_eq!(required, vec!["owner"]);
}

#[test]
fn review_main_plain_lists_proof_obligations_section() {
    let workspace = build_billing_pilot_with_impacts("review-obligations-plain");

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["review", "main", "--format", "plain"])
        .output()
        .expect("adoc review runs");

    assert!(
        output.status.success(),
        "expected adoc review to exit zero\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let stdout = stdout(&output);
    assert!(
        stdout.contains("Proof obligations:"),
        "expected section header in plain output\n{stdout}"
    );
    assert!(
        stdout.contains("billing.refunds: re-verify body"),
        "expected re-verify body obligation line\n{stdout}"
    );
    assert!(
        stdout.contains("billing.refunds: review impacted claim"),
        "expected impact-review obligation line\n{stdout}"
    );
}

#[test]
fn diff_main_markdown_matches_golden() {
    let workspace = build_billing_pilot_with_impacts("diff-markdown");

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["diff", "main", "--format", "markdown"])
        .output()
        .expect("adoc diff runs");

    assert!(
        output.status.success(),
        "expected adoc diff --format markdown to exit zero\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );

    assert_markdown_matches_golden("review_markdown/diff.md", &stdout(&output));
}

#[test]
fn review_main_markdown_matches_golden() {
    let workspace = build_billing_pilot_with_impacts("review-markdown");

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["review", "main", "--format", "markdown"])
        .output()
        .expect("adoc review runs");

    assert!(
        output.status.success(),
        "expected adoc review --format markdown to exit zero\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );

    assert_markdown_matches_golden("review_markdown/review.md", &stdout(&output));
}

#[test]
fn review_unresolvable_ref_exits_nonzero_with_actionable_stderr() {
    let workspace = build_billing_pilot_with_impacts("review-bad-ref");

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["review", "definitely-not-a-real-ref"])
        .output()
        .expect("adoc review runs");

    assert!(
        !output.status.success(),
        "expected unresolvable ref to exit non-zero\nstdout:\n{}",
        stdout(&output)
    );
    let stderr = stderr(&output);
    assert!(stderr.contains("definitely-not-a-real-ref"));
}

// ----- V3.7 patch composition -----

/// Helper for V3.7 tests: run `adoc review main --format json` against the
/// given fixture, parse the envelope, and return the head-side content_hash
/// for the named changed Knowledge Object. Allows tests to construct a patch
/// JSON file whose `base_hash` validates cleanly against the head graph.
fn head_content_hash_for(workspace: &TestWorkspace, object_id: &str) -> String {
    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["review", "main", "--format", "json"])
        .output()
        .expect("adoc review runs");
    assert!(
        output.status.success(),
        "expected adoc review (no patch) to exit zero\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let envelope: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("envelope parses");
    envelope["diff"]["changed"]
        .as_array()
        .expect("changed array")
        .iter()
        .find(|entry| entry["id"] == object_id)
        .unwrap_or_else(|| panic!("{object_id} not in diff.changed; envelope:\n{envelope:#}"))
        ["head"]["content_hash"]
        .as_str()
        .expect("content_hash string")
        .to_string()
}

#[test]
fn review_with_valid_patch_embeds_patch_check_and_unions_obligations() {
    let workspace = build_billing_pilot_with_impacts("review-patch-valid");
    let base_hash = head_content_hash_for(&workspace, "billing.refunds");

    let patch_json = format!(
        concat!(
            "{{\n",
            "  \"schema_version\": \"adoc.patch.v0\",\n",
            "  \"op\": \"replace_body\",\n",
            "  \"target\": \"billing.refunds\",\n",
            "  \"base_hash\": \"{}\",\n",
            "  \"changes\": {{ \"body\": \"Patched body.\" }},\n",
            "  \"reason\": \"V3.7 fixture\"\n",
            "}}\n",
        ),
        base_hash,
    );
    workspace.write("patch.json", &patch_json);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args([
            "review",
            "main",
            "--patch",
            "patch.json",
            "--format",
            "json",
        ])
        .output()
        .expect("adoc review --patch runs");

    assert!(
        output.status.success(),
        "expected adoc review --patch to exit zero\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );

    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("envelope parses");

    let patch_check = &value["patch_check"];
    assert!(
        patch_check.is_object(),
        "patch_check must be present when --patch supplied; got: {value:#}"
    );
    assert_eq!(patch_check["schema_version"], "adoc.patch.check.v0");
    assert_eq!(patch_check["valid"], serde_json::json!(true));
    assert_eq!(patch_check["target"], "billing.refunds");

    // V3.7 acceptance: top-level proof_obligations is the union of
    // diff-driven (V3.4) and patch-driven (V2) obligations. The V3.4 trio
    // (stale-verified + re-verify-body + impact-review) must still appear;
    // additional patch-side obligations may be merged in.
    let obligations = value["proof_obligations"]
        .as_array()
        .expect("proof_obligations array");
    let pairs: Vec<(String, String)> = obligations
        .iter()
        .map(|o| {
            (
                o["object_id"].as_str().expect("object_id").to_string(),
                o["reason"].as_str().expect("reason").to_string(),
            )
        })
        .collect();
    for expected in &[
        (
            "billing.holds-policy".to_string(),
            "stale verified claim".to_string(),
        ),
        ("billing.refunds".to_string(), "re-verify body".to_string()),
        (
            "billing.refunds".to_string(),
            "review impacted claim".to_string(),
        ),
    ] {
        assert!(
            pairs.contains(expected),
            "missing diff-driven obligation {expected:?} after union; got: {pairs:?}"
        );
    }

    // No (object_id, reason) appears twice in the unioned list.
    let mut seen = std::collections::BTreeSet::new();
    for pair in &pairs {
        assert!(
            seen.insert(pair.clone()),
            "duplicate (object_id, reason) {pair:?} in unioned obligations: {pairs:?}"
        );
    }
}

#[test]
fn review_with_stale_patch_base_hash_surfaces_in_envelope_diagnostics() {
    let workspace = build_billing_pilot_with_impacts("review-patch-stale");

    let patch_json = concat!(
        "{\n",
        "  \"schema_version\": \"adoc.patch.v0\",\n",
        "  \"op\": \"replace_body\",\n",
        "  \"target\": \"billing.refunds\",\n",
        "  \"base_hash\": \"sha256:wrong\",\n",
        "  \"changes\": { \"body\": \"Patched body.\" },\n",
        "  \"reason\": \"V3.7 stale fixture\"\n",
        "}\n",
    );
    workspace.write("stale-patch.json", patch_json);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args([
            "review",
            "main",
            "--patch",
            "stale-patch.json",
            "--format",
            "json",
        ])
        .output()
        .expect("adoc review --patch runs");

    // adoc review exits 0 even when the embedded patch_check is invalid —
    // matches the V2 patch-check convention that data-level rejection rides
    // inside the envelope, not the exit code (which is reserved for snapshot
    // / compile / parse-time failures).
    assert!(
        output.status.success(),
        "expected adoc review to exit zero even with stale patch base_hash\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );

    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("envelope parses");
    let patch_check = &value["patch_check"];
    assert!(patch_check.is_object(), "patch_check present: {value:#}");
    assert_eq!(patch_check["valid"], serde_json::json!(false));
    let diagnostics = patch_check["diagnostics"]
        .as_array()
        .expect("diagnostics array");
    assert!(
        diagnostics
            .iter()
            .any(|d| d["code"] == "patch.base_hash_mismatch"),
        "expected patch.base_hash_mismatch diagnostic; got: {diagnostics:#?}"
    );
}

#[test]
fn review_without_patch_omits_patch_check_field_from_envelope() {
    let workspace = build_billing_pilot_with_impacts("review-no-patch-omits-field");

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["review", "main", "--format", "json"])
        .output()
        .expect("adoc review runs");
    assert!(output.status.success());

    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("envelope parses");
    assert!(
        value.get("patch_check").is_none(),
        "patch_check must be omitted (not null) when --patch absent; got: {value:#}"
    );
}

#[test]
fn review_with_missing_patch_file_exits_nonzero_with_actionable_stderr() {
    let workspace = build_billing_pilot_with_impacts("review-patch-missing-file");

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args([
            "review",
            "main",
            "--patch",
            "no-such-patch.json",
            "--format",
            "json",
        ])
        .output()
        .expect("adoc review runs");

    assert!(
        !output.status.success(),
        "expected missing patch file to exit non-zero\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let stderr = stderr(&output);
    assert!(
        stderr.contains("review")
            && (stderr.contains("patch") || stderr.contains("no-such-patch.json")),
        "expected stderr to mention review/patch context; got:\n{stderr}"
    );
}

#[test]
fn check_rejects_impacts_parent_segment_path() {
    let workspace = TestWorkspace::new("review-impacts-parent-segment");
    workspace.write(
        "agentdoc.config.yaml",
        "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dir: dist\nembeddings:\n  provider: deterministic\n",
    );
    workspace.write(
        "docs/billing.adoc",
        concat!(
            "# Billing @doc(team.billing)\n",
            "\n",
            "::claim billing.refunds\n",
            "status: draft\n",
            "impacts: ..\n",
            "--\n",
            "Refunds.\n",
            "::\n",
        ),
    );

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected adoc check to exit non-zero for `impacts: ..`\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let combined = format!("{}\n{}", stdout(&output), stderr(&output));
    assert!(
        combined.contains("schema.impacts_invalid_path"),
        "expected schema.impacts_invalid_path in output; got:\n{combined}"
    );
}

// --- V6.3 `adoc impacted-by` acceptance over the billing-pilot impact fixture ---

/// Build the workspace's graph artifact at `dist/docs.graph.json` so
/// `impacted-by` (a pure artifact read) has something to query.
fn build_graph_artifact(workspace: &TestWorkspace) {
    let build = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "docs", "--out", "dist", "--no-embeddings"])
        .output()
        .expect("adoc build runs");
    assert!(
        build.status.success(),
        "impacted-by prerequisite build must succeed\nstdout:\n{}\nstderr:\n{}",
        stdout(&build),
        stderr(&build)
    );
}

/// Roadmap V6.3 acceptance #1: the explicit-path shape reports the verified
/// claim declaring that path under `reasons[].kind: "impacts_path"` with one
/// impact-review obligation.
#[test]
fn impacted_by_explicit_path_reports_verified_claim_with_impact_obligation() {
    let workspace = build_billing_pilot_with_impacts("impacted-by-path");
    build_graph_artifact(&workspace);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args([
            "impacted-by",
            "crates/billing/src/refund.rs",
            "--format",
            "json",
        ])
        .output()
        .expect("adoc impacted-by runs");

    assert!(
        output.status.success(),
        "impacted-by is a query and must exit 0 with findings\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("impacted-by stdout is JSON");

    assert_eq!(value["schema_version"], "adoc.impacted.v0");
    assert_eq!(
        value["changed_paths"],
        serde_json::json!(["crates/billing/src/refund.rs"])
    );

    let impacted = value["impacted"].as_array().expect("impacted array");
    assert_eq!(
        impacted.len(),
        1,
        "expected exactly one impacted object: {impacted:#?}"
    );
    assert_eq!(impacted[0]["id"], "billing.refunds");
    assert_eq!(impacted[0]["kind"], "claim");
    assert_eq!(impacted[0]["status"], "verified");
    assert_eq!(impacted[0]["owner"], "team-billing");

    let reasons = impacted[0]["reasons"].as_array().expect("reasons array");
    assert_eq!(reasons.len(), 1, "one reason expected: {reasons:#?}");
    assert_eq!(reasons[0]["kind"], "impacts_path");
    assert_eq!(reasons[0]["matched_path"], "crates/billing/src/refund.rs");
    assert!(
        reasons[0].get("via_source_object").is_none(),
        "impacts_path reasons never carry via_source_object"
    );

    let obligations = value["proof_obligations"]
        .as_array()
        .expect("proof_obligations array");
    assert_eq!(obligations.len(), 1, "one obligation: {obligations:#?}");
    assert_eq!(obligations[0]["object_id"], "billing.refunds");
    assert_eq!(
        obligations[0]["required_evidence"],
        serde_json::json!(["source_code"])
    );
}

/// Roadmap V6.3 acceptance #2: `adoc impacted-by --ref main` over the V3
/// two-commit fixture produces the same impacted set as `adoc review main`'s
/// `impact[]`.
#[test]
fn impacted_by_ref_main_matches_review_impact_set() {
    let workspace = build_billing_pilot_with_impacts("impacted-by-ref");
    build_graph_artifact(&workspace);

    let impacted_output = adoc_command()
        .current_dir(&workspace.root)
        .args(["impacted-by", "--ref", "main", "--format", "json"])
        .output()
        .expect("adoc impacted-by runs");
    assert!(
        impacted_output.status.success(),
        "impacted-by --ref main must exit 0\nstdout:\n{}\nstderr:\n{}",
        stdout(&impacted_output),
        stderr(&impacted_output)
    );
    let impacted_value: serde_json::Value =
        serde_json::from_slice(&impacted_output.stdout).expect("impacted-by stdout is JSON");

    let review_output = adoc_command()
        .current_dir(&workspace.root)
        .args(["review", "main", "--format", "json"])
        .output()
        .expect("adoc review runs");
    assert!(review_output.status.success());
    let review_value: serde_json::Value =
        serde_json::from_slice(&review_output.stdout).expect("review stdout is JSON");

    // The changed set must include both committed files (base...workdir).
    let changed: Vec<&str> = impacted_value["changed_paths"]
        .as_array()
        .expect("changed_paths array")
        .iter()
        .map(|p| p.as_str().expect("path string"))
        .collect();
    assert!(changed.contains(&"crates/billing/src/refund.rs"));
    assert!(changed.contains(&"docs/billing.adoc"));

    // Parity: (id, impacts_path matched paths) pairs equal review's impact[].
    let impacted_pairs: Vec<(String, Vec<String>)> = impacted_value["impacted"]
        .as_array()
        .expect("impacted array")
        .iter()
        .map(|record| {
            let id = record["id"].as_str().expect("id").to_string();
            let paths: Vec<String> = record["reasons"]
                .as_array()
                .expect("reasons")
                .iter()
                .filter(|reason| reason["kind"] == "impacts_path")
                .map(|reason| {
                    reason["matched_path"]
                        .as_str()
                        .expect("matched_path")
                        .to_string()
                })
                .collect();
            (id, paths)
        })
        .collect();
    let review_pairs: Vec<(String, Vec<String>)> = review_value["impact"]
        .as_array()
        .expect("impact array")
        .iter()
        .map(|entry| {
            let id = entry["id"].as_str().expect("id").to_string();
            let paths: Vec<String> = entry["paths"]
                .as_array()
                .expect("paths")
                .iter()
                .map(|p| p.as_str().expect("path").to_string())
                .collect();
            (id, paths)
        })
        .collect();
    assert_eq!(
        impacted_pairs, review_pairs,
        "impacted-by --ref main must match review main's impact[]"
    );
    assert!(
        !impacted_pairs.is_empty(),
        "parity must be over a non-empty impact set"
    );
}

/// An unresolvable `--ref` is a user-input error: exit 1, envelope still
/// emitted with the fix-oriented `impacted.ref_unresolvable` diagnostic.
#[test]
fn impacted_by_unknown_ref_exits_one_with_ref_unresolvable_diagnostic() {
    let workspace = build_billing_pilot_with_impacts("impacted-by-bad-ref");
    build_graph_artifact(&workspace);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["impacted-by", "--ref", "does-not-exist", "--format", "json"])
        .output()
        .expect("adoc impacted-by runs");

    assert_eq!(
        output.status.code(),
        Some(1),
        "unresolvable ref is a user-input error\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("JSON envelope still emitted on refusal");
    assert_eq!(value["schema_version"], "adoc.impacted.v0");
    assert_eq!(value["impacted"], serde_json::json!([]));
    assert_eq!(value["diagnostics"][0]["code"], "impacted.ref_unresolvable");
}

/// `--format markdown` on a refusal still writes a visible error block to
/// stdout: a PR-comment bot pasting the output must show *something*, not an
/// empty comment (JSON gets the envelope; plain/styled get stderr only).
#[test]
fn impacted_by_markdown_error_renders_blockquote_on_stdout() {
    let workspace = build_billing_pilot_with_impacts("impacted-by-md-error");
    build_graph_artifact(&workspace);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args([
            "impacted-by",
            "--ref",
            "does-not-exist",
            "--format",
            "markdown",
        ])
        .output()
        .expect("adoc impacted-by runs");

    assert_eq!(
        output.status.code(),
        Some(1),
        "unresolvable ref stays a user-input error\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let stdout = stdout(&output);
    assert!(
        stdout.contains("> ⚠️ adoc impacted-by failed: `impacted.ref_unresolvable`"),
        "markdown error block must name the diagnostic code on stdout; got:\n{stdout}"
    );
}

/// An invalid positional path is a user-input error: exit 1 with
/// `impacted.invalid_path`.
#[test]
fn impacted_by_invalid_path_argument_exits_one_with_invalid_path_diagnostic() {
    let workspace = build_billing_pilot_with_impacts("impacted-by-bad-path");
    build_graph_artifact(&workspace);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["impacted-by", "/absolute/path.rs", "--format", "json"])
        .output()
        .expect("adoc impacted-by runs");

    assert_eq!(
        output.status.code(),
        Some(1),
        "invalid path is a user-input error\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("JSON envelope still emitted on refusal");
    assert_eq!(value["diagnostics"][0]["code"], "impacted.invalid_path");
}

/// The two input shapes are mutually exclusive and one is required — both
/// violations are clap parse errors (exit 1, no envelope).
#[test]
fn impacted_by_input_shapes_are_mutually_exclusive_and_one_is_required() {
    let workspace = build_billing_pilot_with_impacts("impacted-by-xor");

    let both = adoc_command()
        .current_dir(&workspace.root)
        .args(["impacted-by", "--ref", "main", "extra/path.rs"])
        .output()
        .expect("adoc impacted-by runs");
    assert_eq!(
        both.status.code(),
        Some(1),
        "paths and --ref together must be a parse error\nstderr:\n{}",
        stderr(&both)
    );

    let neither = adoc_command()
        .current_dir(&workspace.root)
        .args(["impacted-by"])
        .output()
        .expect("adoc impacted-by runs");
    assert_eq!(
        neither.status.code(),
        Some(1),
        "neither paths nor --ref must be a parse error\nstderr:\n{}",
        stderr(&neither)
    );
}

/// `--format markdown` renders the V8.3.4 embeddable PR-comment shape: a
/// count-first changed-paths line with the list collapsed in `<details>`,
/// plus the proof-obligations task list under a bold label.
#[test]
fn impacted_by_markdown_renders_header_and_obligation_task_list() {
    let workspace = build_billing_pilot_with_impacts("impacted-by-markdown");
    build_graph_artifact(&workspace);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args([
            "impacted-by",
            "crates/billing/src/refund.rs",
            "--format",
            "markdown",
        ])
        .output()
        .expect("adoc impacted-by runs");

    assert!(
        output.status.success(),
        "markdown is a supported impacted-by format\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let stdout = stdout(&output);
    assert!(
        stdout.contains("**Impacted by** 1 changed paths"),
        "markdown must open with the count-first changed-paths line; got:\n{stdout}"
    );
    assert!(
        stdout.contains("<details>") && stdout.contains("<summary>Changed paths</summary>"),
        "changed paths must collapse into a details block; got:\n{stdout}"
    );
    assert!(
        stdout.contains("`billing.refunds`"),
        "impacted object id must be code-quoted; got:\n{stdout}"
    );
    assert!(
        stdout.contains("`crates/billing/src/refund.rs`"),
        "matched path must be code-quoted; got:\n{stdout}"
    );
    assert!(
        stdout.contains("**Proof obligations**"),
        "obligations must sit under a bold label, not a heading; got:\n{stdout}"
    );
    assert!(
        stdout.contains("- [ ] `billing.refunds`"),
        "proof obligations must render as a GitHub task list; got:\n{stdout}"
    );
}
