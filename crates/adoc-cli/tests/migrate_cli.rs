//! V8.1.1 acceptance for `adoc migrate` (ADR-0043): dry-run default over the
//! Markdown Pilot, `--write` in a git-committed tempdir copy, the
//! committed-clean refusal, `--force`, and quarantine-code visibility.
//!
//! Every test operates on a tempdir copy of `examples/markdown-pilot/` —
//! never the checked-in example, which retrieval fixtures pin.

mod support;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use support::{TestWorkspace, adoc_command, copy_tree, stderr, stdout};

const PILOT_MD_COUNT: usize = 15;
const PILOT_PRE_EXISTING_ADOC: [&str; 2] = [
    "knowledge/billing-claims.adoc",
    "knowledge/billing-decisions.adoc",
];

fn markdown_pilot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/markdown-pilot")
}

fn pilot_copy(name: &str) -> TestWorkspace {
    let workspace = TestWorkspace::new(name);
    copy_tree(&markdown_pilot_dir(), &workspace.root);
    workspace
}

fn run_git(workspace: &TestWorkspace, args: &[&str]) {
    let mut command = Command::new("git");
    command.args(args).current_dir(&workspace.root);
    // Strip inherited GIT_* variables so a suite run from inside a
    // pre-commit hook (prek) cannot leak the outer repository into the
    // fixture (the diff_cli.rs precedent).
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

fn committed_pilot_copy(name: &str) -> TestWorkspace {
    let workspace = pilot_copy(name);
    run_git(&workspace, &["init", "--initial-branch=main"]);
    run_git(&workspace, &["config", "user.email", "test@adoc.dev"]);
    run_git(&workspace, &["config", "user.name", "adoc tests"]);
    run_git(&workspace, &["config", "commit.gpgsign", "false"]);
    run_git(&workspace, &["add", "-A"]);
    run_git(&workspace, &["commit", "-m", "pilot corpus"]);
    workspace
}

fn collect_files(root: &Path, extension: &str) -> Vec<PathBuf> {
    fn walk(dir: &Path, extension: &str, found: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).expect("directory is readable") {
            let entry = entry.expect("entry is readable");
            let path = entry.path();
            if path.is_dir() {
                if entry.file_name() != ".git" {
                    walk(&path, extension, found);
                }
            } else if path.extension().and_then(|ext| ext.to_str()) == Some(extension) {
                found.push(path);
            }
        }
    }
    let mut found = Vec::new();
    walk(root, extension, &mut found);
    found.sort();
    found
}

fn tree_contents(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(dir: &Path, tree: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(dir).expect("directory is readable") {
            let entry = entry.expect("entry is readable");
            let path = entry.path();
            if path.is_dir() {
                if entry.file_name() != ".git" {
                    walk(&path, tree);
                }
            } else {
                tree.insert(path.clone(), fs::read(&path).expect("file is readable"));
            }
        }
    }
    let mut tree = BTreeMap::new();
    walk(root, &mut tree);
    tree
}

#[test]
fn dry_run_lists_every_md_file_and_writes_nothing() {
    let workspace = pilot_copy("migrate-cli-dry-run");
    let before = tree_contents(&workspace.root);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["migrate", "."])
        .output()
        .expect("adoc migrate should run");

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let text = stdout(&output);
    for source in collect_files(&workspace.root, "md") {
        let relative = source
            .strip_prefix(&workspace.root)
            .expect("source is under the workspace");
        assert!(
            text.contains(&relative.display().to_string()),
            "dry-run must list {}:\n{text}",
            relative.display()
        );
    }
    assert!(
        text.contains("would migrate"),
        "dry-run must speak in the conditional:\n{text}"
    );
    assert_eq!(
        tree_contents(&workspace.root),
        before,
        "dry-run must be byte-neutral on the tree"
    );
}

#[test]
fn dry_run_shows_all_three_quarantine_codes() {
    let workspace = pilot_copy("migrate-cli-codes");

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["migrate", "."])
        .output()
        .expect("adoc migrate should run");

    assert_eq!(output.status.code(), Some(0));
    let text = stdout(&output);
    for code in [
        "migrate.raw_html_quarantined",
        "migrate.unrecognized_extension",
        "migrate.broken_link",
    ] {
        assert!(
            text.contains(code),
            "pilot dry-run must surface {code}:\n{text}"
        );
    }
}

