mod support;

use std::fs;
use std::process::Command;

use support::{TestWorkspace, adoc_command, stderr, stdout};

const BASE_BILLING_ADOC: &str = concat!(
    "# Billing @doc(team.billing)\n",
    "\n",
    "::claim billing.credits\n",
    "status: draft\n",
    "--\n",
    "Credits apply after payment.\n",
    "::\n",
    "\n",
    "::claim billing.refunds\n",
    "status: draft\n",
    "--\n",
    "Refunds require audit review.\n",
    "::\n",
    "\n",
    "::claim billing.legacy-credits\n",
    "status: draft\n",
    "--\n",
    "Legacy credits behaviour, slated for removal.\n",
    "::\n",
);

const HEAD_BILLING_ADOC: &str = concat!(
    "# Billing @doc(team.billing)\n",
    "\n",
    "::claim billing.credits\n",
    "status: draft\n",
    "--\n",
    "Credits apply after ledger commit.\n",
    "::\n",
    "\n",
    "::claim billing.refunds\n",
    "status: draft\n",
    "--\n",
    "Refunds require audit review.\n",
    "::\n",
    "\n",
    "::claim billing.holds\n",
    "status: draft\n",
    "--\n",
    "Holds delay disbursement pending review.\n",
    "::\n",
);

fn run_git(workspace: &TestWorkspace, args: &[&str]) {
    run_git_in(&workspace.root, args);
}

fn run_git_in(dir: &std::path::Path, args: &[&str]) {
    let mut command = Command::new("git");
    command.args(args).current_dir(dir);
    // Inherited GIT_* environment variables (set by tools like prek when
    // running the test suite from inside a pre-commit hook) make git read
    // the outer repository's config instead of the per-fixture tempdir's
    // — which causes `git init` to lock the parent repo's config file.
    // Strip them so each fixture gets a clean, isolated git context.
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

/// Build a 2-commit git fixture matching V3.1's acceptance scenario:
/// - commit 1 on `main` writes `docs/billing.adoc` with three claims
///   (`billing.credits`, `billing.refunds`, `billing.legacy-credits`).
/// - commit 2 on `feature` modifies `billing.credits`, removes
///   `billing.legacy-credits`, and adds `billing.holds`.
///
/// The returned workspace has `main` pointing at the base commit and the
/// working tree at the head commit, so `adoc diff main` exercises the
/// "base=ref, head=workdir" path.
fn build_two_commit_fixture(name: &str) -> TestWorkspace {
    let workspace = TestWorkspace::new(name);
    run_git(&workspace, &["init", "--initial-branch=main"]);
    run_git(&workspace, &["config", "user.email", "test@adoc.dev"]);
    run_git(&workspace, &["config", "user.name", "adoc tests"]);
    run_git(&workspace, &["config", "commit.gpgsign", "false"]);

    workspace.write(
        "agentdoc.config.yaml",
        "version: 1\nmode: strict\ndocs_path: docs\n",
    );
    workspace.write("docs/billing.adoc", BASE_BILLING_ADOC);
    run_git(&workspace, &["add", "-A"]);
    run_git(&workspace, &["commit", "-m", "base"]);

    run_git(&workspace, &["checkout", "-b", "feature"]);
    workspace.write("docs/billing.adoc", HEAD_BILLING_ADOC);
    run_git(&workspace, &["add", "-A"]);
    run_git(&workspace, &["commit", "-m", "head"]);

    workspace
}

#[test]
fn diff_main_plain_lists_created_deleted_changed_ids() {
    let workspace = build_two_commit_fixture("diff-plain");

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["diff", "main", "--format", "plain"])
        .output()
        .expect("adoc diff runs");

    assert!(
        output.status.success(),
        "expected adoc diff to exit zero\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let stdout = stdout(&output);
    assert!(stdout.contains("Diff: 1 created, 1 deleted, 1 changed"));
    assert!(stdout.contains("Created:"));
    assert!(stdout.contains("billing.holds"));
    assert!(stdout.contains("Deleted:"));
    assert!(stdout.contains("billing.legacy-credits"));
    assert!(stdout.contains("Changed:"));
    assert!(stdout.contains("billing.credits"));
    // V3.2: the Changed section now nests a per-FieldChange line under each
    // entry. The fixture's billing.credits change is a body-only edit, so
    // the only line emitted is `body: changed`.
    assert!(
        stdout.contains("      body: changed"),
        "expected plain output to include `body: changed` under the billing.credits Changed entry; got:\n{stdout}"
    );
}

#[test]
fn diff_main_styled_exits_zero_for_well_formed_diff() {
    let workspace = build_two_commit_fixture("diff-styled");

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["diff", "main", "--format", "styled"])
        .output()
        .expect("adoc diff runs");

    assert!(
        output.status.success(),
        "expected styled diff to exit zero\nstderr:\n{}",
        stderr(&output)
    );
    // Plain content sanity: even with ANSI codes interleaved, the IDs and
    // section labels still appear verbatim somewhere in stdout.
    let stdout = stdout(&output);
    assert!(stdout.contains("billing.credits"));
    assert!(stdout.contains("billing.holds"));
    assert!(stdout.contains("billing.legacy-credits"));
}

