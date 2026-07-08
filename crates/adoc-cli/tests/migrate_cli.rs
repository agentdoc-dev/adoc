//! V8.1.1 acceptance for `adoc migrate` (ADR-0043): dry-run default over the
//! Markdown Pilot, `--write` in a git-committed tempdir copy, the
//! committed-clean refusal, `--force`, and quarantine-code visibility.
//! V8.1.2 pins the `adoc.migrate.report.v0` envelope: exact pilot counts and
//! the counts-reconcile-with-diagnostics invariant (ADR-0043 §4).
//! V8.1.4 adds `--export` (the reverse run, symmetric write semantics), the
//! run-level typed-block refusal, and the additive `direction` field.
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

/// The pilot without its two typed pages: export refuses any run whose root
/// contains typed blocks, so export fixtures start prose-only. The typed
/// pages only cross-reference each other; the rest of the corpus stands.
fn prose_pilot_copy(name: &str) -> TestWorkspace {
    let workspace = pilot_copy(name);
    for page in PILOT_PRE_EXISTING_ADOC {
        fs::remove_file(workspace.root.join(page)).expect("typed pilot page can be removed");
    }
    workspace
}

/// A committed prose-only pilot, migrated with `--write` and re-committed —
/// the state a partner backing out of a migration would export from.
fn migrated_committed_prose_pilot(name: &str) -> TestWorkspace {
    let workspace = prose_pilot_copy(name);
    run_git(&workspace, &["init", "--initial-branch=main"]);
    run_git(&workspace, &["config", "user.email", "test@adoc.dev"]);
    run_git(&workspace, &["config", "user.name", "adoc tests"]);
    run_git(&workspace, &["config", "commit.gpgsign", "false"]);
    run_git(&workspace, &["add", "-A"]);
    run_git(&workspace, &["commit", "-m", "pilot corpus"]);
    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["migrate", ".", "--write"])
        .output()
        .expect("adoc migrate --write should run");
    assert_eq!(
        output.status.code(),
        Some(0),
        "fixture migration must succeed\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    run_git(&workspace, &["add", "-A"]);
    run_git(&workspace, &["commit", "-m", "migrated corpus"]);
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
fn json_format_emits_the_versioned_report_envelope() {
    let workspace = pilot_copy("migrate-cli-json");

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["migrate", ".", "--format", "json"])
        .output()
        .expect("adoc migrate should run");

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let value: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("stdout is a JSON envelope");
    assert_eq!(value["schema_version"], "adoc.migrate.report.v0");
    assert_eq!(value["direction"], "import");
}

fn migrate_json(workspace: &TestWorkspace, args: &[&str]) -> (Option<i32>, serde_json::Value) {
    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["migrate", "."])
        .args(args)
        .args(["--format", "json"])
        .output()
        .expect("adoc migrate should run");
    let value: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap_or_else(|error| {
        panic!("stdout is a JSON envelope ({error}):\n{}", stdout(&output))
    });
    (output.status.code(), value)
}

/// The exact §28.3 counts for the Markdown Pilot, pinned. A drift here means
/// the corpus or the migration rules changed — both must be deliberate.
#[test]
fn json_report_pins_exact_pilot_counts() {
    let workspace = pilot_copy("migrate-cli-pin-counts");

    let (code, value) = migrate_json(&workspace, &[]);

    assert_eq!(code, Some(0), "{value}");
    assert_eq!(
        value["counts"],
        serde_json::json!({
            "files_imported": PILOT_MD_COUNT,
            "pages_created": PILOT_MD_COUNT,
            "prose_blocks": 128,
            "raw_html_quarantined": 3,
            "broken_links": 5,
            "unrecognized_extensions": 18,
            "suggested_typed_blocks": 12,
        })
    );
    let files = value["files"].as_array().expect("files is an array");
    assert_eq!(files.len(), PILOT_MD_COUNT);
    assert!(
        files.iter().all(|file| file["written"] == false),
        "dry-run writes nothing: {value}"
    );
}

