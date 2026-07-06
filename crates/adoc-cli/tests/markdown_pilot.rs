//! V4.4 end-to-end test for the Markdown Pilot.
//!
//! Validates `examples/markdown-pilot/` against the full V4 acceptance
//! contract from `docs/V4-DESIGN.md`:
//!
//! - V4.1: raw HTML quarantined, unsafe link/image schemes dropped, safe
//!   HTML rendered, prose-only graph nodes for `.md` source.
//! - V4.2: GFM extensions rendered (tables, task lists, strikethrough,
//!   footnotes); MDX/Pandoc/math/attribute-block constructs classified as
//!   `compat.unknown_extension` and rendered as escaped code.
//! - V4.3: `adoc search` over a `.md`-only project emits the migration
//!   hint inside the existing `adoc.retrieval.v1` envelope.
//! - V4.4: full-pilot acceptance over 15 `.md` + 2 `.adoc` files at
//!   `examples/markdown-pilot/`, exact-match diagnostic and node counts,
//!   plus mixed-mode `adoc diff` / `adoc review` behavior against a git
//!   fixture carrying both file kinds.
//!
//! Mode boundary (ADR-0022): `.md` files run under Compatibility Mode,
//! `.adoc` files under Strict Mode, in one combined pipeline.

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

const PILOT_PATH: &str = "examples/markdown-pilot/";

/// V4.1 + V4.2 + V4.4 acceptance: `adoc check` over the full pilot exits
/// 0 with the exact compat-warning budget published in
/// `docs/markdown-pilot.md`.
///
/// Diagnostic budget:
/// - 2 × `compat.raw_html_quarantined` (one `<div>` + one `<script>` in
///   `runbooks/incident-response.md`)
/// - 1 × `compat.unsafe_link_dropped` (`javascript:` link in
///   `runbooks/on-call-rotation.md`)
/// - 1 × `compat.unsafe_image_src_dropped` (`data:` image src in
///   `tutorials/deploying.md`)
/// - 4 × `compat.unknown_extension` (MDX + Pandoc in
///   `tutorials/troubleshooting.md`, display math in
///   `reference/glossary-notes.md`, attribute block in
///   `reference/architecture-notes.md`)
#[test]
fn markdown_pilot_check_emits_exact_diagnostic_budget() {
    let repo_root = repo_root();
    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&repo_root)
        .args(["check", PILOT_PATH])
        .output()
        .expect("adoc check runs");

    assert!(
        output.status.success(),
        "pilot must check with exit code 0 (compat warnings do not fail check)\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let raw_html_count = stdout
        .matches("warning[compat.raw_html_quarantined]")
        .count();
    let unsafe_link_count = stdout
        .matches("warning[compat.unsafe_link_dropped]")
        .count();
    let unsafe_image_count = stdout
        .matches("warning[compat.unsafe_image_src_dropped]")
        .count();
    let unknown_extension_count = stdout.matches("warning[compat.unknown_extension]").count();
    let error_count = stdout.matches("error[").count();

    assert_eq!(
        raw_html_count, 2,
        "expected two compat.raw_html_quarantined warnings\nstdout:\n{stdout}"
    );
    assert_eq!(
        unsafe_link_count, 1,
        "expected one compat.unsafe_link_dropped warning\nstdout:\n{stdout}"
    );
    assert_eq!(
        unsafe_image_count, 1,
        "expected one compat.unsafe_image_src_dropped warning\nstdout:\n{stdout}"
    );
    assert_eq!(
        unknown_extension_count, 4,
        "expected four compat.unknown_extension warnings (MDX, Pandoc, display math, attribute block)\nstdout:\n{stdout}"
    );
    assert_eq!(
        error_count, 0,
        "pilot must produce zero errors (mixed-mode .adoc files are strict-valid)\nstdout:\n{stdout}"
    );
    assert!(
        stdout.contains("0 errors, 8 warnings"),
        "expected aggregate summary `0 errors, 8 warnings`\nstdout:\n{stdout}"
    );
}