#[test]
fn diff_unresolvable_ref_exits_nonzero_with_actionable_stderr() {
    let workspace = build_two_commit_fixture("diff-bad-ref");

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["diff", "definitely-not-a-real-ref"])
        .output()
        .expect("adoc diff runs");

    assert!(
        !output.status.success(),
        "expected unresolvable ref to exit non-zero\nstdout:\n{}",
        stdout(&output)
    );
    let stderr = stderr(&output);
    assert!(stderr.contains("review"));
    assert!(stderr.contains("definitely-not-a-real-ref"));
}

#[test]
fn diff_names_a_missing_head_snapshot_config() {
    let workspace = build_two_commit_fixture("diff-no-config");
    fs::remove_file(workspace.root.join("agentdoc.config.yaml")).expect("remove config");

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["diff", "main"])
        .output()
        .expect("adoc diff runs");

    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("head snapshot configuration failed"),
        "stderr: {}",
        stderr(&output)
    );
}

#[test]
fn diff_main_json_envelope_matches_prepared_changes() {
    let workspace = build_two_commit_fixture("diff-json");

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["diff", "main", "--format", "json"])
        .output()
        .expect("adoc diff runs");

    assert!(
        output.status.success(),
        "expected adoc diff to exit zero\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );

    let value: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("adoc diff --format json must emit a JSON envelope on stdout");

    assert_eq!(value["schema_version"], "adoc.diff.v0");

    let created = value["created"].as_array().expect("created is an array");
    assert_eq!(
        created.len(),
        1,
        "expected exactly one created object, got: {created:#?}"
    );
    assert_eq!(created[0]["id"], "billing.holds");
    assert!(
        created[0]["content_hash"]
            .as_str()
            .is_some_and(|hash| hash.starts_with("sha256:")),
        "created entry must carry a sha256-prefixed content_hash"
    );

    let deleted = value["deleted"].as_array().expect("deleted is an array");
    assert_eq!(
        deleted.len(),
        1,
        "expected exactly one deleted object, got: {deleted:#?}"
    );
    assert_eq!(deleted[0]["id"], "billing.legacy-credits");

    let changed = value["changed"].as_array().expect("changed is an array");
    assert_eq!(
        changed.len(),
        1,
        "expected exactly one changed object, got: {changed:#?}"
    );
    let entry = &changed[0];
    assert_eq!(entry["id"], "billing.credits");
    let base_hash = entry["base"]["content_hash"]
        .as_str()
        .expect("base content_hash present on changed entry");
    let head_hash = entry["head"]["content_hash"]
        .as_str()
        .expect("head content_hash present on changed entry");
    assert_ne!(
        base_hash, head_hash,
        "changed entry must carry different base/head content_hashes"
    );
    assert!(base_hash.starts_with("sha256:"));
    assert!(head_hash.starts_with("sha256:"));

    // V3.2 acceptance: the body-only change on billing.credits in the
    // fixture must project to exactly one FieldChange of type "body" with
    // the expected before/after strings.
    let field_changes = entry["field_changes"]
        .as_array()
        .expect("field_changes array present on a body-changed entry");
    assert_eq!(
        field_changes.len(),
        1,
        "expected exactly one field_change for body-only edit, got: {field_changes:#?}"
    );
    let body_change = &field_changes[0];
    assert_eq!(body_change["type"], "body");
    assert_eq!(body_change["before"], "Credits apply after payment.");
    assert_eq!(body_change["after"], "Credits apply after ledger commit.");
}

