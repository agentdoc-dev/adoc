use crate::support;
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
use support::TestWorkspace;

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
fn fixture() -> (TestWorkspace, Value, Value) {
    let workspace = TestWorkspace::new("migration-import");
    let root = &workspace.root;
    git(root, &["init", "-q"]);
    git(root, &["config", "user.email", "test@example.test"]);
    git(root, &["config", "user.name", "Test"]);
    fs::create_dir(root.join("docs")).unwrap();
    fs::write(
        root.join("agentdoc.config.yaml"),
        "version: 1\nmode: strict\ndocs_path: docs\n",
    )
    .unwrap();
    for name in ["one", "two"] {
        fs::write(root.join(format!("docs/{name}.adoc")), format!("# {name} @doc(test.{name}.page)\n\n::claim test.{name}\nstatus: draft\n--\nBody.\n::\n")).unwrap();
    }
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "source"]);
    let request = json!({"schema_version":"adoc.migration_request.v0","request_id":"request-1","workspace_id":"workspace-1","source_id":"source-1","repository_identity":"repo:test","revision":{"system":"git","value":git(root,&["rev-parse","HEAD"])},"evaluation_date":"2026-09-08"});
    let job = json!({"schema_version":"agentdoc.cloud.migration_import_job.v0","connector_id":"connector-1","observed_at":"2026-09-08T12:00:00Z","source_acl_scope":{"snapshot_id":"acl-1","source_container_id":"source-1","source":{"kind":"repository","id":"repo:test"}},"sources":[{"path":"docs/one.adoc","source_record_id":"record-1","source_binding_id":"binding-1"},{"path":"docs/two.adoc","source_record_id":"record-2","source_binding_id":"binding-2"}]});
    (workspace, request, job)
}
fn run_command(root: &Path, request: &Value, job: &Value, command: &str, policy: &str) -> Output {
    let inputs = TestWorkspace::new("migration-import-input");
    let request_path = inputs.root.join("request.json");
    let job_path = inputs.root.join("job.json");
    fs::write(&request_path, serde_json::to_vec(request).unwrap()).unwrap();
    fs::write(&job_path, serde_json::to_vec(job).unwrap()).unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_adoc"));
    cmd.arg(command);
    if command == "migration-qualify" {
        cmd.args(["--qualification-policy-version", policy]);
    }
    cmd.arg("--request")
        .arg(request_path)
        .arg("--job")
        .arg(job_path)
        .arg("--repository")
        .arg(root)
        .args([
            "--runtime-binary-digest",
            &format!("sha256:{}", "a".repeat(64)),
        ])
        .output()
        .unwrap()
}
fn nested(source: &Value, key: &str) -> Value {
    serde_json::from_str(source[key].as_str().unwrap()).unwrap()
}