/// V4.1 + V4.2 + V4.4 acceptance: `adoc build` over the full pilot
/// produces safe HTML and a graph artifact carrying both `.md` page
/// nodes and `.adoc` Knowledge Object nodes (mixed-mode dispatch per
/// ADR-0022).
#[test]
fn markdown_pilot_build_emits_safe_html_and_mixed_graph() {
    let repo_root = repo_root();
    let workspace = TestWorkspace::new("markdown-pilot-build");
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

    // --- HTML safety + GFM rendering ---
    let html =
        std::fs::read_to_string(output_directory.join("docs.html")).expect("pilot HTML is written");

    let quarantine_wrappers = html.matches(r#"<pre class="quarantined-html">"#).count();
    assert_eq!(
        quarantine_wrappers, 2,
        "expected two block-level quarantine wrappers (one per raw HTML block in incident-response.md)"
    );
    assert!(
        html.contains("&lt;div"),
        "raw <div> markup must be present as escaped text inside the quarantine wrapper"
    );
    assert!(
        !html.contains(
            r#"<script>
  // Legacy"#
        ),
        "raw <script> from the fixture must never reach the rendered HTML uninterpreted"
    );
    assert!(
        !html.contains(r#"href="javascript:"#),
        "no live javascript: href may appear in the rendered HTML"
    );
    assert!(
        !html.contains(r#"src="data:"#),
        "no live data: image src may appear in the rendered HTML"
    );

    assert!(
        html.contains(r#"<table class="adoc-table">"#),
        "expected GFM tables to render as <table> (auth.md scopes, on-call-rotation.md rotation)"
    );
    assert!(
        html.contains(r#"<input type="checkbox" disabled checked"#),
        "expected webhooks.md task list checked items to render as <input type=\"checkbox\">"
    );
    assert!(
        html.contains(r#"<del>"#),
        "expected GFM strikethrough (`X-RateLimit-Quota`, `legacy.unknown`) to render as <del>"
    );
    assert!(
        html.contains(r#"<aside class="adoc-footnote""#),
        "expected webhooks.md footnote definition to render as <aside>"
    );

    let unknown_block_wrappers = html.matches(r#"class="adoc-unknown-extension""#).count();
    assert!(
        unknown_block_wrappers >= 4,
        "expected at least four adoc-unknown-extension wrappers (MDX, Pandoc, display math, attribute block); got {unknown_block_wrappers}"
    );
    assert!(
        !html.contains("<DashboardWidget"),
        "MDX component tag must be quarantined as escaped text, never reach raw HTML"
    );

    // --- Graph artifact: mixed-mode page + KO nodes ---
    let graph_text = std::fs::read_to_string(output_directory.join("docs.graph.json"))
        .expect("pilot graph JSON is written");
    let graph: Value = serde_json::from_str(&graph_text).expect("graph JSON is valid");
    assert_eq!(graph["schema_version"], "adoc.graph.v4");

    let nodes = graph["nodes"]
        .as_array()
        .expect("graph JSON nodes is an array");

    let page_count = nodes.iter().filter(|node| node["type"] == "page").count();
    assert_eq!(
        page_count, 17,
        "expected seventeen page nodes (15 .md + 2 .adoc)"
    );

    let knowledge_objects: Vec<&Value> = nodes
        .iter()
        .filter(|node| node["type"] == "knowledge_object")
        .collect();
    assert_eq!(
        knowledge_objects.len(),
        6,
        "expected six Knowledge Object nodes (4 claims + 2 decisions from knowledge/*.adoc)"
    );

    let claim_count = knowledge_objects
        .iter()
        .filter(|object| object["kind"] == "claim")
        .count();
    let decision_count = knowledge_objects
        .iter()
        .filter(|object| object["kind"] == "decision")
        .count();
    assert_eq!(claim_count, 4, "expected four claim KOs");
    assert_eq!(decision_count, 2, "expected two decision KOs");

    let pilot_verified_claim_ids = [
        "billing.refunds.issue-credit",
        "billing.refunds.audit-required",
        "billing.settlement.posts-once",
        "billing.webhooks.signature-required",
    ];
    for expected_id in pilot_verified_claim_ids {
        let claim = knowledge_objects
            .iter()
            .find(|object| object["id"] == expected_id)
            .unwrap_or_else(|| panic!("expected verified claim {expected_id} in graph"));
        assert_eq!(
            claim["status"], "verified",
            "{expected_id} must be verified for diff/review obligation coverage"
        );
    }
}

/// V4.3 acceptance: `adoc search` over a `.md`-only project returns an
/// empty result set with exactly one
/// `retrieval.no_knowledge_objects_consider_migration` diagnostic riding
/// inside the existing `adoc.retrieval.v1.diagnostics[]` array. The full
/// mixed-mode pilot has Knowledge Objects, so the hint sub-test uses a
/// dedicated `.md`-only TestWorkspace fixture.
#[test]
fn markdown_pilot_search_emits_migration_hint_for_md_only_project() {
    let workspace = TestWorkspace::new("markdown-pilot-md-only-search");
    workspace.write(
        "pages/intro.md",
        "# Intro\n\nA short prose introduction with no Knowledge Objects.\n",
    );
    workspace.write(
        "pages/usage.md",
        "# Usage\n\nFollow the standard onboarding steps; no KOs here either.\n",
    );

    let output_directory = workspace.root.join("dist");
    // Build from the workspace root (not `pages/`) so the path-derived
    // page IDs include the `pages.` prefix and satisfy the two-segment
    // Object ID grammar (`pages.intro`, `pages.usage`).
    let build_output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .env("ADOC_TEST_EMBEDDING_PROVIDER", "deterministic")
        .args([
            "build",
            ".",
            "--out",
            output_directory
                .to_str()
                .expect("output directory path is utf-8"),
        ])
        .output()
        .expect("adoc build runs");
    assert!(
        build_output.status.success(),
        "search-hint test prerequisite (build) must succeed\nstderr:\n{}",
        String::from_utf8_lossy(&build_output.stderr)
    );

    let graph_artifact = output_directory.join("docs.graph.json");
    let search_output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "search",
            "anything",
            "--artifact",
            graph_artifact
                .to_str()
                .expect("graph artifact path is utf-8"),
            "--lexical",
            "--format",
            "json",
        ])
        .output()
        .expect("adoc search runs");

    assert!(
        search_output.status.success(),
        "search over a .md-only project must exit 0 (the hint is a warning)\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&search_output.stdout),
        String::from_utf8_lossy(&search_output.stderr)
    );

    let envelope: Value =
        serde_json::from_slice(&search_output.stdout).expect("search stdout is JSON");
    assert_eq!(envelope["schema_version"], "adoc.retrieval.v1");
    let records = envelope["records"].as_array().expect("records is an array");
    assert!(
        records.is_empty(),
        ".md-only project must return zero records, got {records:?}"
    );

    let diagnostics = envelope["diagnostics"]
        .as_array()
        .expect("diagnostics is an array");
    let migration_hints: Vec<&Value> = diagnostics
        .iter()
        .filter(|d| d["code"] == "retrieval.no_knowledge_objects_consider_migration")
        .collect();
    assert_eq!(
        migration_hints.len(),
        1,
        "expected exactly one migration-hint diagnostic, got diagnostics: {diagnostics:?}"
    );
    assert_eq!(migration_hints[0]["severity"], "warning");
    assert!(
        migration_hints[0]["message"]
            .as_str()
            .expect("diagnostic message is a string")
            .contains("Knowledge Objects"),
        "expected migration-hint message to mention Knowledge Objects, got {:?}",
        migration_hints[0]["message"]
    );
}

/// V4.4 acceptance: `adoc diff` and `adoc review` operate cleanly over a
/// mixed-mode pilot (one `.md` page carrying compat-warning constructs,
/// one `.adoc` file carrying a verified claim). A body change on the
/// verified claim must produce exactly one Changed Object Change in the
/// diff envelope and at least one proof obligation (re-verify) in the
/// review envelope. The `.md` sibling proves the V3 surface does not
/// trip on Compatibility-Mode pages — they contribute prose to the
/// graph but no Knowledge Objects, so diff/review treat them as inert.
#[test]
fn markdown_pilot_diff_and_review_handle_mixed_mode_change() {
    let workspace = TestWorkspace::new("markdown-pilot-mixed-diff-review");
    git_init_fixture(&workspace);

    let base_adoc = concat!(
        "# Pilot Billing @doc(pilot.billing.claims)\n",
        "\n",
        "::claim pilot.refunds.issue-credit\n",
        "status: verified\n",
        "owner: team-billing\n",
        "verified_at: 2026-05-12\n",
        "source: refund service production trace 2026-05-10\n",
        "test: cargo test refund_issue_credit_records_ledger_entry\n",
        "reviewed_by: qa-billing\n",
        "--\n",
        "Refund operators issue account credit only after the refund workflow writes its audit record.\n",
        "::\n",
    );
    let prose_md = concat!(
        "# Refund Reference\n",
        "\n",
        "Refunds settle within five business days. See the verified claim\n",
        "for the canonical audit-trail contract.\n",
    );
    workspace.write("knowledge/billing.adoc", base_adoc);
    workspace.write("api/refunds.md", prose_md);
    run_git(&workspace, &["add", "-A"]);
    run_git(&workspace, &["commit", "-m", "base pilot"]);

    run_git(&workspace, &["checkout", "-b", "feature"]);
    let head_adoc = base_adoc.replace(
        "Refund operators issue account credit only after the refund workflow writes its audit record.",
        "Refund operators issue account credit only after the refund workflow writes its audit record AND finance reviews the credit memo.",
    );
    workspace.write("knowledge/billing.adoc", &head_adoc);
    run_git(&workspace, &["add", "-A"]);
    run_git(&workspace, &["commit", "-m", "head: tighten refund body"]);

    // --- adoc diff: exactly one Changed entry, no created/deleted ---
    let diff_output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .env("ADOC_TEST_EMBEDDING_PROVIDER", "deterministic")
        .args(["diff", "main", "--format", "json"])
        .output()
        .expect("adoc diff runs");
    assert!(
        diff_output.status.success(),
        "adoc diff must succeed on mixed-mode fixture\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&diff_output.stdout),
        String::from_utf8_lossy(&diff_output.stderr)
    );
    let diff_envelope: Value =
        serde_json::from_slice(&diff_output.stdout).expect("diff stdout is JSON");
    assert_eq!(diff_envelope["schema_version"], "adoc.diff.v0");
    assert_eq!(
        diff_envelope["created"]
            .as_array()
            .expect("created array")
            .len(),
        0,
        "mixed-mode body change must not produce created entries"
    );
    assert_eq!(
        diff_envelope["deleted"]
            .as_array()
            .expect("deleted array")
            .len(),
        0,
        "mixed-mode body change must not produce deleted entries"
    );
    let changed = diff_envelope["changed"].as_array().expect("changed array");
    assert_eq!(
        changed.len(),
        1,
        "expected exactly one Changed entry for the body edit on pilot.refunds.issue-credit, got: {changed:#?}"
    );
    assert_eq!(changed[0]["id"], "pilot.refunds.issue-credit");
    let field_changes = changed[0]["field_changes"]
        .as_array()
        .expect("field_changes array present on body-edit");
    assert_eq!(
        field_changes.len(),
        1,
        "expected exactly one field_change (body) for the verified-claim body edit"
    );
    assert_eq!(field_changes[0]["type"], "body");

    // --- adoc review: well-formed envelope, body change triggers
    // re-verify obligation on the verified claim ---
    let review_output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .env("ADOC_TEST_EMBEDDING_PROVIDER", "deterministic")
        .args(["review", "main", "--format", "json"])
        .output()
        .expect("adoc review runs");
    assert!(
        review_output.status.success(),
        "adoc review must succeed on mixed-mode fixture\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&review_output.stdout),
        String::from_utf8_lossy(&review_output.stderr)
    );
    let review_envelope: Value =
        serde_json::from_slice(&review_output.stdout).expect("review stdout is JSON");
    assert_eq!(review_envelope["schema_version"], "adoc.review.v0");

    let obligations = review_envelope["proof_obligations"]
        .as_array()
        .expect("review envelope carries proof_obligations array");
    let reverify_obligations: Vec<&Value> = obligations
        .iter()
        .filter(|o| o["object_id"] == "pilot.refunds.issue-credit")
        .collect();
    assert!(
        !reverify_obligations.is_empty(),
        "expected at least one proof obligation on the modified verified claim, got: {obligations:#?}"
    );
}

/// Init a temp git repository for the mixed-mode diff/review fixture.
/// Mirrors the env-var scrubbing pattern from `diff_cli.rs` so prek and
/// other hooks running inside an outer git context do not lock the
/// per-fixture tempdir.
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