#[test]
fn write_in_committed_copy_migrates_and_builds_clean() {
    let workspace = committed_pilot_copy("migrate-cli-write");
    let pre_existing: Vec<Vec<u8>> = PILOT_PRE_EXISTING_ADOC
        .iter()
        .map(|relative| fs::read(workspace.root.join(relative)).expect("pre-existing .adoc"))
        .collect();

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["migrate", ".", "--write"])
        .output()
        .expect("adoc migrate --write should run");

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(stdout(&output).contains("migrated"), "{}", stdout(&output));
    assert!(
        collect_files(&workspace.root, "md").is_empty(),
        "--write must remove every source .md"
    );
    assert_eq!(
        collect_files(&workspace.root, "adoc").len(),
        PILOT_MD_COUNT + PILOT_PRE_EXISTING_ADOC.len(),
        "every .md must have produced one .adoc beside the pre-existing pages"
    );
    for (relative, before) in PILOT_PRE_EXISTING_ADOC.iter().zip(pre_existing) {
        assert_eq!(
            fs::read(workspace.root.join(relative)).expect("pre-existing .adoc"),
            before,
            "{relative} must be byte-untouched"
        );
    }

    let build = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", ".", "--no-embeddings", "--out", "dist"])
        .output()
        .expect("adoc build should run");
    assert_eq!(
        build.status.code(),
        Some(0),
        "migrated tree must build clean\nstdout:\n{}\nstderr:\n{}",
        stdout(&build),
        stderr(&build)
    );
}

#[test]
fn write_refuses_a_dirty_source_and_removes_nothing() {
    let workspace = committed_pilot_copy("migrate-cli-dirty");
    let dirty = workspace.root.join("api/errors.md");
    let mut text = fs::read_to_string(&dirty).expect("dirty source is readable");
    text.push_str("\nUncommitted trailing note.\n");
    fs::write(&dirty, text).expect("dirty edit can be written");
    let before = tree_contents(&workspace.root);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["migrate", ".", "--write"])
        .output()
        .expect("adoc migrate --write should run");

    assert_eq!(output.status.code(), Some(1), "{}", stdout(&output));
    assert!(
        stdout(&output).contains("migrate.source_not_committed"),
        "{}",
        stdout(&output)
    );
    assert_eq!(
        tree_contents(&workspace.root),
        before,
        "a refused run must write and remove nothing (all-or-nothing)"
    );
}

#[test]
fn write_refuses_outside_a_git_repository() {
    let workspace = pilot_copy("migrate-cli-norepo");
    let before = tree_contents(&workspace.root);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["migrate", ".", "--write"])
        .output()
        .expect("adoc migrate --write should run");

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stdout(&output).contains("migrate.source_not_committed"),
        "{}",
        stdout(&output)
    );
    assert_eq!(tree_contents(&workspace.root), before);
}

#[test]
fn write_force_bypasses_the_committed_clean_refusal() {
    let workspace = committed_pilot_copy("migrate-cli-force");
    let dirty = workspace.root.join("api/errors.md");
    let mut text = fs::read_to_string(&dirty).expect("dirty source is readable");
    text.push_str("\nUncommitted trailing note.\n");
    fs::write(&dirty, text).expect("dirty edit can be written");

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["migrate", ".", "--write", "--force"])
        .output()
        .expect("adoc migrate --write --force should run");

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(collect_files(&workspace.root, "md").is_empty());
}

#[test]
fn json_format_is_rejected_until_the_report_slice() {
    let workspace = pilot_copy("migrate-cli-json");

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["migrate", ".", "--format", "json"])
        .output()
        .expect("adoc migrate should run");

    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("error[cli.format]"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn markdown_format_is_rejected_like_other_non_pr_commands() {
    let workspace = pilot_copy("migrate-cli-markdown");

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["migrate", ".", "--format", "markdown"])
        .output()
        .expect("adoc migrate should run");

    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("error[cli.format]"),
        "{}",
        stderr(&output)
    );
}