// E1.1.T4 (ADR-0058): `adoc.diff.v0` change detection re-derives from the v6
// governed-meaning hash — position-only moves are invisible to diff, a
// one-word body edit is exactly one change, and the hash is byte-identical
// across clones and the review worktree.

/// The Knowledge Object block moved between files in the position-move
/// fixture. Byte-identical on both sides of the move — only its file and
/// position change.
const MOVED_REFUNDS_BLOCK: &str = concat!(
    "::claim billing.refunds\n",
    "status: draft\n",
    "--\n",
    "Refunds require audit review.\n",
    "::\n",
);

const MOVE_CREDITS_PAGE: &str = concat!(
    "# Billing @doc(team.billing)\n",
    "\n",
    "::claim billing.credits\n",
    "status: draft\n",
    "--\n",
    "Credits apply after payment.\n",
    "::\n",
);

const MOVE_HOLDS_PAGE: &str = concat!(
    "# Payments @doc(team.payments)\n",
    "\n",
    "::claim payments.holds\n",
    "status: draft\n",
    "--\n",
    "Holds delay disbursement pending review.\n",
    "::\n",
);

/// Two commits differing only by the position of `billing.refunds`: on
/// `main` it closes docs/billing.adoc, on `feature` the byte-identical
/// block closes docs/payments.adoc instead (different file, different page,
/// different line).
fn build_position_move_fixture(name: &str) -> TestWorkspace {
    let workspace = TestWorkspace::new(name);
    run_git(&workspace, &["init", "--initial-branch=main"]);
    run_git(&workspace, &["config", "user.email", "test@adoc.dev"]);
    run_git(&workspace, &["config", "user.name", "adoc tests"]);
    run_git(&workspace, &["config", "commit.gpgsign", "false"]);

    workspace.write(
        "agentdoc.config.yaml",
        "version: 1\nmode: strict\ndocs_path: docs\n",
    );
    workspace.write(
        "docs/billing.adoc",
        &format!("{MOVE_CREDITS_PAGE}\n{MOVED_REFUNDS_BLOCK}"),
    );
    workspace.write("docs/payments.adoc", MOVE_HOLDS_PAGE);
    run_git(&workspace, &["add", "-A"]);
    run_git(&workspace, &["commit", "-m", "base"]);

    run_git(&workspace, &["checkout", "-b", "feature"]);
    workspace.write("docs/billing.adoc", MOVE_CREDITS_PAGE);
    workspace.write(
        "docs/payments.adoc",
        &format!("{MOVE_HOLDS_PAGE}\n{MOVED_REFUNDS_BLOCK}"),
    );
    run_git(&workspace, &["add", "-A"]);
    run_git(&workspace, &["commit", "-m", "move refunds claim"]);

    workspace
}