fn run(root: &Path, request: &Value, job: &Value) -> Output {
    run_command(root, request, job, "migration-qualify", "1")
}
#[test]
fn qualification_retains_t2_bytes_and_draft_candidates_without_authority() {
    let (workspace, request, job) = fixture();
    let imported = run_command(&workspace.root, &request, &job, "migration-import", "1");
    assert!(imported.status.success());
    let output = run(&workspace.root, &request, &job);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        envelope["candidate_bundle_bytes"]
            .as_str()
            .unwrap()
            .as_bytes(),
        imported.stdout
    );
    let receipt = nested(&envelope, "qualification_receipt_bytes");
    assert_eq!(receipt["objects"].as_array().unwrap().len(), 2);
    for object in receipt["objects"].as_array().unwrap() {
        assert_eq!(object["eligible"], false);
        assert_eq!(object["reasons"][0]["code"], "lifecycle_not_adopted");
    }
    assert_eq!(output.stdout, run(&workspace.root, &request, &job).stdout);
    fs::write(workspace.root.join("docs/two.adoc"), "dirty").unwrap();
    assert_eq!(output.stdout, run(&workspace.root, &request, &job).stdout);
}
#[test]
fn qualification_preserves_failed_raw_evidence_including_invalid_utf8() {
    let (workspace, mut request, job) = fixture();
    for bytes in [
        b"# Invalid @doc(invalid.page)\n\n::claim missing.status\n--\nBody\n::\n".as_slice(),
        &[0xff, 0xfe],
    ] {
        fs::write(workspace.root.join("docs/two.adoc"), bytes).unwrap();
        git(&workspace.root, &["add", "."]);
        git(&workspace.root, &["commit", "-qm", "invalid source"]);
        request["revision"]["value"] = json!(git(&workspace.root, &["rev-parse", "HEAD"]));
        let output = run(&workspace.root, &request, &job);
        assert_eq!(
            output.status.code(),
            Some(1),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(envelope["outcome"], "flagged_source_evidence");
        assert!(envelope.get("candidate_bundle_bytes").is_none());
        assert!(envelope.get("graph_artifact_bytes").is_none());
        let receipt = nested(&envelope, "validation_receipt_bytes");
        assert_eq!(receipt["result"], "fail");
        assert_eq!(envelope["sources"].as_array().unwrap().len(), 2);
        assert!(
            !nested(&envelope, "diagnostics_bytes")
                .as_array()
                .unwrap()
                .is_empty()
        );
        if bytes == [0xff, 0xfe] {
            assert_eq!(envelope["sources"][1]["source_bytes_base64"], "//4=");
        }
        assert_eq!(output.stdout, run(&workspace.root, &request, &job).stdout);
        assert_eq!(
            run_command(&workspace.root, &request, &job, "migration-import", "1")
                .status
                .code(),
            Some(2)
        );
    }
}
#[test]
fn flagged_source_record_media_type_follows_bytes_not_outcome() {
    let (workspace, mut request, job) = fixture();
    fs::write(workspace.root.join("docs/two.adoc"), [0xff, 0xfe]).unwrap();
    git(&workspace.root, &["add", "."]);
    git(&workspace.root, &["commit", "-qm", "invalid source"]);
    request["revision"]["value"] = json!(git(&workspace.root, &["rev-parse", "HEAD"]));
    let output = run(&workspace.root, &request, &job);
    assert_eq!(output.status.code(), Some(1));
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["outcome"], "flagged_source_evidence");
    let media_type =
        |i: usize| nested(&envelope["sources"][i], "source_record_bytes")["media_type"].clone();
    assert_eq!(media_type(0), "text/plain");
    assert_eq!(media_type(1), "application/octet-stream");
}
#[test]
fn qualification_refuses_unknown_policy_and_incomplete_coverage() {
    let (workspace, request, mut job) = fixture();
    let output = run_command(&workspace.root, &request, &job, "migration-qualify", "2");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    job["sources"].as_array_mut().unwrap().pop();
    let output = run(&workspace.root, &request, &job);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}

