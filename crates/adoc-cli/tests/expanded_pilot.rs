//! V5.10 end-to-end test for the V5 Expanded Pilot.
//!
//! Validates `examples/expanded-pilot/` against the full V5 + V5.10 acceptance
//! contract from `docs/V5-DESIGN.md`. The pilot exercises every new V5 kind
//! (constraint, procedure, example, policy, agent_instruction, contradiction,
//! source) plus the V5.8 typed evidence model and all four V5.10 lifecycle
//! signals, across auth / billing / security domains.
//!
//! Diagnostic budget (documented in `docs/expanded-pilot.md`): 0 errors,
//! 5 warnings:
//!
//! | Code                                  | Count | Object                                     |
//! | :------------------------------------ | :---: | :----------------------------------------- |
//! | `lifecycle.expired`                   |   2   | `billing.credits.legacy-export`, `security.audit.retention` |
//! | `schema.policy_review_overdue`        |   1   | `security.production-db-access`            |
//! | `claim.evidence_quality_low`          |   1   | `security.csrf-advisory`                   |
//! | `schema.claim_contradicted_by_unresolved` | 1 | `auth.session.csrf-protection`             |
//!
//! All warnings are driven by fixed past dates / wide-margin fixtures so the
//! budget is clock-stable on any realistic future run date.

use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

mod support;

use support::TestWorkspace;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cli crate has workspace parent")
        .parent()
        .expect("workspace has repo root")
        .to_path_buf()
}

const PILOT_PATH: &str = "examples/expanded-pilot/";

/// V5.10 acceptance: `adoc check` over the full pilot exits 0 with the exact
/// diagnostic budget published in `docs/expanded-pilot.md` — 0 errors and
/// exactly 5 warnings (2 `lifecycle.expired`, 1 `schema.policy_review_overdue`,
/// 1 `claim.evidence_quality_low`, 1 `schema.claim_contradicted_by_unresolved`).
#[test]
fn expanded_pilot_check_emits_documented_diagnostic_budget() {
    let repo_root = repo_root();
    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&repo_root)
        .args(["check", PILOT_PATH])
        .output()
        .expect("adoc check runs");

    assert!(
        output.status.success(),
        "pilot must check with exit code 0 (lifecycle warnings do not fail check)\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let expired_count = stdout.matches("warning[lifecycle.expired]").count();
    let policy_review_overdue_count = stdout
        .matches("warning[schema.policy_review_overdue]")
        .count();
    let evidence_quality_low_count = stdout
        .matches("warning[claim.evidence_quality_low]")
        .count();
    let claim_contradicted_count = stdout
        .matches("warning[schema.claim_contradicted_by_unresolved]")
        .count();
    let error_count = stdout.matches("error[").count();

    assert_eq!(
        expired_count, 2,
        "expected two lifecycle.expired warnings (billing.credits.legacy-export, security.audit.retention)\nstdout:\n{stdout}"
    );
    assert_eq!(
        policy_review_overdue_count, 1,
        "expected one schema.policy_review_overdue warning (security.production-db-access)\nstdout:\n{stdout}"
    );
    assert_eq!(
        evidence_quality_low_count, 1,
        "expected one claim.evidence_quality_low warning (security.csrf-advisory)\nstdout:\n{stdout}"
    );
    assert_eq!(
        claim_contradicted_count, 1,
        "expected one schema.claim_contradicted_by_unresolved warning (auth.session.csrf-protection)\nstdout:\n{stdout}"
    );
    assert_eq!(
        error_count, 0,
        "pilot must produce zero errors (every kind is strict-valid)\nstdout:\n{stdout}"
    );
    assert!(
        stdout.contains("0 errors, 5 warnings"),
        "expected aggregate summary `0 errors, 5 warnings`\nstdout:\n{stdout}"
    );
}