fn diff_json(dir: &std::path::Path) -> serde_json::Value {
    let output = adoc_command()
        .current_dir(dir)
        .args(["diff", "main", "--format", "json"])
        .output()
        .expect("adoc diff runs");
    assert!(
        output.status.success(),
        "expected adoc diff to exit zero\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    serde_json::from_slice(&output.stdout).expect("adoc diff --format json emits JSON")
}

/// Acceptance: two snapshots differing only by object position → zero diff
/// changes (`changed = hash differs` re-derives from the v6 hash, which
/// excludes placement).
#[test]
fn diff_position_only_move_yields_zero_changes() {
    let workspace = build_position_move_fixture("diff-move-zero");

    let value = diff_json(&workspace.root);

    assert_eq!(value["schema_version"], "adoc.diff.v0");
    for section in ["created", "deleted", "changed"] {
        let entries = value[section].as_array().expect("section is an array");
        assert!(
            entries.is_empty(),
            "position-only move must yield an empty `{section}` section, got: {entries:#?}"
        );
    }
}

/// Acceptance: one-word body edit → exactly one change.
#[test]
fn diff_one_word_body_edit_yields_exactly_one_change() {
    let workspace = build_position_move_fixture("diff-one-word");
    run_git(&workspace, &["checkout", "main"]);
    run_git(&workspace, &["checkout", "-b", "edit"]);
    workspace.write(
        "docs/billing.adoc",
        &format!("{MOVE_CREDITS_PAGE}\n{MOVED_REFUNDS_BLOCK}").replace(
            "Credits apply after payment.",
            "Credits apply after settlement.",
        ),
    );
    run_git(&workspace, &["add", "-A"]);
    run_git(&workspace, &["commit", "-m", "one-word edit"]);

    let value = diff_json(&workspace.root);

    assert!(value["created"].as_array().expect("array").is_empty());
    assert!(value["deleted"].as_array().expect("array").is_empty());
    let changed = value["changed"].as_array().expect("array");
    assert_eq!(
        changed.len(),
        1,
        "one-word body edit must be exactly one change, got: {changed:#?}"
    );
    assert_eq!(changed[0]["id"], "billing.credits");
}

fn build_dist(dir: &std::path::Path) {
    let dist = dir.join("dist");
    let output = adoc_command()
        .current_dir(dir)
        .args(["build", "--out", dist.to_str().expect("dist path is UTF-8")])
        .output()
        .expect("adoc build runs");
    assert!(
        output.status.success(),
        "expected adoc build to exit zero\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

fn node_content_hash(dir: &std::path::Path, id: &str) -> String {
    let graph: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(dir.join("dist/docs.graph.json")).expect("graph artifact is readable"),
    )
    .expect("graph artifact parses");
    graph["nodes"]
        .as_array()
        .expect("nodes array")
        .iter()
        .find(|node| node["id"] == id && node["type"] == "knowledge_object")
        .and_then(|node| node["content_hash"].as_str())
        .expect("target node with content_hash")
        .to_string()
}

/// Acceptance: two clones + a review worktree yield byte-identical
/// `content_hash` for position-only moves — clone A builds the base
/// position, clone B the moved position, from different filesystem roots,
/// and clone B's `adoc diff main` (review worktree snapshots) agrees.
#[test]
fn diff_position_only_move_hashes_are_byte_identical_across_clones_and_worktree() {
    let origin = build_position_move_fixture("diff-clone-origin");
    let clones = TestWorkspace::new("diff-clones");
    let clone_a = clones.root.join("a");
    let clone_b = clones.root.join("b");
    let origin_path = origin.root.to_str().expect("origin path is UTF-8");
    run_git_in(
        &clones.root,
        &["clone", origin_path, clone_a.to_str().expect("UTF-8")],
    );
    run_git_in(
        &clones.root,
        &["clone", origin_path, clone_b.to_str().expect("UTF-8")],
    );
    // Clone A checks out the base position; clone B stays on the moved one.
    run_git_in(&clone_a, &["checkout", "main"]);
    run_git_in(&clone_b, &["branch", "main", "origin/main"]);

    build_dist(&clone_a);
    build_dist(&clone_b);
    assert_eq!(
        node_content_hash(&clone_a, "billing.refunds"),
        node_content_hash(&clone_b, "billing.refunds"),
        "two clones must yield byte-identical content_hash across a position-only move"
    );

    // Review worktree: diff compiles `main` in a snapshot worktree — the
    // re-derived hashes must agree, so the move is invisible.
    let value = diff_json(&clone_b);
    for section in ["created", "deleted", "changed"] {
        assert!(
            value[section].as_array().expect("array").is_empty(),
            "review-worktree diff must see zero `{section}` entries for the move"
        );
    }
}