#[test]
fn qualification_uses_fixed_date_and_cross_source_contradiction_facts() {
    let (workspace, mut request, job) = fixture();
    let verified = |id: &str, expiry: &str| {
        format!(
            "::claim {id}\nstatus: verified\nowner: team\nverified_at: 2026-01-01\ntest: cargo test\nexpires_at: {expiry}\n--\nBody.\n::\n\n"
        )
    };
    fs::write(workspace.root.join("docs/one.adoc"), format!("# One @doc(test.one.page)\n\n{}{}{}::policy test.policy\nstatus: active\nowner: team\napproved_by: team\neffective_at: 2026-01-01\nreview_interval: 30d\n--\nPolicy.\n::\n", verified("test.current", "2026-09-08"), verified("test.expired", "2026-09-07"), verified("test.clear", "2026-09-09"))).unwrap();
    fs::write(workspace.root.join("docs/two.adoc"), "# Two @doc(test.two.page)\n\n::contradiction test.conflict\nseverity: high\nstatus: unresolved\nclaims: [test.current, test.expired]\n--\nConflict.\n::\n").unwrap();
    git(&workspace.root, &["add", "."]);
    git(&workspace.root, &["commit", "-qm", "qualification facts"]);
    request["revision"]["value"] = json!(git(&workspace.root, &["rev-parse", "HEAD"]));
    let output = run(&workspace.root, &request, &job);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{} {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    let receipt = nested(&envelope, "qualification_receipt_bytes");
    let objects = receipt["objects"].as_array().unwrap();
    let find = |id: &str| {
        objects
            .iter()
            .find(|object| object["object_id"] == id)
            .unwrap()
    };
    assert_eq!(find("test.clear")["eligible"], true);
    assert_eq!(
        find("test.current")["reasons"],
        json!([{"code":"contradicted","related_object_ids":["test.conflict"],"diagnostic_codes":[]}])
    );
    assert_eq!(
        find("test.expired")["reasons"],
        json!([{"code":"stale","related_object_ids":[],"diagnostic_codes":[]},{"code":"contradicted","related_object_ids":["test.conflict"],"diagnostic_codes":[]}])
    );
    assert_eq!(find("test.policy")["reasons"][0]["code"], "review_overdue");
    assert_eq!(find("test.conflict")["source_record_id"], "record-2");
}

#[test]
fn qualification_refuses_encoded_output_overflow_without_partial_stdout() {
    let (workspace, mut request, job) = fixture();
    // Raw input is below the 8MiB cap, but canonical padded base64 exceeds it.
    fs::write(
        workspace.root.join("docs/two.adoc"),
        vec![0xff; 7 * 1024 * 1024],
    )
    .unwrap();
    git(&workspace.root, &["add", "."]);
    git(&workspace.root, &["commit", "-qm", "large invalid binary"]);
    request["revision"]["value"] = json!(git(&workspace.root, &["rev-parse", "HEAD"]));
    let output = run(&workspace.root, &request, &job);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("migration.output_limit"));
}

#[test]
fn qualification_refuses_unsafe_checkout_instead_of_retaining_it() {
    let (workspace, mut request, job) = fixture();
    fs::write(
        workspace.root.join(".gitattributes"),
        "*.adoc filter=external\n",
    )
    .unwrap();
    git(&workspace.root, &["add", "."]);
    git(&workspace.root, &["commit", "-qm", "unsafe attributes"]);
    request["revision"]["value"] = json!(git(&workspace.root, &["rev-parse", "HEAD"]));
    let output = run(&workspace.root, &request, &job);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("migration.unsafe_source"));
}