/// V5.10 acceptance: `adoc build` emits an `adoc.graph.v3` artifact carrying
/// every V5 kind with exact per-kind node counts, the V5.8 evidence edges,
/// V5.10 derived lifecycle fields (`effective_status`, `evidence_quality`),
/// and pilot-scoped source spans; `docs.html` renders each kind distinctly.
#[test]
fn expanded_pilot_build_emits_all_kinds_in_html_and_graph() {
    let repo_root = repo_root();
    let workspace = TestWorkspace::new("expanded-pilot-build");
    let output_directory = workspace.root.join("dist");
    let build_output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&repo_root)
        .env("ADOC_TEST_EMBEDDING_PROVIDER", "deterministic")
        .args([
            "build",
            PILOT_PATH,
            "--out",
            output_directory
                .to_str()
                .expect("output directory path is utf-8"),
        ])
        .output()
        .expect("adoc build runs");

    assert!(
        build_output.status.success(),
        "expected pilot to build cleanly\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build_output.stdout),
        String::from_utf8_lossy(&build_output.stderr)
    );

    // --- Graph artifact: every V5 kind with exact node counts ---
    let graph_text = std::fs::read_to_string(output_directory.join("docs.graph.json"))
        .expect("pilot graph JSON is written");
    let graph: Value = serde_json::from_str(&graph_text).expect("graph JSON is valid");
    assert_eq!(graph["schema_version"], "adoc.graph.v3");

    let nodes = graph["nodes"]
        .as_array()
        .expect("graph JSON nodes is an array");

    let page_count = nodes.iter().filter(|node| node["type"] == "page").count();
    assert_eq!(
        page_count, 12,
        "expected twelve page nodes (11 .adoc strict pages + the markdown meta/REVIEW-CHECKLIST.md)"
    );

    let knowledge_objects: Vec<&Value> = nodes
        .iter()
        .filter(|node| node["type"] == "knowledge_object")
        .collect();
    assert_eq!(
        knowledge_objects.len(),
        20,
        "expected twenty Knowledge Object nodes across all V5 + V5.10 kinds"
    );

    let count_kind = |kind: &str| {
        knowledge_objects
            .iter()
            .filter(|object| object["kind"] == kind)
            .count()
    };
    assert_eq!(count_kind("claim"), 8, "expected eight claim KOs");
    assert_eq!(count_kind("decision"), 1, "expected one decision KO");
    assert_eq!(count_kind("glossary"), 2, "expected two glossary KOs");
    assert_eq!(count_kind("constraint"), 1, "expected one constraint KO");
    assert_eq!(count_kind("procedure"), 1, "expected one procedure KO");
    assert_eq!(count_kind("example"), 2, "expected two example KOs");
    assert_eq!(count_kind("policy"), 1, "expected one policy KO");
    assert_eq!(
        count_kind("agent_instruction"),
        1,
        "expected one agent_instruction KO"
    );
    assert_eq!(
        count_kind("contradiction"),
        1,
        "expected one contradiction KO"
    );
    assert_eq!(count_kind("source"), 2, "expected two source KOs");

    // --- V5.10: derived lifecycle fields on graph nodes ---
    // stale: security.audit.retention is verified + past expires_at.
    let stale_node = knowledge_objects
        .iter()
        .find(|node| node["id"] == "security.audit.retention")
        .expect("security.audit.retention node must exist");
    assert_eq!(
        stale_node["effective_status"], "stale",
        "security.audit.retention must have effective_status stale"
    );
    assert!(
        stale_node["effective_reason"]
            .as_str()
            .unwrap_or("")
            .starts_with("expired:"),
        "security.audit.retention effective_reason must start with expired: (got {:?})",
        stale_node["effective_reason"]
    );
    // The authored status is unchanged — effective_status is derived/additive.
    assert_eq!(
        stale_node["status"], "verified",
        "security.audit.retention authored status must remain verified"
    );

    // contradicted nudge: auth.session.csrf-protection is accepted but referenced by
    // an unresolved contradiction → effective_status: contradicted.
    let nudge_node = knowledge_objects
        .iter()
        .find(|node| node["id"] == "auth.session.csrf-protection")
        .expect("auth.session.csrf-protection node must exist");
    assert_eq!(
        nudge_node["effective_status"], "contradicted",
        "auth.session.csrf-protection must have effective_status contradicted"
    );
    assert_eq!(
        nudge_node["status"], "accepted",
        "auth.session.csrf-protection authored status must remain accepted"
    );

    // evidence quality low: security.csrf-advisory uses only external_url evidence.
    let low_ev_node = knowledge_objects
        .iter()
        .find(|node| node["id"] == "security.csrf-advisory")
        .expect("security.csrf-advisory node must exist");
    assert_eq!(
        low_ev_node["evidence_quality"], "low",
        "security.csrf-advisory must have evidence_quality low"
    );

    // Every Knowledge Object must carry a pilot-scoped source span for citation.
    for object in &knowledge_objects {
        let path = object["source_span"]["path"]
            .as_str()
            .unwrap_or_else(|| panic!("KO {} missing source_span.path", object["id"]));
        assert!(
            path.contains("examples/expanded-pilot/"),
            "KO {} source span must point back into the pilot, got {path}",
            object["id"]
        );
    }

    // --- V5.8 evidence edges: claim + decision -> source ---
    let edges = graph["edges"]
        .as_array()
        .expect("graph JSON edges is an array");
    let evidence_edges: Vec<&Value> = edges
        .iter()
        .filter(|edge| edge["kind"] == "evidence")
        .collect();
    assert_eq!(
        evidence_edges.len(),
        2,
        "expected two evidence edges (claim + decision -> billing.consume-use-case)"
    );
    for edge in &evidence_edges {
        assert_eq!(
            edge["target"], "billing.consume-use-case",
            "evidence edges must target the source object"
        );
    }

    // --- HTML: every V5 kind renders distinctly ---
    let html =
        std::fs::read_to_string(output_directory.join("docs.html")).expect("pilot HTML is written");

    assert!(
        html.contains("Authored knowledge, NOT runtime ACL"),
        "agent_instruction must render the runtime-not-enforced banner"
    );
    assert!(
        html.contains(r#"class="contradiction__claims""#),
        "contradiction must render its conflicting-claims block"
    );
    assert!(
        html.contains(r##"href="#auth.session.memory-storage""##)
            && html.contains(r##"href="#auth.session.local-storage-allowed""##),
        "contradiction must link both conflicting claims"
    );
    assert!(
        html.contains("source--source_code") && html.contains("source--external_url"),
        "both source kinds must render distinct metadata blocks"
    );
    assert!(
        html.contains("consume.use-case.ts") && html.contains("cwe.mitre.org"),
        "source path and external URL must appear in rendered HTML"
    );
    assert!(
        html.contains("<ol"),
        "procedure body must render as an ordered list"
    );
}

/// V5.10 acceptance: the V5 kinds are retrievable. `adoc why` cites the
/// verified procedure, `adoc graph` traverses the active policy to a related
/// object (now stale) and back, and `adoc search "policy"` returns the policy
/// first (docs/V5-DESIGN.md §V5.9/§V5.10).
#[test]
fn expanded_pilot_retrieval_why_graph_search() {
    let repo_root = repo_root();
    let workspace = TestWorkspace::new("expanded-pilot-retrieval");
    let dist = workspace.root.join("dist");
    let build = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&repo_root)
        .env("ADOC_TEST_EMBEDDING_PROVIDER", "deterministic")
        .args([
            "build",
            PILOT_PATH,
            "--out",
            dist.to_str().expect("dist path is utf-8"),
        ])
        .output()
        .expect("adoc build runs");
    assert!(
        build.status.success(),
        "retrieval prerequisite build must succeed\nstderr:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let graph = dist.join("docs.graph.json");
    let graph_arg = graph.to_str().expect("graph path is utf-8");

    // --- why: cite the verified procedure ---
    let why = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "why",
            "auth.key.rotate",
            "--artifact",
            graph_arg,
            "--format",
            "json",
        ])
        .output()
        .expect("adoc why runs");
    assert!(why.status.success(), "adoc why must succeed");
    let why_env: Value = serde_json::from_slice(&why.stdout).expect("why stdout is JSON");
    assert_eq!(why_env["schema_version"], "adoc.retrieval.v0");
    assert_eq!(why_env["records"][0]["id"], "auth.key.rotate");
    assert_eq!(why_env["records"][0]["kind"], "procedure");
    assert_eq!(why_env["records"][0]["status"], "verified");

    // --- graph: active policy traverses to its related object and back ---
    let graph_out = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "graph",
            "security.production-db-access",
            "--artifact",
            graph_arg,
            "--format",
            "json",
        ])
        .output()
        .expect("adoc graph runs");
    assert!(graph_out.status.success(), "adoc graph must succeed");
    let graph_env: Value = serde_json::from_slice(&graph_out.stdout).expect("graph stdout is JSON");
    assert_eq!(graph_env["schema_version"], "adoc.graph.traversal.v0");
    assert_eq!(graph_env["root"], "security.production-db-access");
    let reached = graph_env["nodes"]
        .as_array()
        .expect("nodes array")
        .iter()
        .any(|node| node["id"] == "security.audit.retention");
    assert!(
        reached,
        "graph traversal from the policy must reach its related claim\n{graph_env:#}"
    );

    // --- search: "policy" returns the policy first ---
    let search = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "search",
            "policy",
            "--artifact",
            graph_arg,
            "--format",
            "json",
        ])
        .output()
        .expect("adoc search runs");
    assert!(search.status.success(), "adoc search must succeed");
    let search_env: Value = serde_json::from_slice(&search.stdout).expect("search stdout is JSON");
    assert_eq!(search_env["schema_version"], "adoc.retrieval.v0");
    assert_eq!(
        search_env["records"][0]["id"], "security.production-db-access",
        "the active policy must be the top search result for \"policy\""
    );
}

