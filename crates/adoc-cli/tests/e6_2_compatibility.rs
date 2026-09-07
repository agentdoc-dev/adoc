mod support;

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use support::{TestWorkspace, copy_tree, fixture_path};

const ARTIFACTS: [&str; 3] = ["docs.html", "docs.graph.json", "docs.search.json"];
const DATE: &str = "2026-09-07";
const BASELINE_COMMIT: &str = "e1b74a3f579933e3f8194c6c998f91a24578e9db";
const BASELINE_SHA256: &str = "9b4ad1331514d79f5c772ad3ba2ad73cbcd2c25bf828afe8fa2f6b71b6476d6f";

fn command(binary: &Path, root: &Path) -> Command {
    let mut command = Command::new(binary);
    command
        .current_dir(root)
        .env("ADOC_TEST_EMBEDDING_PROVIDER", "deterministic")
        .env_remove("ADOC_CONFIG")
        .env_remove("ADOC_AUDIENCE")
        .env_remove("ADOC_RETRIEVAL_POLICY");
    command
}

fn build(binary: &Path, root: &Path, out: &str) -> Output {
    assert!(
        !root.join(out).exists(),
        "each producer starts without a cache"
    );
    let output = command(binary, root)
        .args(["build", "docs", "--out", out, "--as-of", DATE])
        .output()
        .unwrap();
    assert!(output.status.success(), "build failed: {output:?}");
    output
}

fn assert_no_opt_in(value: &Value) {
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                assert!(
                    !matches!(
                        key.as_str(),
                        "visibility"
                            | "field_visibility"
                            | "classification"
                            | "field_projection"
                            | "declassification"
                            | "sensitive_access"
                    ),
                    "unexpected opt-in field {key}"
                );
                assert_no_opt_in(value);
            }
        }
        Value::Array(items) => items.iter().for_each(assert_no_opt_in),
        _ => {}
    }
}

fn assert_artifacts_equal(expected: &Path, actual: &Path) {
    for artifact in ARTIFACTS {
        let expected = fs::read(expected.join(artifact)).expect("baseline evidence must exist");
        let actual = fs::read(actual.join(artifact)).unwrap();
        assert!(expected == actual, "raw {artifact} bytes differ");
    }
    for (artifact, schema) in [
        ("docs.graph.json", "adoc.graph.v6"),
        ("docs.search.json", "adoc.search.v2"),
    ] {
        let value: Value =
            serde_json::from_slice(&fs::read(actual.join(artifact)).unwrap()).unwrap();
        assert_eq!(value["schema_version"], schema);
        assert_no_opt_in(&value);
    }
}

fn assert_golden_artifacts_equal(actual: &Path) {
    let root = fixture_path("e6_2_compatibility");
    let manifest: Value =
        serde_json::from_slice(&fs::read(root.join("baseline.json")).unwrap()).unwrap();
    for artifact in ARTIFACTS {
        let golden = manifest["golden_files"][artifact].as_str().unwrap();
        assert!(
            fs::read(root.join(golden)).unwrap() == fs::read(actual.join(artifact)).unwrap(),
            "raw {artifact} differs from historical golden"
        );
    }
}

#[test]
fn unclassified_artifacts_match_pre_e6_goldens() {
    let workspace = TestWorkspace::new("e6-2-compatibility");
    copy_tree(&fixture_path("v0_6/project"), &workspace.root.join("docs"));
    let binary = Path::new(env!("CARGO_BIN_EXE_adoc"));
    build(binary, &workspace.root, "candidate");
    assert_golden_artifacts_equal(&workspace.root.join("candidate"));
    build(binary, &workspace.root, "repeat");
    assert_artifacts_equal(
        &workspace.root.join("candidate"),
        &workspace.root.join("repeat"),
    );
}

