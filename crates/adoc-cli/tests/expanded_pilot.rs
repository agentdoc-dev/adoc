//! V5.10 + V6.5.5 end-to-end test for the Expanded Pilot.
//!
//! Validates `examples/expanded-pilot/` against the full V5 + V5.10 acceptance
//! contract from `docs/design/V5-DESIGN.md`, extended by V6.5.5 to the fifteen-kind
//! vocabulary: the V5 kinds (constraint, procedure, example, policy,
//! agent_instruction, contradiction, source) plus the V6.5 kinds (api,
//! observation, question, task), the V5.8 typed evidence model, and all four
//! V5.10 lifecycle signals, across auth / billing / security / team domains.
//!
//! Diagnostic budget (documented in `docs/guides/expanded-pilot.md`): 0 errors,
//! 6 warnings:
//!
//! | Code                                  | Count | Object                                     |
//! | :------------------------------------ | :---: | :----------------------------------------- |
//! | `lifecycle.expired`                   |   2   | `billing.credits.legacy-export`, `security.audit.retention` |
//! | `schema.policy_review_overdue`        |   1   | `security.production-db-access`            |
//! | `claim.evidence_quality_low`          |   1   | `security.csrf-advisory`                   |
//! | `schema.claim_contradicted_by_unresolved` | 1 | `auth.session.csrf-protection`             |
//! | `task.overdue`                        |   1   | `billing.update-support-runbook`           |
//!
//! All warnings are driven by fixed past dates / wide-margin fixtures so the
//! budget is clock-stable on any realistic future run date.

use std::path::{Path, PathBuf};
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

/// V5.10 + V6.5.5 acceptance: `adoc check` over the full pilot exits 0 with
/// the exact diagnostic budget published in `docs/guides/expanded-pilot.md` — 0
/// errors and exactly 6 warnings (2 `lifecycle.expired`,
/// 1 `schema.policy_review_overdue`, 1 `claim.evidence_quality_low`,
/// 1 `schema.claim_contradicted_by_unresolved`, 1 `task.overdue`).
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
    let task_overdue_count = stdout.matches("warning[task.overdue]").count();
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
        task_overdue_count, 1,
        "expected one task.overdue warning (billing.update-support-runbook, wide-margin past due)\nstdout:\n{stdout}"
    );
    assert_eq!(
        error_count, 0,
        "pilot must produce zero errors (every kind is strict-valid)\nstdout:\n{stdout}"
    );
    assert!(
        stdout.contains("0 errors, 6 warnings"),
        "expected aggregate summary `0 errors, 6 warnings`\nstdout:\n{stdout}"
    );
}

