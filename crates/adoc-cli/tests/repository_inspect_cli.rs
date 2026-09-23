use crate::support;
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
use support::TestWorkspace;

const DOC: &str = "# One @doc(test.one.page)\n\n::claim test.one\nstatus: draft\n--\nBody.\n::\n";

fn git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().into()
}
fn digest() -> String {
    format!("sha256:{}", "a".repeat(64))
}
/// Commit `files` into a fresh repository; returns the workspace and exact request.
fn repo(files: &[(&str, &str)]) -> (TestWorkspace, Value) {
    let dir = TestWorkspace::new("repository-inspect");
    git(&dir.root, &["init", "-q"]);
    git(&dir.root, &["config", "user.email", "test@example.test"]);
    git(&dir.root, &["config", "user.name", "Test"]);
    fs::write(dir.root.join("README.txt"), "not a source\n").unwrap();
    commit(&dir.root, files);
    let request = request(&dir.root);
    (dir, request)
}
fn commit(root: &Path, files: &[(&str, &str)]) {
    for (path, text) in files {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    git(root, &["add", "-A"]);
    git(root, &["commit", "-qm", "source", "--allow-empty"]);
}
fn request(root: &Path) -> Value {
    json!({"schema_version":"adoc.repository_inspection_request.v0", "workspace_id":"workspace-1", "provider_repository_id":"123456", "ref":"main", "git_revision":git(root, &["rev-parse", "HEAD"]), "evaluation_date":"2026-09-23", "runtime":{"version":env!("CARGO_PKG_VERSION"), "binary_digest":digest()}})
}
fn run(root: &Path, request: &Value) -> Output {
    let input = TestWorkspace::new("repository-inspect-request");
    let path = input.root.join("request.json");
    fs::write(&path, serde_json::to_vec(request).unwrap()).unwrap();
    Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args(["repository", "inspect", "--request"])
        .arg(&path)
        .arg("--repository")
        .arg(root)
        .args(["--runtime-binary-digest", &digest()])
        .output()
        .unwrap()
}
fn receipt(root: &Path, request: &Value) -> Value {
    let out = run(root, request);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}
fn refusal(root: &Path, request: &Value, code: &str) {
    let out = run(root, request);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty(), "no partial receipt");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains(&format!("error[{code}]")),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn configured_commit_reports_exact_counts_and_is_deterministic_and_read_only() {
    let (dir, request) = repo(&[
        (
            "agentdoc.config.yaml",
            "version: 1\nmode: strict\ndocs_path: docs\n",
        ),
        ("docs/one.adoc", DOC),
        ("docs/two.adoc", &DOC.replace("one", "two")),
    ]);
    fs::write(dir.root.join("docs/dirty.adoc"), "uncommitted\n").unwrap();
    let status = git(&dir.root, &["status", "--porcelain", "--ignored"]);
    let head = git(&dir.root, &["rev-parse", "HEAD"]);
    let first = run(&dir.root, &request);
    let second = run(&dir.root, &request);
    assert_eq!(first.stdout, second.stdout, "deterministic receipt bytes");
    assert_eq!(
        git(&dir.root, &["status", "--porcelain", "--ignored"]),
        status
    );
    assert_eq!(git(&dir.root, &["rev-parse", "HEAD"]), head);
    assert_eq!(
        git(&dir.root, &["worktree", "list", "--porcelain"])
            .lines()
            .filter(|l| l.starts_with("worktree "))
            .count(),
        1
    );
    let receipt: Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(
        receipt["schema_version"],
        "adoc.repository_inspection_receipt.v0"
    );
    assert_eq!(receipt["request"], request);
    assert_eq!(receipt["config_state"], "valid");
    assert_eq!(receipt["profile"], "committed");
    assert_eq!(receipt["finding"], "configured");
    assert_eq!(receipt["validation_result"], "pass");
    assert_eq!(receipt["eligible_file_count"], 2, "dirty file ignored");
    assert_eq!(receipt["parsed_item_count"], 4, "page plus claim per file");
    assert!(
        receipt["config_digest"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert!(receipt["generated_config_digest"].is_null());
    assert_eq!(receipt["runtime_binary_digest"], digest());
}

#[test]
fn no_config_uses_generated_profile_without_writing_it() {
    let (dir, request) = repo(&[("guide/one.adoc", DOC)]);
    let receipt = receipt(&dir.root, &request);
    assert_eq!(receipt["config_state"], "absent");
    assert_eq!(receipt["profile"], "generated_default_v1");
    assert_eq!(receipt["finding"], "files_without_config");
    assert_eq!(
        receipt["generated_config"],
        "# generated_default_v1: repository root; eligible sources: *.adoc only\nversion: 1\nmode: strict\ndocs_path: .\n"
    );
    assert!(
        receipt["generated_config_digest"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert!(receipt["config_digest"].is_null());
    assert_eq!(receipt["eligible_file_count"], 1);
    assert_eq!(receipt["parsed_item_count"], 2);
    assert!(!dir.root.join("agentdoc.config.yaml").exists());
    assert_eq!(git(&dir.root, &["status", "--porcelain"]), "");
}

#[test]
fn invalid_config_is_reported_not_replaced() {
    let (dir, request) = repo(&[
        (
            "agentdoc.config.yaml",
            "version: 1\nmode: strict\ndocs_path: ../outside\n",
        ),
        ("one.adoc", DOC),
    ]);
    let receipt = receipt(&dir.root, &request);
    assert_eq!(receipt["config_state"], "invalid");
    assert_eq!(receipt["profile"], "committed");
    assert_eq!(receipt["finding"], "invalid_config");
    assert!(receipt["config_digest"].as_str().is_some());
    assert!(receipt["eligible_file_count"].is_null());
    assert!(receipt["parsed_item_count"].is_null());
    assert!(receipt["manifest_digest"].is_null());
    assert!(receipt["generated_config"].is_null());
}

#[test]
fn empty_and_malformed_commits_are_distinct() {
    let (empty, request) = repo(&[(
        "agentdoc.config.yaml",
        "version: 1\nmode: strict\ndocs_path: .\n",
    )]);
    let receipt_empty = receipt(&empty.root, &request);
    assert_eq!(receipt_empty["finding"], "no_eligible_files");
    assert_eq!(receipt_empty["config_state"], "valid");
    assert_eq!(receipt_empty["eligible_file_count"], 0);
    assert_eq!(receipt_empty["validation_result"], "not_run");

    let (bad, request) = repo(&[("bad.adoc", "::claim\nno id or close\n")]);
    let receipt_bad = receipt(&bad.root, &request);
    assert_eq!(receipt_bad["finding"], "files_without_config");
    assert_eq!(receipt_bad["validation_result"], "fail");
    assert_eq!(receipt_bad["eligible_file_count"], 1);
    assert!(receipt_bad["parsed_item_count"].is_null());
    let codes = receipt_bad["diagnostic_codes"].as_array().unwrap();
    assert!(!codes.is_empty() && codes.iter().all(Value::is_string));
}

#[test]
fn two_commits_bind_different_manifest_digests() {
    let (dir, first_request) = repo(&[("one.adoc", DOC)]);
    let first = receipt(&dir.root, &first_request);
    commit(
        &dir.root,
        &[("one.adoc", &DOC.replace("Body.", "Changed."))],
    );
    let second_request = request(&dir.root);
    let second = receipt(&dir.root, &second_request);
    assert_ne!(first["manifest_digest"], second["manifest_digest"]);
    assert_eq!(
        receipt(&dir.root, &first_request)["manifest_digest"],
        first["manifest_digest"]
    );
}

#[test]
fn symlink_escape_oversize_and_foreign_requests_refuse_without_receipt() {
    let (dir, _) = repo(&[("one.adoc", DOC)]);
    std::os::unix::fs::symlink("/etc/hosts", dir.root.join("escape.adoc")).unwrap();
    git(&dir.root, &["add", "-A"]);
    git(&dir.root, &["commit", "-qm", "symlink"]);
    refusal(&dir.root, &request(&dir.root), "migration.unsafe_source");

    let big = "a".repeat(adoc_core::MIGRATION_IMPORT_MAX_BYTES + 1);
    let (large, request_large) = repo(&[("big.adoc", &big)]);
    refusal(&large.root, &request_large, "migration.output_limit");

    let (ok, request_ok) = repo(&[("one.adoc", DOC)]);
    let mut foreign = request_ok.clone();
    foreign["foreign"] = true.into();
    refusal(&ok.root, &foreign, "migration.invalid_request");
    let mut pinned = request_ok.clone();
    pinned["runtime"]["binary_digest"] = format!("sha256:{}", "b".repeat(64)).into();
    refusal(&ok.root, &pinned, "migration.invalid_request");
    let mut unknown = request_ok.clone();
    unknown["git_revision"] = "f".repeat(40).into();
    refusal(&ok.root, &unknown, "migration.snapshot_unavailable");
}

#[test]
fn generated_profile_selects_only_adoc_sources() {
    let (dir, request) = repo(&[("README.md", "# Readme\n"), ("docs/guide.md", "# Guide\n")]);
    let md_only = receipt(&dir.root, &request);
    assert_eq!(md_only["config_state"], "absent");
    assert_eq!(md_only["finding"], "no_eligible_files");
    assert_eq!(md_only["eligible_file_count"], 0);
    assert_eq!(md_only["parsed_item_count"], 0);
    let (mixed, request) = repo(&[("README.md", "# Readme\n"), ("one.adoc", DOC)]);
    assert_eq!(receipt(&mixed.root, &request)["eligible_file_count"], 1);
}

#[test]
fn parsed_item_count_counts_failed_graphs_and_nulls_unparseable_sources() {
    let (dup, request) = repo(&[("one.adoc", DOC), ("two.adoc", DOC)]);
    let failed = receipt(&dup.root, &request);
    assert_eq!(failed["validation_result"], "fail");
    assert_eq!(failed["parsed_item_count"], 4, "{failed}");
    let (bad, request) = repo(&[("bad.adoc", "::claim\nno id or close\n")]);
    let unparseable = receipt(&bad.root, &request);
    assert_eq!(unparseable["validation_result"], "fail");
    assert!(unparseable["parsed_item_count"].is_null(), "{unparseable}");
}

#[test]
fn malformed_runtime_digests_refuse_before_inspection() {
    let (dir, mut request) = repo(&[("one.adoc", DOC)]);
    request["runtime"]["binary_digest"] = "sha256:short".into();
    refusal(&dir.root, &request, "migration.invalid_request");
    let input = TestWorkspace::new("repository-inspect-request");
    let path = input.root.join("request.json");
    request["runtime"]["binary_digest"] = "not-a-digest".into();
    fs::write(&path, serde_json::to_vec(&request).unwrap()).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args(["repository", "inspect", "--request"])
        .arg(&path)
        .arg("--repository")
        .arg(&dir.root)
        .args(["--runtime-binary-digest", "not-a-digest"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    assert!(String::from_utf8_lossy(&out.stderr).contains("error[migration.invalid_request]"));
}

#[test]
fn source_count_gitlink_symlinked_config_and_escaping_docs_path_are_bounded() {
    let many: Vec<(String, String)> = (0..=adoc_core::MIGRATION_IMPORT_MAX_SOURCES)
        .map(|n| (format!("f{n}.adoc"), format!("# F{n} @doc(test.f{n})\n")))
        .collect();
    let files: Vec<(&str, &str)> = many.iter().map(|(p, t)| (p.as_str(), t.as_str())).collect();
    let (large, large_request) = repo(&files);
    refusal(&large.root, &large_request, "migration.output_limit");

    let (sub, _) = repo(&[("one.adoc", DOC)]);
    let head = git(&sub.root, &["rev-parse", "HEAD"]);
    git(
        &sub.root,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{head},vendor"),
        ],
    );
    git(&sub.root, &["commit", "-qm", "gitlink"]);
    refusal(&sub.root, &request(&sub.root), "migration.unsafe_source");

    let (link, _) = repo(&[
        ("real.yaml", "version: 1\nmode: strict\ndocs_path: .\n"),
        ("one.adoc", DOC),
    ]);
    std::os::unix::fs::symlink("real.yaml", link.root.join("agentdoc.config.yaml")).unwrap();
    git(&link.root, &["add", "-A"]);
    git(&link.root, &["commit", "-qm", "symlinked config"]);
    refusal(&link.root, &request(&link.root), "migration.unsafe_source");

    let (escape, escape_request) = repo(&[
        (
            "agentdoc.config.yaml",
            "version: 1\nmode: strict\ndocs_path: ../../etc\n",
        ),
        ("one.adoc", DOC),
    ]);
    let escaped = receipt(&escape.root, &escape_request);
    assert_eq!(escaped["finding"], "invalid_config");
    assert!(escaped["eligible_file_count"].is_null());
}

#[test]
fn generated_profile_never_reads_or_counts_markdown() {
    let big = "a".repeat(adoc_core::MIGRATION_IMPORT_MAX_BYTES + 1);
    let (dir, request) = repo(&[("one.adoc", DOC), ("big.md", &big)]);
    let inspected = receipt(&dir.root, &request);
    assert_eq!(inspected["eligible_file_count"], 1);
    assert_eq!(inspected["validation_result"], "pass");
    let many: Vec<(String, String)> = (0..=adoc_core::MIGRATION_IMPORT_MAX_SOURCES)
        .map(|n| (format!("f{n}.md"), format!("# F{n}\n")))
        .collect();
    let mut files: Vec<(&str, &str)> = many.iter().map(|(p, t)| (p.as_str(), t.as_str())).collect();
    files.push(("one.adoc", DOC));
    let (many_md, request) = repo(&files);
    assert_eq!(receipt(&many_md.root, &request)["eligible_file_count"], 1);
}

#[test]
fn unreadable_source_has_null_parsed_item_count() {
    let (dir, _) = repo(&[("one.adoc", DOC)]);
    fs::write(dir.root.join("two.adoc"), [0xff, 0xfe]).unwrap();
    git(&dir.root, &["add", "-A"]);
    git(&dir.root, &["commit", "-qm", "non-utf8"]);
    let inspected = receipt(&dir.root, &request(&dir.root));
    assert_eq!(inspected["validation_result"], "fail");
    assert!(inspected["parsed_item_count"].is_null(), "{inspected}");
}