/// The ADR-0043 §4 acceptance invariant: every report count reconciles
/// one-to-one with an emitted diagnostic — the report never claims what the
/// diagnostics don't show, and never hides what they do (the `compat.*`
/// diagnostics belong to no bucket but still travel). The tally-by-code
/// semantic deliberately folds quarantines and drops into one bucket:
/// `unrecognized_extensions` counts fenced-verbatim constructs and diagnosed
/// drops (front matter, task-list checkboxes, empty parser artifacts) alike.
#[test]
fn every_report_count_reconciles_with_an_emitted_diagnostic() {
    let workspace = pilot_copy("migrate-cli-reconcile");

    let (code, value) = migrate_json(&workspace, &[]);

    assert_eq!(code, Some(0), "{value}");
    let diagnostics = value["diagnostics"].as_array().expect("diagnostics array");
    let tally = |wire_code: &str| {
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic["code"] == wire_code)
            .count() as u64
    };
    let counts = &value["counts"];
    assert_eq!(
        counts["raw_html_quarantined"].as_u64(),
        Some(tally("migrate.raw_html_quarantined"))
    );
    assert_eq!(
        counts["broken_links"].as_u64(),
        Some(tally("migrate.broken_link"))
    );
    assert_eq!(
        counts["unrecognized_extensions"].as_u64(),
        Some(tally("migrate.unrecognized_extension"))
    );
    let files = value["files"].as_array().expect("files array");
    assert_eq!(counts["files_imported"].as_u64(), Some(files.len() as u64));
    assert_eq!(counts["pages_created"], counts["files_imported"]);
    let prose_sum: u64 = files
        .iter()
        .map(|file| file["prose_blocks"].as_u64().expect("prose_blocks"))
        .sum();
    assert_eq!(counts["prose_blocks"].as_u64(), Some(prose_sum));
    // Suggestions are report records, not diagnostics; the count reconciles
    // against the envelope's own `suggestions` array (V8.1.3).
    let suggestions = value["suggestions"].as_array().expect("suggestions array");
    assert_eq!(
        counts["suggested_typed_blocks"].as_u64(),
        Some(suggestions.len() as u64)
    );
    assert_eq!(
        tally("compat.unknown_extension"),
        2,
        "unbucketed diagnostics still travel in the envelope: {value}"
    );
}

#[test]
fn write_report_marks_every_file_written() {
    let workspace = committed_pilot_copy("migrate-cli-write-json");

    let (code, value) = migrate_json(&workspace, &["--write"]);

    assert_eq!(code, Some(0), "{value}");
    let files = value["files"].as_array().expect("files array");
    assert_eq!(files.len(), PILOT_MD_COUNT);
    assert!(
        files.iter().all(|file| file["written"] == true),
        "--write marks every file written: {value}"
    );
    assert_eq!(value["counts"]["files_imported"], PILOT_MD_COUNT);
}

#[test]
fn refused_write_still_emits_the_envelope_under_json() {
    let workspace = committed_pilot_copy("migrate-cli-refusal-json");
    let dirty = workspace.root.join("api/errors.md");
    let mut text = fs::read_to_string(&dirty).expect("dirty source is readable");
    text.push_str("\nUncommitted trailing note.\n");
    fs::write(&dirty, text).expect("dirty edit can be written");
    let before = tree_contents(&workspace.root);

    let (code, value) = migrate_json(&workspace, &["--write"]);

    assert_eq!(code, Some(1), "{value}");
    assert_eq!(value["schema_version"], "adoc.migrate.report.v0");
    assert!(
        value["diagnostics"]
            .as_array()
            .expect("diagnostics array")
            .iter()
            .any(|diagnostic| diagnostic["code"] == "migrate.source_not_committed"),
        "{value}"
    );
    assert_eq!(
        tree_contents(&workspace.root),
        before,
        "a refused run must write and remove nothing"
    );
}