// ---- U3.3.P3a versioned history/fresh prepare contract ----
fn digest() -> String {
    format!("sha256:{}", "a".repeat(64))
}
fn v1(request: &Value, starting_point: &str) -> Value {
    let mut request = request.clone();
    request["schema_version"] = json!("adoc.migration_request.v1");
    request["inspection_id"] = json!("inspection-1");
    request["inspection_digest"] = json!(format!("sha256:{}", "b".repeat(64)));
    request["starting_point"] = json!(starting_point);
    request
}
fn prepare(root: &Path, request: &Value) -> Output {
    let inputs = TestWorkspace::new("migration-prepare-input");
    let request_path = inputs.root.join("request.json");
    fs::write(&request_path, serde_json::to_vec(request).unwrap()).unwrap();
    Command::new(env!("CARGO_BIN_EXE_adoc"))
        .arg("migration-prepare")
        .arg("--request")
        .arg(request_path)
        .arg("--repository")
        .arg(root)
        .args(["--runtime-binary-digest", &digest()])
        .output()
        .unwrap()
}
fn ok_json(output: &Output) -> Value {
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn keys(value: &Value) -> std::collections::BTreeSet<&str> {
    value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect()
}
/// One adopted (verified) item so history and fresh eligibility can differ.
fn adopted_fixture() -> (TestWorkspace, Value, Value) {
    let (workspace, mut request, job) = fixture();
    let root = &workspace.root;
    fs::write(
        root.join("docs/one.adoc"),
        "# one @doc(test.one.page)\n\n::claim test.one\nstatus: verified\nowner: team\nverified_at: 2026-01-01\ntest: cargo test\nexpires_at: 2027-01-01\n--\nBody.\n::\n",
    )
    .unwrap();
    git(root, &["commit", "-qam", "adopt"]);
    request["revision"]["value"] = json!(git(root, &["rev-parse", "HEAD"]));
    (workspace, request, job)
}

#[test]
fn migration_v0_output_shape_and_request_bytes_are_unchanged() {
    let (workspace, request, job) = adopted_fixture();
    let receipt = ok_json(&prepare(&workspace.root, &request));
    assert_eq!(
        keys(&receipt),
        std::collections::BTreeSet::from([
            "schema_version",
            "phase",
            "request",
            "request_digest",
            "validation_receipt",
            "diagnostics"
        ])
    );
    assert_eq!(receipt["schema_version"], "adoc.migration_receipt.v0");
    assert_eq!(receipt["request"], request);
    let envelope = ok_json(&run(&workspace.root, &request, &job));
    assert_eq!(
        envelope["schema_version"],
        "adoc.migration_qualification.v0"
    );
    assert_eq!(envelope["request"], request);
    assert_eq!(envelope["qualification_policy_version"], "1");
    let qualification = nested(&envelope, "qualification_receipt_bytes");
    assert_eq!(
        keys(&qualification),
        std::collections::BTreeSet::from([
            "schema_version",
            "request_digest",
            "job_digest",
            "candidate_bundle_digest",
            "graph_artifact_digest",
            "config_digest",
            "evaluation_date",
            "qualification_policy_version",
            "lifecycle_mapping_version",
            "objects"
        ])
    );
    assert_eq!(
        nested(&envelope, "candidate_bundle_bytes")["schema_version"],
        "adoc.migration_import.v0"
    );
    // v0 cannot request fresh, implicitly or explicitly.
    assert_eq!(
        run_command(
            &workspace.root,
            &request,
            &job,
            "migration-qualify",
            "fresh.1"
        )
        .status
        .code(),
        Some(2)
    );
}

#[test]
fn migration_v1_history_and_fresh_differ_in_identity_and_eligibility_only() {
    let (workspace, request, job) = adopted_fixture();
    let root = &workspace.root;
    let (history, fresh) = (v1(&request, "recorded_history"), v1(&request, "fresh"));
    let prepared_history = ok_json(&prepare(root, &history));
    let prepared_fresh = ok_json(&prepare(root, &fresh));
    for (receipt, sent) in [(&prepared_history, &history), (&prepared_fresh, &fresh)] {
        assert_eq!(receipt["schema_version"], "adoc.migration_receipt.v1");
        assert_eq!(receipt["config_profile"], "committed");
        assert_eq!(&receipt["request"], sent);
    }
    assert_ne!(
        prepared_history["request_digest"],
        prepared_fresh["request_digest"]
    );
    let history_envelope = ok_json(&run_command(root, &history, &job, "migration-qualify", "1"));
    let fresh_envelope = ok_json(&run_command(
        root,
        &fresh,
        &job,
        "migration-qualify",
        "fresh.1",
    ));
    assert_eq!(
        history_envelope["schema_version"],
        "adoc.migration_qualification.v1"
    );
    assert_eq!(fresh_envelope["qualification_policy_version"], "fresh.1");
    let history_receipt = nested(&history_envelope, "qualification_receipt_bytes");
    let fresh_receipt = nested(&fresh_envelope, "qualification_receipt_bytes");
    assert_eq!(
        fresh_receipt["schema_version"],
        "adoc.migration_qualification_receipt.v1"
    );
    assert_eq!(history_receipt["starting_point"], "recorded_history");
    assert_eq!(fresh_receipt["starting_point"], "fresh");
    assert_ne!(
        history_receipt["request_digest"],
        fresh_receipt["request_digest"]
    );
    // Source and graph hashing never change merely to strip history.
    assert_eq!(
        history_receipt["graph_artifact_digest"],
        fresh_receipt["graph_artifact_digest"]
    );
    let sources = |envelope: &Value| {
        nested(envelope, "candidate_bundle_bytes")["sources"]
            .as_array()
            .unwrap()
            .iter()
            .map(|source| source["source_bytes"].clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(sources(&history_envelope), sources(&fresh_envelope));
    let history_objects = history_receipt["objects"].as_array().unwrap();
    assert!(
        history_objects
            .iter()
            .any(|object| object["eligible"] == true)
    );
    let fresh_objects = fresh_receipt["objects"].as_array().unwrap();
    assert_eq!(fresh_objects.len(), history_objects.len());
    for (fresh, history) in fresh_objects.iter().zip(history_objects) {
        assert_eq!(fresh["eligible"], false);
        assert_eq!(
            fresh["reasons"],
            json!([{"code":"fresh_review_required","related_object_ids":[],"diagnostic_codes":[]}])
        );
        assert_eq!(fresh["content_hash"], history["content_hash"]);
    }
}

#[test]
fn migration_v1_refuses_mode_tampering_unknown_versions_and_fields() {
    let (workspace, request, job) = adopted_fixture();
    let root = &workspace.root;
    let code = |output: Output| output.status.code();
    for (sent, policy) in [
        (v1(&request, "fresh"), "1"),
        (v1(&request, "recorded_history"), "fresh.1"),
        (v1(&request, "recorded_history"), "2"),
    ] {
        assert_eq!(
            code(run_command(root, &sent, &job, "migration-qualify", policy)),
            Some(2)
        );
    }
    let mut invalid = Vec::new();
    let mut version = v1(&request, "fresh");
    version["schema_version"] = json!(format!("adoc.migration_request.v{}", 2));
    invalid.push(version);
    let mut foreign = v1(&request, "fresh");
    foreign["foreign"] = json!(true);
    invalid.push(foreign);
    let mut mode = v1(&request, "fresh");
    mode["starting_point"] = json!("auto");
    invalid.push(mode);
    let mut null_mode = v1(&request, "fresh");
    null_mode["starting_point"] = Value::Null;
    invalid.push(null_mode);
    let mut missing = v1(&request, "fresh");
    missing.as_object_mut().unwrap().remove("inspection_digest");
    invalid.push(missing);
    let mut bad_digest = v1(&request, "fresh");
    bad_digest["inspection_digest"] = json!("sha256:short");
    invalid.push(bad_digest);
    let mut v0_with_mode = request.clone();
    v0_with_mode["starting_point"] = json!("recorded_history");
    invalid.push(v0_with_mode);
    for sent in &invalid {
        assert_eq!(code(prepare(root, sent)), Some(2), "{sent}");
        assert_eq!(
            code(run_command(
                root,
                sent,
                &job,
                "migration-qualify",
                "fresh.1"
            )),
            Some(2),
            "{sent}"
        );
    }
}

#[test]
fn migration_v1_no_config_is_fresh_only_via_generated_profile() {
    let workspace = TestWorkspace::new("migration-no-config");
    let root = &workspace.root;
    git(root, &["init", "-q"]);
    git(root, &["config", "user.email", "test@example.test"]);
    git(root, &["config", "user.name", "Test"]);
    fs::write(
        root.join("one.adoc"),
        "# one @doc(test.one.page)\n\n::claim test.one\nstatus: verified\nowner: team\nverified_at: 2026-01-01\ntest: cargo test\nexpires_at: 2027-01-01\n--\nBody.\n::\n",
    )
    .unwrap();
    fs::write(
        root.join("README.md"),
        "# not eligible under the generated profile\n",
    )
    .unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "source"]);
    let (_, base, mut job) = fixture();
    let mut request = base.clone();
    request["revision"]["value"] = json!(git(root, &["rev-parse", "HEAD"]));
    job["sources"] =
        json!([{"path":"one.adoc","source_record_id":"record-1","source_binding_id":"binding-1"}]);
    let fresh = v1(&request, "fresh");
    let receipt = ok_json(&prepare(root, &fresh));
    assert_eq!(receipt["config_profile"], "generated_default_v1");
    let envelope = ok_json(&run_command(
        root,
        &fresh,
        &job,
        "migration-qualify",
        "fresh.1",
    ));
    assert!(
        envelope["config_bytes"]
            .as_str()
            .unwrap()
            .starts_with("# generated_default_v1")
    );
    let objects = nested(&envelope, "qualification_receipt_bytes")["objects"].clone();
    assert!(!objects.as_array().unwrap().is_empty());
    assert!(
        objects
            .as_array()
            .unwrap()
            .iter()
            .all(|o| o["eligible"] == false)
    );
    let imported = ok_json(&run_command(root, &fresh, &job, "migration-import", "1"));
    assert_eq!(imported["schema_version"], "adoc.migration_import.v1");
    assert_eq!(imported["sources"].as_array().unwrap().len(), 1);
    assert_eq!(imported["sources"][0]["path"], "one.adoc");
    let history = v1(&request, "recorded_history");
    assert_eq!(
        run_command(root, &history, &job, "migration-import", "1")
            .status
            .code(),
        Some(2)
    );
    assert_eq!(prepare(root, &history).status.code(), Some(2));
    assert_eq!(
        run_command(root, &history, &job, "migration-qualify", "1")
            .status
            .code(),
        Some(2)
    );
    // v0 keeps its committed-config-only behavior.
    assert_eq!(prepare(root, &request).status.code(), Some(2));
}

fn pinned_git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_AUTHOR_DATE", "2026-09-08T12:00:00Z")
        .env("GIT_COMMITTER_DATE", "2026-09-08T12:00:00Z")
        .output()
        .unwrap();
    assert!(out.status.success());
    String::from_utf8(out.stdout).unwrap().trim().into()
}