// Python is an explicit prerequisite of this historical gate, not of normal
// workspace tests. Its standard library verifies the recorded raw SHA-256
// bindings without introducing a dependency or a second hashing implementation.
fn verify_bindings(baseline: &Path) -> Output {
    command(Path::new("python3"), &fixture_path("e6_2_compatibility"))
        .args(["-c", r#"
import hashlib, json, pathlib, sys
root = pathlib.Path.cwd()
manifest = json.loads((root / 'baseline.json').read_bytes())
assert manifest['commit'] == sys.argv[2], 'baseline commit changed'
assert manifest['binary_sha256'] == sys.argv[3], 'baseline binary binding changed'
assert hashlib.sha256(pathlib.Path(sys.argv[1]).read_bytes()).hexdigest() == sys.argv[3], 'wrong baseline binary'
assert manifest['build_args'] == ['build', 'docs', '--out', '<empty-output>', '--as-of', '2026-09-07']
assert manifest['embedding_provider'] == 'deterministic'
source = root.parent / 'v0_6' / 'project'
actual_sources = {str(p.relative_to(source)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(source.rglob('*')) if p.is_file()}
assert manifest['source_sha256'] == actual_sources, 'fixture source binding changed'
assert set(manifest['artifact_sha256']) == {'docs.html', 'docs.graph.json', 'docs.search.json'}
assert manifest['golden_files'] == {'docs.html': 'docs.html', 'docs.graph.json': 'docs.graph.golden.agent.json', 'docs.search.json': 'docs.search.golden.agent.json'}
for name, digest in manifest['artifact_sha256'].items():
    assert hashlib.sha256((root / manifest['golden_files'][name]).read_bytes()).hexdigest() == digest, 'golden binding changed: ' + name
print('baseline binary, commit, inputs and raw artifact bindings verified')
"#])
        .arg(baseline)
        .arg(BASELINE_COMMIT)
        .arg(BASELINE_SHA256)
        .output()
        .expect("historical gate requires python3")
}

#[test]
#[ignore = "historical producer gate; requires ADOC_RETRIEVAL_BASELINE_BIN and python3"]
fn independently_built_pre_e6_artifacts_are_byte_identical() {
    let baseline =
        std::env::var_os("ADOC_RETRIEVAL_BASELINE_BIN").expect("historical executable required");
    let baseline = Path::new(&baseline).canonicalize().unwrap();
    let bindings = verify_bindings(&baseline);
    assert!(
        bindings.status.success(),
        "binding verification failed: {}",
        String::from_utf8_lossy(&bindings.stderr)
    );
    let workspace = TestWorkspace::new("e6-2-independent-producers");
    copy_tree(&fixture_path("v0_6/project"), &workspace.root.join("docs"));
    let candidate = Path::new(env!("CARGO_BIN_EXE_adoc"));
    let before = build(&baseline, &workspace.root, "baseline");
    let after = build(candidate, &workspace.root, "candidate");
    assert_eq!(
        (before.status.code(), before.stderr),
        (after.status.code(), after.stderr)
    );
    assert_artifacts_equal(
        &workspace.root.join("baseline"),
        &workspace.root.join("candidate"),
    );
    assert_golden_artifacts_equal(&workspace.root.join("baseline"));

    for args in [
        vec!["search", "credits", "--lexical"],
        vec!["why", "billing.refunds"],
    ] {
        let run = |binary: &Path, producer: &str| {
            let query_root = workspace.root.join("query");
            if query_root.exists() {
                fs::remove_dir_all(&query_root).unwrap();
            }
            copy_tree(&workspace.root.join(producer), &query_root);
            command(binary, &workspace.root)
                .args(&args)
                .args(["--artifact", "query/docs.graph.json", "--format", "json"])
                .output()
                .unwrap()
        };
        let before = run(&baseline, "baseline");
        let after = run(candidate, "candidate");
        assert!(before.status.success(), "query failed: {before:?}");
        assert_eq!(
            (before.status.code(), &before.stdout, &before.stderr),
            (after.status.code(), &after.stdout, &after.stderr),
            "{args:?}"
        );
        let envelope: Value = serde_json::from_slice(&after.stdout).unwrap();
        assert!(!envelope["records"].as_array().unwrap().is_empty());
        assert_no_opt_in(&envelope);
    }
}