#[test]
fn empty_workspace_reports_zero_counts() {
    let workspace = TestWorkspace::new("migrate-cli-empty");

    let (code, value) = migrate_json(&workspace, &[]);

    assert_eq!(code, Some(0), "{value}");
    assert_eq!(
        value["counts"],
        serde_json::json!({
            "files_imported": 0,
            "pages_created": 0,
            "prose_blocks": 0,
            "raw_html_quarantined": 0,
            "broken_links": 0,
            "unrecognized_extensions": 0,
            "suggested_typed_blocks": 0,
        })
    );
    assert_eq!(value["files"], serde_json::json!([]));
    assert_eq!(value["suggested_next_steps"], serde_json::json!([]));
    assert_eq!(value["suggestions"], serde_json::json!([]));
    assert_eq!(value["direction"], "import");
}

#[test]
fn plain_output_prints_the_section_28_3_report_block() {
    let workspace = pilot_copy("migrate-cli-plain-report");

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["migrate", "."])
        .output()
        .expect("adoc migrate should run");

    assert_eq!(output.status.code(), Some(0));
    let text = stdout(&output);
    for line in [
        "Migration report",
        "Files imported: 15",
        "Pages created: 15",
        "Prose blocks: 128",
        "Raw HTML blocks quarantined: 3",
        "Broken links: 5",
        "Unrecognized extensions: 18",
        "Suggested typed blocks: 12",
        "Suggested next steps:",
        "Review the 12 typed-block suggestion(s); migration never auto-types",
        "Typed-block suggestions:",
        "runbooks/db-failover.md:23:1 procedure (numbered_step_list): Quiesce write traffic at the load balancer.",
        "would migrate",
    ] {
        assert!(
            text.contains(line),
            "plain output must contain {line:?}:\n{text}"
        );
    }
}

/// The V8.1.3 acceptance fixture: one TODO line and one numbered step list
/// yield exactly one `task` and one `procedure` suggestion with correct spans.
#[test]
fn suggestion_fixture_pins_one_task_and_one_procedure_record() {
    let workspace = TestWorkspace::new("migrate-cli-suggestion-fixture");
    workspace.write(
        "guides/notes.md",
        "# Notes\n\nTODO: rotate the signing key.\n\n1. Stop the writer.\n2. Promote the standby.\n",
    );

    let (code, value) = migrate_json(&workspace, &[]);

    assert_eq!(code, Some(0), "{value}");
    assert_eq!(value["counts"]["suggested_typed_blocks"], 2, "{value}");
    let suggestions = value["suggestions"].as_array().expect("suggestions array");
    assert_eq!(suggestions.len(), 2, "{value}");

    let task = &suggestions[0];
    assert_eq!(task["suggested_kind"], "task");
    assert_eq!(task["matched_rule"], "todo_line");
    assert_eq!(task["excerpt"], "TODO: rotate the signing key.");
    assert_eq!(task["span"]["start"]["line"], 3);
    assert_eq!(task["span"]["start"]["column"], 1);

    let procedure = &suggestions[1];
    assert_eq!(procedure["suggested_kind"], "procedure");
    assert_eq!(procedure["matched_rule"], "numbered_step_list");
    assert_eq!(procedure["excerpt"], "Stop the writer.");
    assert_eq!(procedure["span"]["start"]["line"], 5);
    assert_eq!(procedure["span"]["start"]["column"], 1);

    for suggestion in suggestions {
        let file = suggestion["span"]["file"].as_str().expect("span file");
        assert!(file.ends_with("notes.md"), "{suggestion}");
    }
}