/// Golden prepare/qualify/import bytes were captured from the pre-v1 binary (a5e6f02d). Regenerate only with
/// `ADOC_GOLDEN_BIN=<that binary> cargo test ... v0_bytes_match_pre_v1_golden`.
#[test]
fn migration_v0_bytes_match_pre_v1_golden() {
    let workspace = TestWorkspace::new("migration-golden");
    let root = &workspace.root;
    pinned_git(root, &["init", "-q"]);
    pinned_git(root, &["config", "user.email", "test@example.test"]);
    pinned_git(root, &["config", "user.name", "Test"]);
    fs::create_dir(root.join("docs")).unwrap();
    fs::write(
        root.join("agentdoc.config.yaml"),
        "version: 1\nmode: strict\ndocs_path: docs\n",
    )
    .unwrap();
    fs::write(root.join("docs/one.adoc"), "# one @doc(test.one.page)\n\n::claim test.one\nstatus: verified\nowner: team\nverified_at: 2026-01-01\ntest: cargo test\nexpires_at: 2027-01-01\n--\nBody.\n::\n").unwrap();
    fs::write(
        root.join("docs/two.adoc"),
        "# two @doc(test.two.page)\n\n::claim test.two\nstatus: draft\n--\nBody.\n::\n",
    )
    .unwrap();
    pinned_git(root, &["add", "."]);
    pinned_git(root, &["commit", "-qm", "source"]);
    let (_, mut request, job) = fixture();
    request["revision"]["value"] = json!(pinned_git(root, &["rev-parse", "HEAD"]));
    let inputs = TestWorkspace::new("migration-golden-input");
    let (request_path, job_path) = (
        inputs.root.join("request.json"),
        inputs.root.join("job.json"),
    );
    fs::write(&request_path, serde_json::to_vec(&request).unwrap()).unwrap();
    fs::write(&job_path, serde_json::to_vec(&job).unwrap()).unwrap();
    let golden = std::env::var_os("ADOC_GOLDEN_BIN");
    let binary = golden
        .clone()
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_adoc").into());
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for (command, name) in [
        ("migration-prepare", "migration-v0-prepare.golden.json"),
        ("migration-qualify", "migration-v0-qualify.golden.json"),
        ("migration-import", "migration-v0-import.golden.json"),
    ] {
        let mut cmd = Command::new(&binary);
        cmd.arg(command).arg("--request").arg(&request_path);
        if command == "migration-qualify" {
            cmd.args(["--qualification-policy-version", "1"]);
        }
        if command != "migration-prepare" {
            cmd.arg("--job").arg(&job_path);
        }
        let output = cmd
            .arg("--repository")
            .arg(root)
            .args(["--runtime-binary-digest", &digest()])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0), "{command}");
        if golden.is_some() {
            fs::write(fixtures.join(name), &output.stdout).unwrap();
        }
        assert!(
            fs::read(fixtures.join(name)).unwrap() == output.stdout,
            "{command} v0 output bytes changed"
        );
    }
}