/// V5.10 acceptance: `adoc build` emits an `adoc.graph.v5` artifact carrying
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
    assert_eq!(graph["schema_version"], "adoc.graph.v5");

    let nodes = graph["nodes"]
        .as_array()
        .expect("graph JSON nodes is an array");

    let page_count = nodes.iter().filter(|node| node["type"] == "page").count();
    assert_eq!(
        page_count, 16,
        "expected sixteen page nodes (15 .adoc strict pages + the markdown meta/REVIEW-CHECKLIST.md)"
    );

    let knowledge_objects: Vec<&Value> = nodes
        .iter()
        .filter(|node| node["type"] == "knowledge_object")
        .collect();
    assert_eq!(
        knowledge_objects.len(),
        27,
        "expected twenty-seven Knowledge Object nodes across the fifteen-kind vocabulary"
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
    assert_eq!(count_kind("source"), 3, "expected three source KOs");
    assert_eq!(count_kind("api"), 1, "expected one api KO");
    assert_eq!(count_kind("observation"), 1, "expected one observation KO");
    assert_eq!(count_kind("question"), 2, "expected two question KOs");
    assert_eq!(count_kind("task"), 2, "expected two task KOs");

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
            Path::new(path).is_relative() && path.ends_with(".adoc"),
            "KO {} source span must be relative to the explicit pilot root, got {path}",
            object["id"]
        );
    }

    // --- V5.8 evidence edges: claim + decision -> source, api -> api_schema source ---
    let edges = graph["edges"]
        .as_array()
        .expect("graph JSON edges is an array");
    let evidence_edges: Vec<&Value> = edges
        .iter()
        .filter(|edge| edge["kind"] == "evidence")
        .collect();
    assert_eq!(
        evidence_edges.len(),
        3,
        "expected three evidence edges (claim + decision -> billing.consume-use-case, api -> billing.openapi-schema)"
    );
    let mut evidence_targets: Vec<&str> = evidence_edges
        .iter()
        .map(|edge| edge["target"].as_str().expect("evidence edge target"))
        .collect();
    evidence_targets.sort_unstable();
    assert_eq!(
        evidence_targets,
        [
            "billing.consume-use-case",
            "billing.consume-use-case",
            "billing.openapi-schema"
        ],
        "evidence edges must target the source objects"
    );

    // --- V6.5.3/V6.5.4 relation edges: question resolved_by + task depends_on ---
    assert!(
        edges.iter().any(|edge| edge["kind"] == "resolved_by"
            && edge["source"] == "billing.ledger-architecture"
            && edge["target"] == "billing.credits.use-ledger"),
        "answered question must emit a derived resolved_by edge to the resolving decision"
    );
    assert!(
        edges.iter().any(|edge| edge["kind"] == "relation"
            && edge["relation"] == "depends_on"
            && edge["source"] == "billing.update-support-runbook"
            && edge["target"] == "billing.credits.consume"),
        "open task must emit its depends_on relation edge"
    );

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

    // --- V6.5.5: the four V6.5 kinds render distinctly ---
    assert!(
        html.contains(r#"<span class="api__method">POST</span>"#)
            && html.contains(r#"<code class="api__path">/api/billing/credits/consume</code>"#),
        "api must render its endpoint signature (method badge + code path)"
    );
    assert!(
        html.contains("source--api_schema"),
        "the api_schema source kind must render its distinct metadata block"
    );
    assert!(
        html.contains("37") && html.contains("2024-04-30"),
        "observation must render sample size and observed date"
    );
    assert!(
        html.contains(r#"class="question__open-badge""#),
        "open question must render the Open badge"
    );
    assert!(
        html.contains(r#"class="question__resolved-by""#)
            && html.contains(r##"href="#billing.credits.use-ledger""##),
        "answered question must link the resolving decision"
    );
    assert!(
        html.contains("task task--open task--overdue"),
        "the overdue open task must render the task--overdue modifier"
    );
    assert!(
        html.contains("task task--done"),
        "the done task must render its done state"
    );
}

/// V5.10 acceptance: the V5 kinds are retrievable. `adoc why` cites the
/// verified procedure, `adoc graph` traverses the active policy to a related
/// object (now stale) and back, and `adoc search "policy"` returns the policy
/// first (docs/design/V5-DESIGN.md §V5.9/§V5.10).
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
    assert_eq!(why_env["schema_version"], "adoc.retrieval.v1");
    assert_eq!(why_env["records"][0]["id"], "auth.key.rotate");
    assert_eq!(why_env["records"][0]["kind"], "procedure");
    assert_eq!(why_env["records"][0]["status"], "verified");

    // --- why: ADR-0035 dual-emit clones severity/trust into retrieval records ---
    let why_constraint = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "why",
            "auth.session.no-local-storage",
            "--artifact",
            graph_arg,
            "--format",
            "json",
        ])
        .output()
        .expect("adoc why runs");
    assert!(
        why_constraint.status.success(),
        "adoc why must succeed for the constraint"
    );
    let why_constraint_env: Value =
        serde_json::from_slice(&why_constraint.stdout).expect("why stdout is JSON");
    assert_eq!(
        why_constraint_env["records"][0]["severity"], "critical",
        "constraint retrieval record must carry the dual-emitted severity"
    );

    let why_agent = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "why",
            "auth.docs-answering-policy",
            "--artifact",
            graph_arg,
            "--format",
            "json",
        ])
        .output()
        .expect("adoc why runs");
    assert!(
        why_agent.status.success(),
        "adoc why must succeed for the agent_instruction"
    );
    let why_agent_env: Value =
        serde_json::from_slice(&why_agent.stdout).expect("why stdout is JSON");
    assert_eq!(
        why_agent_env["records"][0]["trust"], "team",
        "agent_instruction retrieval record must carry the dual-emitted trust"
    );

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

    // --- search: "policy" returns the policy first among Knowledge Objects.
    // V1.7.1 blends prose into the default list (a policies-page prose block
    // legitimately competes for this query), so the KO-ranking assertion
    // scopes with --objects-only per the ADR-0040 filters-and-fixtures rule.
    let search = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "search",
            "policy",
            "--objects-only",
            "--artifact",
            graph_arg,
            "--format",
            "json",
        ])
        .output()
        .expect("adoc search runs");
    assert!(search.status.success(), "adoc search must succeed");
    let search_env: Value = serde_json::from_slice(&search.stdout).expect("search stdout is JSON");
    assert_eq!(search_env["schema_version"], "adoc.retrieval.v1");
    assert_eq!(
        search_env["records"][0]["id"], "security.production-db-access",
        "the active policy must be the top search result for \"policy\""
    );

    // --- V6.5.5: the V6.5 kinds are retrievable ---
    // why: the verified api record carries kind and lifecycle status.
    let why_api = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "why",
            "billing.consume-credit",
            "--artifact",
            graph_arg,
            "--format",
            "json",
        ])
        .output()
        .expect("adoc why runs");
    assert!(
        why_api.status.success(),
        "adoc why must succeed for the api"
    );
    let why_api_env: Value = serde_json::from_slice(&why_api.stdout).expect("why stdout is JSON");
    assert_eq!(why_api_env["records"][0]["kind"], "api");
    assert_eq!(why_api_env["records"][0]["status"], "verified");

    // why: the answering decision lists the answered question (V6.5.3).
    let why_decision = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "why",
            "billing.credits.use-ledger",
            "--artifact",
            graph_arg,
            "--format",
            "json",
        ])
        .output()
        .expect("adoc why runs");
    assert!(
        why_decision.status.success(),
        "adoc why must succeed for the decision"
    );
    let why_decision_env: Value =
        serde_json::from_slice(&why_decision.stdout).expect("why stdout is JSON");
    assert_eq!(
        why_decision_env["records"][0]["resolved_questions"],
        serde_json::json!(["billing.ledger-architecture"]),
        "the answering decision must list the question it resolved"
    );

    // why: the overdue task record carries kind and lifecycle status.
    let why_task = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "why",
            "billing.update-support-runbook",
            "--artifact",
            graph_arg,
            "--format",
            "json",
        ])
        .output()
        .expect("adoc why runs");
    assert!(
        why_task.status.success(),
        "adoc why must succeed for the task"
    );
    let why_task_env: Value = serde_json::from_slice(&why_task.stdout).expect("why stdout is JSON");
    assert_eq!(why_task_env["records"][0]["kind"], "task");
    assert_eq!(why_task_env["records"][0]["status"], "open");

    // search: the observation body is BM25-findable (V6.5.2 acceptance shape).
    let search_observation = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "search",
            "misunderstand credit usage",
            "--artifact",
            graph_arg,
            "--format",
            "json",
        ])
        .output()
        .expect("adoc search runs");
    assert!(
        search_observation.status.success(),
        "adoc search must succeed for the observation query"
    );
    let search_observation_env: Value =
        serde_json::from_slice(&search_observation.stdout).expect("search stdout is JSON");
    let observation_found = search_observation_env["records"]
        .as_array()
        .expect("search records array")
        .iter()
        .any(|record| record["id"] == "onboarding.credit-confusion");
    assert!(
        observation_found,
        "the observation body must be findable via search: {search_observation_env:#}"
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
    workspace.write(
        "agentdoc.config.yaml",
        "version: 1\nmode: strict\ndocs_path: knowledge\n",
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

/// V5.9 acceptance (docs/design/V5-DESIGN.md:539): a reader pinned to the old
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
    assert_eq!(env["schema_version"], "adoc.retrieval.v1");
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

/// V6.1 acceptance: `adoc stale` re-derives lifecycle signals at read time
/// from the built graph artifact (docs/roadmap/ROADMAP-V6.md §V6.1).
///
/// The default listing must contain exactly 3 records sorted most-overdue
/// first (fixed due dates 2020-03-31 / 2024-01-01 / 2026-01-15 keep the order
/// clock-stable); `--within 36500d` additionally lists the two far-future
/// verified objects as `expiring_soon`. The command is a query: exit 0 with
/// records, exit 2 only on artifact-load failure, and `--format markdown`
/// is still rejected here (no markdown presenter for this command).
#[test]
fn expanded_pilot_stale_query() {
    let repo_root = repo_root();
    let workspace = TestWorkspace::new("expanded-pilot-stale");
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
        "stale prerequisite build must succeed\nstderr:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let graph = dist.join("docs.graph.json");
    let graph_arg = graph.to_str().expect("graph path is utf-8");

    // --- default listing: exactly 3 records, most-overdue first ---
    let stale = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args(["stale", "--artifact", graph_arg, "--format", "json"])
        .output()
        .expect("adoc stale runs");
    assert!(
        stale.status.success(),
        "adoc stale is a query and must exit 0 even with records\nstderr:\n{}",
        String::from_utf8_lossy(&stale.stderr)
    );
    let envelope: Value = serde_json::from_slice(&stale.stdout).expect("stale stdout is JSON");
    assert_eq!(envelope["schema_version"], "adoc.stale.v0");
    assert_eq!(
        envelope["evaluated_at"].as_str().map(str::len),
        Some(10),
        "evaluated_at must be a YYYY-MM-DD date: {:?}",
        envelope["evaluated_at"]
    );
    let records = envelope["records"].as_array().expect("records array");
    assert_eq!(records.len(), 3, "expected exactly 3 records: {records:#?}");

    assert_eq!(records[0]["id"], "security.production-db-access");
    assert_eq!(records[0]["category"], "review_overdue");
    assert_eq!(records[0]["reason"], "review_due:2020-03-31");
    assert_eq!(records[0]["kind"], "policy");
    assert_eq!(records[0]["authored_status"], "active");
    assert_eq!(records[0]["effective_status"], "active");
    assert!(
        records[0]["days_overdue"].as_u64().expect("days_overdue") > 0,
        "review_overdue must carry positive days_overdue"
    );
    assert!(
        records[0]["source_path"]
            .as_str()
            .expect("source_path")
            .contains("security/policies.adoc")
    );

    assert_eq!(records[1]["id"], "security.audit.retention");
    assert_eq!(records[1]["category"], "stale");
    assert_eq!(records[1]["authored_status"], "verified");
    assert_eq!(
        records[1]["effective_status"], "stale",
        "verified + expired must re-derive stale at read time"
    );
    assert_eq!(records[1]["reason"], "expired:2024-01-01");
    assert_eq!(records[1]["expires_at"], "2024-01-01");

    assert_eq!(records[2]["id"], "billing.credits.legacy-export");
    assert_eq!(records[2]["category"], "stale");
    assert_eq!(records[2]["authored_status"], "draft");
    assert_eq!(
        records[2]["effective_status"], "draft",
        "draft + expired is listed by category but derives no effective status"
    );
    assert_eq!(records[2]["reason"], "expired:2026-01-15");

    // --- --within horizon adds the two far-future verified objects ---
    let within = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "stale",
            "--artifact",
            graph_arg,
            "--within",
            "36500d",
            "--format",
            "json",
        ])
        .output()
        .expect("adoc stale --within runs");
    assert!(within.status.success(), "adoc stale --within must exit 0");
    let within_env: Value =
        serde_json::from_slice(&within.stdout).expect("stale --within stdout is JSON");
    let within_records = within_env["records"].as_array().expect("records array");
    assert_eq!(
        within_records.len(),
        5,
        "expected the 3 default records plus 2 expiring_soon: {within_records:#?}"
    );
    assert_eq!(within_records[3]["id"], "billing.credits.consume");
    assert_eq!(within_records[3]["category"], "expiring_soon");
    assert_eq!(within_records[3]["reason"], "expires:2120-01-01");
    assert!(
        within_records[3]["days_remaining"]
            .as_u64()
            .expect("days_remaining")
            > 0
    );
    assert_eq!(within_records[4]["id"], "auth.mfa.enforced");
    assert_eq!(within_records[4]["category"], "expiring_soon");
    assert!(within_records[4].get("days_overdue").is_none());

    // --- plain output smoke ---
    let plain = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args(["stale", "--artifact", graph_arg, "--format", "plain"])
        .output()
        .expect("adoc stale --format plain runs");
    assert!(plain.status.success(), "plain stale must exit 0");
    let plain_stdout = String::from_utf8_lossy(&plain.stdout);
    assert!(
        plain_stdout.contains("Stale: 3 record(s) as of"),
        "plain output must lead with the record count header:\n{plain_stdout}"
    );
    assert!(plain_stdout.contains("security.audit.retention"));
    assert!(
        plain_stdout.contains("verified -> stale"),
        "plain output must show the authored -> effective transition:\n{plain_stdout}"
    );

    // --- markdown stays diff/review-only ---
    let markdown = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args(["stale", "--artifact", graph_arg, "--format", "markdown"])
        .output()
        .expect("adoc stale --format markdown runs");
    assert_eq!(
        markdown.status.code(),
        Some(2),
        "markdown format must be rejected for adoc stale"
    );
    assert!(
        String::from_utf8_lossy(&markdown.stderr).contains("cli.format"),
        "markdown rejection must name the cli.format diagnostic"
    );

    // --- artifact-load failure exits 2 with an empty-records envelope ---
    let missing = dist.join("does-not-exist.graph.json");
    let missing_arg = missing.to_str().expect("missing path is utf-8");
    let failed = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args(["stale", "--artifact", missing_arg, "--format", "json"])
        .output()
        .expect("adoc stale with missing artifact runs");
    assert_eq!(
        failed.status.code(),
        Some(2),
        "artifact-load failure must exit 2"
    );
    let failed_env: Value =
        serde_json::from_slice(&failed.stdout).expect("failure envelope is JSON");
    assert_eq!(failed_env["schema_version"], "adoc.stale.v0");
    assert_eq!(failed_env["records"], serde_json::json!([]));
    assert!(
        !failed_env["diagnostics"]
            .as_array()
            .expect("diagnostics array")
            .is_empty(),
        "failure envelope must carry fix-oriented diagnostics"
    );
}