/// The never-auto-typed property (§28.4, a V8 roadmap invariant): however
/// many suggestions the run emits, the migrated output contains zero typed
/// blocks — the human types the block. Asserted over the entire Markdown
/// Pilot, per-kind fence prefixes so the quarantined `:::warning` Pandoc
/// directive in tutorials/troubleshooting.md can never false-positive.
#[test]
fn migrated_pilot_output_never_contains_typed_blocks() {
    let workspace = committed_pilot_copy("migrate-cli-never-auto-typed");

    let (code, value) = migrate_json(&workspace, &["--write"]);

    assert_eq!(code, Some(0), "{value}");
    let suggested = value["counts"]["suggested_typed_blocks"]
        .as_u64()
        .expect("count");
    assert!(
        suggested > 0,
        "the property must not hold vacuously: {value}"
    );
    let migrated: Vec<PathBuf> = collect_files(&workspace.root, "adoc")
        .into_iter()
        .filter(|path| {
            !PILOT_PRE_EXISTING_ADOC
                .iter()
                .any(|pre| path.ends_with(pre))
        })
        .collect();
    assert_eq!(migrated.len(), PILOT_MD_COUNT);
    for path in migrated {
        let text = fs::read_to_string(&path).expect("migrated .adoc is readable");
        for kind in adoc_core::block_kind_names() {
            let fence = format!("::{kind} ");
            assert!(
                !text.lines().any(|line| line.starts_with(&fence)),
                "{} must contain zero typed blocks; found a `{fence}` fence",
                path.display()
            );
        }
    }
}

#[test]
fn export_dry_run_lists_every_adoc_source_and_writes_nothing() {
    let workspace = migrated_committed_prose_pilot("migrate-cli-export-dry-run");
    let before = tree_contents(&workspace.root);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["migrate", ".", "--export"])
        .output()
        .expect("adoc migrate --export should run");

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let text = stdout(&output);
    let sources = collect_files(&workspace.root, "adoc");
    assert_eq!(sources.len(), PILOT_MD_COUNT);
    for source in sources {
        let relative = source
            .strip_prefix(&workspace.root)
            .expect("source is under the workspace");
        assert!(
            text.contains(&relative.display().to_string()),
            "export dry-run must list {}:\n{text}",
            relative.display()
        );
    }
    assert!(
        text.contains("would export"),
        "export dry-run must speak in the conditional:\n{text}"
    );
    assert_eq!(
        tree_contents(&workspace.root),
        before,
        "dry-run must be byte-neutral on the tree"
    );
}

#[test]
fn export_dry_run_surfaces_unwrap_warning_codes() {
    let workspace = migrated_committed_prose_pilot("migrate-cli-export-codes");

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["migrate", ".", "--export"])
        .output()
        .expect("adoc migrate --export should run");

    assert_eq!(output.status.code(), Some(0));
    let text = stdout(&output);
    for code in [
        "migrate.raw_html_quarantined",
        "migrate.unrecognized_extension",
    ] {
        assert!(
            text.contains(code),
            "pilot export must surface {code} for fence unwraps:\n{text}"
        );
    }
}

#[test]
fn export_write_restores_markdown_and_removes_adoc_sources() {
    let workspace = migrated_committed_prose_pilot("migrate-cli-export-write");

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["migrate", ".", "--export", "--write"])
        .output()
        .expect("adoc migrate --export --write should run");

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(stdout(&output).contains("exported"), "{}", stdout(&output));
    assert!(
        collect_files(&workspace.root, "adoc").is_empty(),
        "--export --write must remove every source .adoc"
    );
    assert_eq!(collect_files(&workspace.root, "md").len(), PILOT_MD_COUNT);

    let build = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", ".", "--no-embeddings", "--out", "dist"])
        .output()
        .expect("adoc build should run");
    assert_eq!(
        build.status.code(),
        Some(0),
        "exported tree must build clean (Compatibility Mode)\nstdout:\n{}\nstderr:\n{}",
        stdout(&build),
        stderr(&build)
    );
}

