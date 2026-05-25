//! Shared test helpers for V3.6 MCP review tests.
//!
//! Builds a 2-commit git fixture matching the V3.3/V3.4 acceptance scenario:
//! a verified claim (`billing.refunds`) whose body changes and a second
//! verified claim (`billing.holds-policy`) whose head delta exercises every
//! other `FieldChange` variant. The base ref `main` points at the initial
//! commit; the head ref `feature` carries the deltas. The fixture also
//! touches `crates/billing/src/refund.rs`, which is declared in
//! `billing.refunds.impacts`, so impact analysis and proof obligations
//! fire on review.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static FIXTURE_COUNTER: AtomicU64 = AtomicU64::new(0);

pub const BASE_BILLING_ADOC: &str = concat!(
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
);

pub const HEAD_BILLING_ADOC: &str = concat!(
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
);

pub const CONFIG_YAML: &str = "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dir: dist\nembeddings:\n  provider: deterministic\n";

pub const BASE_REFUND_SRC: &str = "// initial stub\n";
pub const HEAD_REFUND_SRC: &str = "// updated implementation\n";

/// RAII handle for a temp fixture directory; removes the directory on drop.
pub struct FixtureWorkspace {
    pub root: PathBuf,
}

impl FixtureWorkspace {
    pub fn new(name: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock is after epoch")
            .as_nanos();
        let counter = FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "adoc-mcp-{name}-{}-{counter}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("fixture root creates");
        Self { root }
    }

    pub fn write(&self, relative_path: &str, contents: &str) -> PathBuf {
        let path = self.root.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent directory creates");
        }
        fs::write(&path, contents).expect("fixture file writes");
        path
    }
}

impl Drop for FixtureWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Build a 2-commit V3.6 review fixture under a fresh tempdir. Mirrors the
/// V3.3/V3.4 acceptance fixture used in `crates/adoc-cli/tests/review_cli.rs`.
pub fn build_v3_review_fixture(name: &str) -> FixtureWorkspace {
    let workspace = FixtureWorkspace::new(name);
    workspace.write("agentdoc.config.yaml", CONFIG_YAML);
    run_git(&workspace.root, &["init", "--initial-branch=main"]);
    run_git(&workspace.root, &["config", "user.email", "test@adoc.dev"]);
    run_git(&workspace.root, &["config", "user.name", "adoc tests"]);
    run_git(&workspace.root, &["config", "commit.gpgsign", "false"]);

    workspace.write("docs/billing.adoc", BASE_BILLING_ADOC);
    workspace.write("crates/billing/src/refund.rs", BASE_REFUND_SRC);
    run_git(&workspace.root, &["add", "-A"]);
    run_git(&workspace.root, &["commit", "-m", "base"]);

    run_git(&workspace.root, &["checkout", "-b", "feature"]);
    workspace.write("docs/billing.adoc", HEAD_BILLING_ADOC);
    workspace.write("crates/billing/src/refund.rs", HEAD_REFUND_SRC);
    run_git(&workspace.root, &["add", "-A"]);
    run_git(&workspace.root, &["commit", "-m", "head"]);

    workspace
}

pub fn run_git(repo_root: &Path, args: &[&str]) {
    let mut command = Command::new("git");
    command.arg("-C").arg(repo_root).args(args);
    // Strip inherited GIT_* env vars so fixtures stay isolated from any outer
    // git context (e.g. prek-driven pre-commit hooks).
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
        .unwrap_or_else(|error| panic!("spawn `git {args:?}`: {error}"));
    assert!(
        output.status.success(),
        "git {args:?} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