/// V6.2 acceptance: exactly 1 unresolved contradiction and exactly 3
/// contradicted claims, each carrying the implicating contradiction ids so
/// consumers never join the two lists themselves. The envelope is a pure
/// function of the artifact: no `evaluated_at`. Exit 0 with findings, exit 2
/// only on artifact-load failure; `--format markdown` is still rejected
/// here (no markdown presenter for this command).
#[test]
fn expanded_pilot_contradictions_query() {
    let repo_root = repo_root();
    let workspace = TestWorkspace::new("expanded-pilot-contradictions");
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
        "contradictions prerequisite build must succeed\nstderr:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let graph = dist.join("docs.graph.json");
    let graph_arg = graph.to_str().expect("graph path is utf-8");

    // --- default listing: exactly 1 contradiction, exactly 3 claims ---
    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "contradictions",
            "--artifact",
            graph_arg,
            "--format",
            "json",
        ])
        .output()
        .expect("adoc contradictions runs");
    assert!(
        output.status.success(),
        "adoc contradictions is a query and must exit 0 even with findings\nstderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: Value =
        serde_json::from_slice(&output.stdout).expect("contradictions stdout is JSON");
    assert_eq!(envelope["schema_version"], "adoc.contradictions.v0");
    assert!(
        envelope.get("evaluated_at").is_none(),
        "the contradictions envelope is clock-free — no evaluated_at"
    );

    let contradictions = envelope["contradictions"]
        .as_array()
        .expect("contradictions array");
    assert_eq!(
        contradictions.len(),
        1,
        "expected exactly 1 contradiction: {contradictions:#?}"
    );
    let conflict = &contradictions[0];
    assert_eq!(conflict["id"], "auth.session.conflict");
    assert_eq!(conflict["severity"], "high");
    assert_eq!(conflict["status"], "unresolved");
    assert_eq!(
        conflict["claims"],
        serde_json::json!([
            "auth.session.csrf-protection",
            "auth.session.local-storage-allowed",
            "auth.session.memory-storage",
        ]),
        "claims echo the parse-time sorted list"
    );
    assert!(
        conflict["source_path"]
            .as_str()
            .expect("source_path")
            .contains("security/contradictions.adoc")
    );
    assert_eq!(
        conflict["summary"], "Claim auth.session.memory-storage requires memory-only storage while",
        "summary is the first body line"
    );

    let claims = envelope["contradicted_claims"]
        .as_array()
        .expect("contradicted_claims array");
    assert_eq!(
        claims.len(),
        3,
        "expected exactly 3 contradicted claims: {claims:#?}"
    );

    assert_eq!(claims[0]["id"], "auth.session.csrf-protection");
    assert_eq!(
        claims[0]["authored_status"], "accepted",
        "csrf-protection's authored status is untouched"
    );
    assert_eq!(
        claims[0]["effective_status"], "contradicted",
        "implication by an unresolved contradiction derives contradicted"
    );
    assert_eq!(
        claims[0]["effective_reason"],
        "contradiction:auth.session.conflict"
    );

    assert_eq!(claims[1]["id"], "auth.session.local-storage-allowed");
    assert_eq!(claims[1]["authored_status"], "contradicted");
    assert_eq!(claims[2]["id"], "auth.session.memory-storage");
    assert_eq!(claims[2]["authored_status"], "contradicted");

    for claim in claims {
        assert_eq!(
            claim["contradiction_ids"],
            serde_json::json!(["auth.session.conflict"]),
            "every contradicted claim carries its implicating contradiction ids"
        );
    }

    // --- --all is identical on this pilot (no resolved/dismissed records) ---
    let all = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "contradictions",
            "--artifact",
            graph_arg,
            "--all",
            "--format",
            "json",
        ])
        .output()
        .expect("adoc contradictions --all runs");
    assert!(
        all.status.success(),
        "adoc contradictions --all must exit 0"
    );
    let all_env: Value = serde_json::from_slice(&all.stdout).expect("--all stdout is JSON");
    assert_eq!(
        all_env, envelope,
        "--all must be identical when every contradiction is unresolved"
    );

    // --- plain output smoke ---
    let plain = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "contradictions",
            "--artifact",
            graph_arg,
            "--format",
            "plain",
        ])
        .output()
        .expect("adoc contradictions --format plain runs");
    assert!(plain.status.success(), "plain contradictions must exit 0");
    let plain_stdout = String::from_utf8_lossy(&plain.stdout);
    assert!(
        plain_stdout.contains("Contradictions: 1 contradiction(s)"),
        "plain output must lead with the contradiction count header:\n{plain_stdout}"
    );
    assert!(
        plain_stdout.contains("Contradicted claims: 3 contradicted claim(s)"),
        "plain output must include the contradicted-claims section:\n{plain_stdout}"
    );
    assert!(
        plain_stdout.contains("accepted -> contradicted"),
        "plain output must show the authored -> effective transition:\n{plain_stdout}"
    );

    // --- markdown stays diff/review-only ---
    let markdown = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "contradictions",
            "--artifact",
            graph_arg,
            "--format",
            "markdown",
        ])
        .output()
        .expect("adoc contradictions --format markdown runs");
    assert_eq!(
        markdown.status.code(),
        Some(2),
        "markdown format must be rejected for adoc contradictions"
    );
    assert!(
        String::from_utf8_lossy(&markdown.stderr).contains("cli.format"),
        "markdown rejection must name the cli.format diagnostic"
    );

    // --- artifact-load failure exits 2 with an empty-lists envelope ---
    let missing = dist.join("does-not-exist.graph.json");
    let missing_arg = missing.to_str().expect("missing path is utf-8");
    let failed = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "contradictions",
            "--artifact",
            missing_arg,
            "--format",
            "json",
        ])
        .output()
        .expect("adoc contradictions with missing artifact runs");
    assert_eq!(
        failed.status.code(),
        Some(2),
        "artifact-load failure must exit 2"
    );
    let failed_env: Value =
        serde_json::from_slice(&failed.stdout).expect("failure envelope is JSON");
    assert_eq!(failed_env["schema_version"], "adoc.contradictions.v0");
    assert_eq!(failed_env["contradictions"], serde_json::json!([]));
    assert_eq!(failed_env["contradicted_claims"], serde_json::json!([]));
    assert!(
        !failed_env["diagnostics"]
            .as_array()
            .expect("diagnostics array")
            .is_empty(),
        "failure envelope must carry fix-oriented diagnostics"
    );
}

