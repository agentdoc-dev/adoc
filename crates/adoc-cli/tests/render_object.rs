//! Issue 286 (M4.1.T0): `adoc render-object` round-trips every authored node member.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use crate::support::{TestWorkspace, adoc_command, copy_tree, stderr, stdout};

/// Placement and derived members: never authored, never rendered.
const NOT_AUTHORED: [&str; 7] = [
    "content_hash",
    "page_id",
    "source_span",
    "source_binding",
    "effective_status",
    "effective_reason",
    "evidence_quality",
];

const EXTRA: &str = r#"# Extra @doc(extra.page)

::warning extra.restricted-warning
severity: high
visibility: restricted
related_to: [auth.mfa.enforced]
--
Restricted warning body with [[auth.mfa.enforced]].
::

::claim extra.internal-claim
status: draft
visibility: internal
depends_on: auth.mfa.enforced, extra.restricted-warning
--
Internal claim.
::

::policy extra.field-policy
status: active
owner: security
effective_at: 2026-01-01
review_interval: 90d
approved_by: [security-lead, platform-lead]
field_visibility: owner=restricted, review_interval=internal
--
Policy with per-field visibility.
::
"#;

fn build_graph(workspace: &TestWorkspace, out: &str) -> Value {
    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "docs", "--out", out, "--no-embeddings"])
        .output()
        .expect("adoc build runs");
    assert!(
        output.status.success(),
        "build failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let text = fs::read_to_string(workspace.root.join(out).join("docs.graph.json"))
        .expect("graph artifact is readable");
    serde_json::from_str(&text).expect("graph is JSON")
}