/// V5.10 acceptance: `adoc diff`, `adoc review`, and `adoc review --patch`
/// behave over a git fixture carrying V5 kinds. A body edit on a verified
/// claim produces one Changed entry, a re-verify obligation, and a clean
/// embedded `adoc.patch.check.v0` result when a matching patch is supplied.
#[test]
fn expanded_pilot_diff_review_patch() {
    let workspace = TestWorkspace::new("expanded-pilot-diff-review-patch");
    git_init_fixture(&workspace);

    let base_adoc = concat!(
        "# Pilot Billing @doc(pilot.billing)\n",
        "\n",
        "::source pilot.consume-use-case\n",
        "kind: source_code\n",
        "path: apps/backend/src/features/credits/consume.use-case.ts\n",
        "owner: backend-platform\n",
        "--\n",
        "Implementation of credit consumption.\n",
        "::\n",
        "\n",
        "::claim pilot.credits.consume\n",
        "status: verified\n",
        "owner: backend-platform\n",
        "verified_at: 2026-05-01\n",
        "test: cargo test credits\n",
        "evidence_ref: pilot.consume-use-case\n",
        "--\n",
        "Credit consumption is handled by the use-case implementation.\n",
        "::\n",
    );
    workspace.write("knowledge/billing.adoc", base_adoc);
    run_git(&workspace, &["add", "-A"]);
    run_git(&workspace, &["commit", "-m", "base pilot"]);

    run_git(&workspace, &["checkout", "-b", "feature"]);
    let head_adoc = base_adoc.replace(
        "Credit consumption is handled by the use-case implementation.",
        "Credit consumption is handled by the ledger-first use-case implementation.",
    );
    workspace.write("knowledge/billing.adoc", &head_adoc);
    run_git(&workspace, &["add", "-A"]);
    run_git(&workspace, &["commit", "-m", "head: tighten claim body"]);

    // --- diff: one Changed entry, body field change ---
    let diff = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .env("ADOC_TEST_EMBEDDING_PROVIDER", "deterministic")
        .args(["diff", "main", "--format", "json"])
        .output()
        .expect("adoc diff runs");
    assert!(
        diff.status.success(),
        "adoc diff must succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&diff.stdout),
        String::from_utf8_lossy(&diff.stderr)
    );
    let diff_env: Value = serde_json::from_slice(&diff.stdout).expect("diff stdout is JSON");
    assert_eq!(diff_env["schema_version"], "adoc.diff.v0");
    let changed = diff_env["changed"].as_array().expect("changed array");
    assert_eq!(changed.len(), 1, "expected one Changed entry: {changed:#?}");
    assert_eq!(changed[0]["id"], "pilot.credits.consume");
    let field_changes = changed[0]["field_changes"]
        .as_array()
        .expect("field_changes array");
    assert_eq!(field_changes[0]["type"], "body");
    let base_hash = changed[0]["head"]["content_hash"]
        .as_str()
        .expect("head content_hash")
        .to_string();

    // --- review: re-verify obligation on the verified claim ---
    let review = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .env("ADOC_TEST_EMBEDDING_PROVIDER", "deterministic")
        .args(["review", "main", "--format", "json"])
        .output()
        .expect("adoc review runs");
    assert!(review.status.success(), "adoc review must succeed");
    let review_env: Value = serde_json::from_slice(&review.stdout).expect("review stdout is JSON");
    assert_eq!(review_env["schema_version"], "adoc.review.v0");
    let obligations = review_env["proof_obligations"]
        .as_array()
        .expect("proof_obligations array");
    assert!(
        obligations
            .iter()
            .any(|o| o["object_id"] == "pilot.credits.consume"),
        "expected a proof obligation on the modified verified claim: {obligations:#?}"
    );

    // --- review --patch: embeds a valid adoc.patch.check.v0 ---
    let patch_json = format!(
        concat!(
            "{{\n",
            "  \"schema_version\": \"adoc.patch.v0\",\n",
            "  \"op\": \"replace_body\",\n",
            "  \"target\": \"pilot.credits.consume\",\n",
            "  \"base_hash\": \"{}\",\n",
            "  \"changes\": {{ \"body\": \"Patched body.\" }},\n",
            "  \"reason\": \"V5.9 pilot patch composition\"\n",
            "}}\n",
        ),
        base_hash,
    );
    workspace.write("patch.json", &patch_json);
    let review_patch = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .env("ADOC_TEST_EMBEDDING_PROVIDER", "deterministic")
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
        review_patch.status.success(),
        "adoc review --patch must succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&review_patch.stdout),
        String::from_utf8_lossy(&review_patch.stderr)
    );
    let rp_env: Value = serde_json::from_slice(&review_patch.stdout).expect("review --patch JSON");
    assert_eq!(
        rp_env["patch_check"]["schema_version"],
        "adoc.patch.check.v0"
    );
    assert_eq!(rp_env["patch_check"]["valid"], serde_json::json!(true));
    assert_eq!(rp_env["patch_check"]["target"], "pilot.credits.consume");
}

