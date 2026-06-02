mod support;

use std::process::Command;

use support::{TestWorkspace, adoc_command, fixture_path, stderr, stdout};

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

/// Read or refresh a V3.5 golden Markdown file under
/// `tests/fixtures/review_markdown/`. Set `ADOC_UPDATE_GOLDEN=1` in the env
/// to rewrite the golden from the current run; otherwise compare for byte
/// equality.
fn assert_markdown_matches_golden(relative: &str, actual: &str) {
    let golden = fixture_path(relative);
    if std::env::var_os("ADOC_UPDATE_GOLDEN").is_some() {
        if let Some(parent) = golden.parent() {
            std::fs::create_dir_all(parent).expect("create golden parent dir");
        }
        std::fs::write(&golden, actual).expect("write golden file");
        return;
    }
    let expected = std::fs::read_to_string(&golden).unwrap_or_else(|error| {
        panic!(
            "missing golden file at {}: {error}\nTo bootstrap, re-run with ADOC_UPDATE_GOLDEN=1",
            golden.display()
        )
    });
    assert_eq!(
        actual,
        expected,
        "markdown output diverged from golden at {}\n--- expected ---\n{expected}\n--- actual ---\n{actual}",
        golden.display()
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