fn objects(graph: &Value) -> Vec<Value> {
    graph["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .filter(|node| node["type"] == "knowledge_object")
        .cloned()
        .collect()
}

fn authored(node: &Value) -> Value {
    let mut map = node.as_object().expect("node object").clone();
    map.remove("type");
    for member in NOT_AUTHORED {
        map.remove(member);
    }
    Value::Object(map)
}

fn render(workspace: &TestWorkspace, input: &Value, out: &Path) -> std::process::Output {
    let input_path = workspace.write("input.json", &input.to_string());
    adoc_command()
        .current_dir(&workspace.root)
        .arg("render-object")
        .arg("--input")
        .arg(input_path)
        .arg("--out")
        .arg(out)
        .output()
        .expect("adoc render-object runs")
}

fn with_schema(mut input: Value) -> Value {
    input["schema_version"] = json!("adoc.render_object.v0");
    input
}

#[test]
fn render_object_round_trips_every_kind_and_authored_member() {
    let source = TestWorkspace::new("render-object-source");
    copy_tree(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/expanded-pilot"),
        &source.root.join("docs"),
    );
    source.write("docs/extra.adoc", EXTRA);
    let original = objects(&build_graph(&source, "dist"));

    let kinds: BTreeSet<_> = original
        .iter()
        .map(|node| node["kind"].as_str().expect("kind").to_string())
        .collect();
    assert_eq!(kinds.len(), 15, "fixture must cover every kind: {kinds:?}");
    let has = |member: &str| original.iter().any(|node| node.get(member).is_some());
    for member in [
        "visibility",
        "field_visibility",
        "severity",
        "trust",
        "approved_by",
        "evidence",
        "impacts",
        "allowed_actions",
        "forbidden_actions",
        "contradiction_claims",
    ] {
        assert!(has(member), "fixture must exercise {member}");
    }

    let rendered = TestWorkspace::new("render-object-rendered");
    for node in &original {
        let id = node["id"].as_str().expect("id");
        // Page identity comes from the path (`objects.<id>`), never from the block.
        let out = rendered
            .root
            .join("docs/objects")
            .join(format!("{id}.adoc"));
        fs::create_dir_all(out.parent().expect("parent")).expect("mkdir");
        let output = render(&rendered, &with_schema(authored(node)), &out);
        assert!(
            output.status.success(),
            "render-object {id} failed\nstderr:\n{}",
            stderr(&output)
        );
    }
    let check = adoc_command()
        .current_dir(&rendered.root)
        .args(["check", "docs"])
        .output()
        .expect("adoc check runs");
    assert!(
        check.status.success(),
        "check failed\n{}{}",
        stdout(&check),
        stderr(&check)
    );
    let round = objects(&build_graph(&rendered, "dist"));

    assert_eq!(round.len(), original.len());
    for node in &original {
        let found = round
            .iter()
            .find(|candidate| candidate["id"] == node["id"])
            .unwrap_or_else(|| panic!("missing {}", node["id"]));
        assert_eq!(authored(found), authored(node), "node {}", node["id"]);
        assert_eq!(found["content_hash"], node["content_hash"]);
    }
}

#[test]
fn render_object_output_is_deterministic_and_exactly_one_block() {
    let workspace = TestWorkspace::new("render-object-determinism");
    let input = json!({
        "schema_version": "adoc.render_object.v0",
        "id": "t.claim", "kind": "claim", "status": "draft", "body": "Body.",
        "fields": {"owner": "me", "source": "report", "audience": "all"},
        "relations": {"depends_on": [], "supersedes": [], "related_to": []},
    });
    let first = workspace.root.join("a.adoc");
    let second = workspace.root.join("b.adoc");
    assert!(render(&workspace, &input, &first).status.success());
    assert!(render(&workspace, &input, &second).status.success());
    let text = fs::read_to_string(&first).expect("output");
    assert_eq!(text, fs::read_to_string(&second).expect("output"));
    assert!(text.starts_with("::claim t.claim\n"), "{text}");
    assert!(text.ends_with("--\nBody.\n::\n"), "{text}");
}

#[test]
fn render_object_refuses_bad_input_with_a_structured_error() {
    let workspace = TestWorkspace::new("render-object-refusals");
    let good = json!({
        "schema_version": "adoc.render_object.v0",
        "id": "t.claim", "kind": "claim", "status": "draft", "body": "Body.",
    });
    let cases = [
        (
            "unknown member",
            {
                let mut v = good.clone();
                v["surprise"] = json!(1);
                v
            },
            "io.artifact_malformed",
        ),
        (
            "wrong schema_version",
            {
                let mut v = good.clone();
                v["schema_version"] = json!("v1");
                v
            },
            "schema.unsupported_version",
        ),
        (
            "invalid kind",
            {
                let mut v = good.clone();
                v["kind"] = json!("bogus");
                v
            },
            "error[schema.",
        ),
        (
            "invalid status",
            {
                let mut v = good.clone();
                v["kind"] = json!("task");
                v["status"] = json!("bogus");
                v["fields"] = json!({"owner": "me"});
                v
            },
            "schema.task_invalid_status",
        ),
    ];
    for (label, input, expected) in cases {
        let out = workspace.root.join("refused.adoc");
        let _ = fs::remove_file(&out);
        let output = render(&workspace, &input, &out);
        assert!(!output.status.success(), "{label}: expected non-zero");
        assert!(
            stderr(&output).contains(expected),
            "{label}: stderr was {}",
            stderr(&output)
        );
        assert!(!out.exists(), "{label}: no output on refusal");
    }
}

fn claim() -> Value {
    json!({
        "schema_version": "adoc.render_object.v0",
        "id": "t.claim", "kind": "claim", "status": "draft", "body": "Body.",
    })
}

fn refused(workspace: &TestWorkspace, input: &Value) -> String {
    let out = workspace.root.join("refused.adoc");
    fs::write(&out, "stale").expect("stale output");
    let output = render(workspace, input, &out);
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(!out.exists(), "a refusal removes the stale output");
    stderr(&output)
}

// Issue 286 review repair 1: block syntax in id/kind/status must never emit a second object.
#[test]
fn render_object_refuses_block_injection_through_scalars() {
    let workspace = TestWorkspace::new("render-object-injection");
    let injected = "draft\n--\nFirst\n::\n\n::claim t.injected\nstatus: draft";
    for (member, value) in [
        ("status", injected),
        ("kind", "claim t.injected\nstatus: draft"),
        ("id", "t.claim\n--\nx\n::\n\n::claim t.injected"),
    ] {
        let mut input = claim();
        input[member] = json!(value);
        let text = refused(&workspace, &input);
        assert!(
            text.starts_with("error[render.member_not_representable] `"),
            "{member}: {text}"
        );
    }
}

// Review repair 2: a member the block cannot carry is refused, never changed.
#[test]
fn render_object_refuses_a_member_that_does_not_rebuild_equal() {
    let workspace = TestWorkspace::new("render-object-unrepresentable");
    let mut input = claim();
    input["evidence"] = json!([{"kind": "test", "value": "cargo test"}]);
    assert_eq!(
        refused(&workspace, &input),
        "error[render.member_not_representable] object `t.claim` does not rebuild to the input's authored members; a member is not representable for this kind or status\n  help: Fix or drop the named member in the render-object input; the block must rebuild to exactly the input's authored members.\n"
    );
}

// Review repair 3: nested objects are strict; a partial `relations` is accepted.
#[test]
fn render_object_nested_members_are_strict() {
    let workspace = TestWorkspace::new("render-object-nested");
    let mut input = claim();
    input["relations"] = json!({"surprise": ["secret"]});
    let text = refused(&workspace, &input);
    assert!(
        text.starts_with(
            "error[io.artifact_malformed] render-object input is invalid: unknown field `surprise`"
        ),
        "{text}"
    );
    assert!(!text.contains("docs.graph.json"), "{text}");
    let mut input = claim();
    input["evidence"] = json!([{"kind": "test", "value": "x", "surprise": 1}]);
    assert!(refused(&workspace, &input).contains("unknown field `surprise`"));

    let mut input = claim();
    input["relations"] = json!({"depends_on": ["a.b"]});
    let out = workspace.root.join("partial.adoc");
    assert!(render(&workspace, &input, &out).status.success());
}

// Review repairs 4-7: output ownership, registered codes, exact stderr, no synthetic spans.
#[test]
fn render_object_output_paths_and_diagnostics_are_honest() {
    let workspace = TestWorkspace::new("render-object-output");
    let input_path = workspace.write("same.json", &claim().to_string());
    let output = adoc_command()
        .arg("render-object")
        .arg("--input")
        .arg(&input_path)
        .arg("--out")
        .arg(&input_path)
        .output()
        .expect("runs");
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).starts_with("error: "),
        "{}",
        stderr(&output)
    );
    assert!(
        fs::read_to_string(&input_path)
            .expect("input kept")
            .contains("t.claim")
    );

    let missing = adoc_command()
        .arg("render-object")
        .arg("--input")
        .arg(workspace.root.join("absent.json"))
        .arg("--out")
        .arg(workspace.root.join("o.adoc"))
        .output()
        .expect("runs");
    assert_eq!(missing.status.code(), Some(2));
    assert!(
        stderr(&missing).contains("absent.json"),
        "{}",
        stderr(&missing)
    );
    assert!(!stderr(&missing).contains("io.artifact_unwritable"));

    let mut bad = claim();
    bad["kind"] = json!("task");
    bad["status"] = json!("bogus");
    bad["fields"] = json!({"owner": "me"});
    assert_eq!(
        refused(&workspace, &bad),
        "error[schema.task_invalid_status] task `t.claim` has invalid status `bogus`\n  object_id: t.claim\n  help: Valid task statuses are: open, done.\n"
    );
}