/// V5.9 acceptance (docs/V5-DESIGN.md:539): a reader pinned to the old
/// `adoc.graph.v2` model fails gracefully. Feeding a stale v2 artifact to a
/// read command exits 2 with the `schema.unsupported_version` diagnostic
/// rather than silently dropping the new V5 kinds.
#[test]
fn expanded_pilot_reader_rejects_stale_v2_graph() {
    let workspace = TestWorkspace::new("expanded-pilot-stale-v2");
    let stale = workspace.write(
        "stale.graph.json",
        "{\n  \"schema_version\": \"adoc.graph.v2\",\n  \"nodes\": [],\n  \"edges\": [],\n  \"diagnostics\": []\n}\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args([
            "why",
            "auth.key.rotate",
            "--artifact",
            stale.to_str().expect("artifact path is utf-8"),
            "--format",
            "json",
        ])
        .output()
        .expect("adoc why runs");

    assert_eq!(
        output.status.code(),
        Some(2),
        "stale v2 artifact must produce an artifact error exit"
    );
    let env: Value = serde_json::from_slice(&output.stdout).expect("stdout is JSON");
    assert_eq!(env["schema_version"], "adoc.retrieval.v0");
    assert_eq!(env["diagnostics"][0]["code"], "schema.unsupported_version");
}

/// Init a temp git repository for the diff/review/patch fixture. Mirrors the
/// env-var scrubbing pattern from `markdown_pilot.rs` so hooks running inside
/// an outer git context do not lock the per-fixture tempdir.
fn git_init_fixture(workspace: &TestWorkspace) {
    run_git(workspace, &["init", "--initial-branch=main"]);
    run_git(workspace, &["config", "user.email", "test@adoc.dev"]);
    run_git(workspace, &["config", "user.name", "adoc tests"]);
    run_git(workspace, &["config", "commit.gpgsign", "false"]);
}

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