#[test]
fn export_refuses_typed_block_pages_and_writes_nothing() {
    // The full pilot: its two typed knowledge pages survive migration
    // byte-untouched, so the migrated corpus still contains typed blocks.
    let workspace = committed_pilot_copy("migrate-cli-export-typed");
    let migrate = adoc_command()
        .current_dir(&workspace.root)
        .args(["migrate", ".", "--write"])
        .output()
        .expect("adoc migrate --write should run");
    assert_eq!(migrate.status.code(), Some(0), "{}", stdout(&migrate));
    run_git(&workspace, &["add", "-A"]);
    run_git(&workspace, &["commit", "-m", "migrated corpus"]);
    let before = tree_contents(&workspace.root);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["migrate", ".", "--export", "--write"])
        .output()
        .expect("adoc migrate --export --write should run");

    assert_eq!(output.status.code(), Some(1), "{}", stdout(&output));
    assert!(
        stdout(&output).contains("migrate.export_typed_blocks_present"),
        "{}",
        stdout(&output)
    );
    assert_eq!(
        tree_contents(&workspace.root),
        before,
        "a refused export must write and remove nothing (all-or-nothing)"
    );
}

#[test]
fn export_write_refuses_a_dirty_adoc_source() {
    let workspace = migrated_committed_prose_pilot("migrate-cli-export-dirty");
    let dirty = workspace.root.join("api/errors.adoc");
    let mut text = fs::read_to_string(&dirty).expect("dirty source is readable");
    text.push_str("\nUncommitted trailing note.\n");
    fs::write(&dirty, text).expect("dirty edit can be written");
    let before = tree_contents(&workspace.root);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["migrate", ".", "--export", "--write"])
        .output()
        .expect("adoc migrate --export --write should run");

    assert_eq!(output.status.code(), Some(1), "{}", stdout(&output));
    assert!(
        stdout(&output).contains("migrate.source_not_committed"),
        "{}",
        stdout(&output)
    );
    assert_eq!(tree_contents(&workspace.root), before);
}

#[test]
fn export_write_force_bypasses_the_committed_clean_refusal() {
    let workspace = migrated_committed_prose_pilot("migrate-cli-export-force");
    let dirty = workspace.root.join("api/errors.adoc");
    let mut text = fs::read_to_string(&dirty).expect("dirty source is readable");
    text.push_str("\nUncommitted trailing note.\n");
    fs::write(&dirty, text).expect("dirty edit can be written");

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["migrate", ".", "--export", "--write", "--force"])
        .output()
        .expect("adoc migrate --export --write --force should run");

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(collect_files(&workspace.root, "adoc").is_empty());
}

#[test]
fn export_json_envelope_carries_direction_and_zero_suggestions() {
    let workspace = migrated_committed_prose_pilot("migrate-cli-export-json");

    let (code, value) = migrate_json(&workspace, &["--export"]);

    assert_eq!(code, Some(0), "{value}");
    assert_eq!(value["schema_version"], "adoc.migrate.report.v0");
    assert_eq!(value["direction"], "export");
    assert_eq!(value["counts"]["suggested_typed_blocks"], 0);
    assert_eq!(value["suggestions"], serde_json::json!([]));
    let files = value["files"].as_array().expect("files array");
    assert_eq!(files.len(), PILOT_MD_COUNT);
    assert!(
        files.iter().all(|file| {
            file["written"] == false
                && file["target"]
                    .as_str()
                    .expect("target is a string")
                    .ends_with(".md")
        }),
        "dry-run export targets are unwritten .md files: {value}"
    );
}

#[test]
fn export_refuses_an_existing_md_target() {
    let workspace = TestWorkspace::new("migrate-cli-export-target");
    workspace.write("guides/guide.adoc", "# Guide\n\nProse.\n");
    workspace.write("guides/guide.md", "# Guide\n\nStill Markdown.\n");

    let (code, value) = migrate_json(&workspace, &["--export"]);

    assert_eq!(code, Some(1), "{value}");
    assert!(
        value["diagnostics"]
            .as_array()
            .expect("diagnostics array")
            .iter()
            .any(|diagnostic| diagnostic["code"] == "migrate.target_exists"),
        "{value}"
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