/// V6.3 + V6.5.5 `adoc impacted-by` over the Expanded Pilot.
///
/// `billing.credits.consume` (verified claim) and `billing.credits.use-ledger`
/// (accepted decision) both carry `evidence_ref: billing.consume-use-case`, a
/// `source` object whose `path` is the changed file — both must surface with
/// one `evidence_path` reason resolved `via_source_object`. The verified api
/// `billing.consume-credit` (a verified subject since V6.5.1) surfaces for its
/// schema path `openapi/billing.yaml`. The constraint and procedure `impacts:`
/// declarations remain outside the verified-subject scope: querying their
/// paths is empty, exit 0.
#[test]
fn expanded_pilot_impacted_by_query() {
    let repo_root = repo_root();
    let workspace = TestWorkspace::new("expanded-pilot-impacted-by");
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
        "impacted-by prerequisite build must succeed\nstderr:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let graph = dist.join("docs.graph.json");
    let graph_arg = graph.to_str().expect("graph path is utf-8");

    // --- evidence-path query: both verified subjects via the source object ---
    let impacted = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "impacted-by",
            "apps/backend/src/features/credits/consume.use-case.ts",
            "--artifact",
            graph_arg,
            "--format",
            "json",
        ])
        .output()
        .expect("adoc impacted-by runs");
    assert!(
        impacted.status.success(),
        "impacted-by is a query and must exit 0\nstderr:\n{}",
        String::from_utf8_lossy(&impacted.stderr)
    );
    let envelope: Value =
        serde_json::from_slice(&impacted.stdout).expect("impacted-by stdout is JSON");
    assert_eq!(envelope["schema_version"], "adoc.impacted.v0");

    let records = envelope["impacted"].as_array().expect("impacted array");
    assert_eq!(
        records.len(),
        2,
        "expected exactly 2 impacted objects: {records:#?}"
    );

    assert_eq!(records[0]["id"], "billing.credits.consume");
    assert_eq!(records[0]["kind"], "claim");
    assert_eq!(records[0]["status"], "verified");
    assert_eq!(records[1]["id"], "billing.credits.use-ledger");
    assert_eq!(records[1]["kind"], "decision");
    assert_eq!(records[1]["status"], "accepted");

    for record in records {
        let reasons = record["reasons"].as_array().expect("reasons array");
        assert_eq!(reasons.len(), 1, "one reason each: {reasons:#?}");
        assert_eq!(reasons[0]["kind"], "evidence_path");
        assert_eq!(
            reasons[0]["matched_path"],
            "apps/backend/src/features/credits/consume.use-case.ts"
        );
        assert_eq!(reasons[0]["via_source_object"], "billing.consume-use-case");
    }

    let obligations = envelope["proof_obligations"]
        .as_array()
        .expect("proof_obligations array");
    assert_eq!(obligations.len(), 2, "one obligation per impacted record");
    assert_eq!(obligations[0]["object_id"], "billing.credits.consume");
    assert_eq!(obligations[1]["object_id"], "billing.credits.use-ledger");

    // --- V6.5.5: the verified api surfaces for its schema path ---
    let api_impacted = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "impacted-by",
            "openapi/billing.yaml",
            "--artifact",
            graph_arg,
            "--format",
            "json",
        ])
        .output()
        .expect("adoc impacted-by runs");
    assert!(
        api_impacted.status.success(),
        "impacted-by must exit 0 for the api schema path\nstderr:\n{}",
        String::from_utf8_lossy(&api_impacted.stderr)
    );
    let api_envelope: Value =
        serde_json::from_slice(&api_impacted.stdout).expect("impacted-by stdout is JSON");
    let api_records = api_envelope["impacted"].as_array().expect("impacted array");
    assert_eq!(
        api_records.len(),
        1,
        "expected exactly the verified api impacted: {api_records:#?}"
    );
    assert_eq!(api_records[0]["id"], "billing.consume-credit");
    assert_eq!(api_records[0]["kind"], "api");
    assert_eq!(api_records[0]["status"], "verified");

    // --- scope negative: the constraint declaring this path is not a
    // verified subject, so the impacted set is empty and the exit stays 0 ---
    let negative = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "impacted-by",
            "crates/auth/src/session.rs",
            "--artifact",
            graph_arg,
            "--format",
            "json",
        ])
        .output()
        .expect("adoc impacted-by runs");
    assert!(
        negative.status.success(),
        "empty findings are still exit 0\nstderr:\n{}",
        String::from_utf8_lossy(&negative.stderr)
    );
    let negative_env: Value =
        serde_json::from_slice(&negative.stdout).expect("impacted-by stdout is JSON");
    assert_eq!(negative_env["impacted"], serde_json::json!([]));
    assert_eq!(negative_env["proof_obligations"], serde_json::json!([]));
}
