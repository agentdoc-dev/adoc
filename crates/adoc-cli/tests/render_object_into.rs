//! Issue 291 (M4.2.T0): `adoc render-object --into FILE [--append]` patches one block in place.

use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use crate::support::{TestWorkspace, adoc_command, stderr, stdout};

const FILE: &str = "# Page @doc(page.one)\n\nProse before.\n\n::claim t.first\nstatus: draft\n--\nFirst body.\n::\n\nProse between.\n\n::claim t.second\nstatus: draft\nowner: me\n--\nSecond body.\n::\n\nProse after.\n";

fn input(id: &str, body: &str) -> Value {
    json!({
        "schema_version": "adoc.render_object.v0",
        "id": id, "kind": "claim", "status": "draft", "body": body,
        "fields": {"owner": "you"},
    })
}

fn run(
    workspace: &TestWorkspace,
    input: &Value,
    into: &Path,
    append: bool,
) -> std::process::Output {
    let input_path = workspace.write("input.json", &input.to_string());
    let mut command = adoc_command();
    command
        .current_dir(&workspace.root)
        .arg("render-object")
        .arg("--input")
        .arg(input_path)
        .arg("--into")
        .arg(into)
        .arg("--out")
        .arg(workspace.root.join("out.adoc"));
    if append {
        command.arg("--append");
    }
    command.output().expect("adoc render-object runs")
}

fn refused(workspace: &TestWorkspace, input: &Value, bytes: &[u8], append: bool) -> String {
    let into = workspace.root.join("in.adoc");
    fs::write(&into, bytes).expect("into");
    let out = workspace.root.join("out.adoc");
    fs::write(&out, "stale").expect("stale");
    let output = run(workspace, input, &into, append);
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert_eq!(stdout(&output), "", "stdout is silent");
    assert!(!out.exists(), "nothing written on refusal");
    assert_eq!(fs::read(&into).expect("into kept"), bytes);
    stderr(&output)
}

fn patched(file: &str, input: &Value, append: bool) -> String {
    let workspace = TestWorkspace::new("render-into");
    let into = workspace.write("in.adoc", file);
    let output = run(&workspace, input, &into, append);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "");
    assert_eq!(
        fs::read_to_string(&into).expect("into"),
        file,
        "into is not modified"
    );
    fs::read_to_string(workspace.root.join("out.adoc")).expect("out")
}

const SECOND_PATCHED: &str = "::claim t.second\nowner: you\nstatus: draft\n--\nNew body.\n::";

#[test]
fn replaces_only_the_block_span_and_keeps_prose_and_other_blocks() {
    let text = patched(FILE, &input("t.second", "New body."), false);
    let expected = format!(
        "{}{SECOND_PATCHED}{}",
        &FILE[..FILE.find("::claim t.second").unwrap()],
        &FILE[FILE.find("Second body.\n::").unwrap() + "Second body.\n::".len()..]
    );
    assert_eq!(text, expected);
    // Same call twice: deterministic.
    assert_eq!(text, patched(FILE, &input("t.second", "New body."), false));
}

#[test]
fn crlf_file_stays_crlf() {
    let crlf = FILE.replace('\n', "\r\n");
    let text = patched(&crlf, &input("t.first", "Line one.\nLine two."), false);
    assert_eq!(
        text.matches('\n').count(),
        text.matches("\r\n").count(),
        "{text:?}"
    );
    assert!(text.contains("::claim t.first\r\nowner: you\r\nstatus: draft\r\n--\r\nLine one.\r\nLine two.\r\n::\r\n"), "{text:?}");
    assert!(text.ends_with("::\r\n\r\nProse between.\r\n\r\n::claim t.second\r\nstatus: draft\r\nowner: me\r\n--\r\nSecond body.\r\n::\r\n\r\nProse after.\r\n"), "{text:?}");
}

#[test]
fn refuses_each_mismatch_and_bound_with_its_own_code() {
    let workspace = TestWorkspace::new("render-into-refusals");
    let text = refused(&workspace, &input("t.absent", "B."), FILE.as_bytes(), false);
    assert!(text.starts_with("error[render.into_id_absent] "), "{text}");
    let text = refused(&workspace, &input("t.first", "B."), FILE.as_bytes(), true);
    assert!(text.starts_with("error[render.into_id_present] "), "{text}");
    let duplicate = format!("{FILE}\n::claim t.first\nstatus: draft\n--\nAgain.\n::\n");
    for append in [false, true] {
        let text = refused(
            &workspace,
            &input("t.first", "B."),
            duplicate.as_bytes(),
            append,
        );
        assert!(
            text.starts_with("error[render.into_id_duplicate] "),
            "{text}"
        );
    }
    // Review of issue 291: a file that does not parse cleanly is never patched.
    let unclosed = "# Page @doc(page.one)\n\n::claim t.first\nstatus: draft\n--\nBody.\n";
    let fence = "# Page @doc(page.one)\n\n```\nunclosed fence\n";
    for (bytes, id, append) in [
        (unclosed, "t.first", false),
        (unclosed, "t.first", true),
        (unclosed, "t.other", true),
        (fence, "t.other", true),
    ] {
        let text = refused(&workspace, &input(id, "B."), bytes.as_bytes(), append);
        assert!(
            text.starts_with("error[render.into_file_unparsable] "),
            "{text}"
        );
        assert!(!text.contains("adoc.render_object.v0"), "{text}");
    }
    let text = refused(
        &workspace,
        &input("t.first", "B."),
        &[0xff, 0xfe, b'\n'],
        false,
    );
    assert!(
        text.starts_with("error[render.into_file_unparsable] "),
        "{text}"
    );
    assert!(!text.contains("adoc.render_object.v0"), "{text}");
    let big = vec![b'x'; 1024 * 1024 + 1];
    let text = refused(&workspace, &input("t.first", "B."), &big, false);
    assert!(
        text.starts_with("error[render.into_file_too_large] "),
        "{text}"
    );
    let mut oversized = input("t.first", "B.");
    oversized["body"] = json!("x".repeat(1024 * 1024 + 1));
    let text = refused(&workspace, &oversized, FILE.as_bytes(), false);
    assert!(text.starts_with("error[io.artifact_malformed] "), "{text}");
    // A bad document is still refused the same way with --into.
    let mut bad = input("t.first", "B.");
    bad["kind"] = json!("bogus");
    assert!(refused(&workspace, &bad, FILE.as_bytes(), false).starts_with("error[schema."));
}

fn build_nodes(workspace: &TestWorkspace) -> Vec<Value> {
    let check = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "docs"])
        .output()
        .expect("check runs");
    assert!(
        check.status.success(),
        "{}{}",
        stdout(&check),
        stderr(&check)
    );
    let build = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "docs", "--out", "dist", "--no-embeddings"])
        .output()
        .expect("build runs");
    assert!(build.status.success(), "{}", stderr(&build));
    let graph: Value = serde_json::from_str(
        &fs::read_to_string(workspace.root.join("dist/docs.graph.json")).expect("graph"),
    )
    .expect("json");
    graph["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .filter(|n| n["type"] == "knowledge_object")
        .cloned()
        .collect()
}

fn authored(node: &Value) -> Value {
    let mut map = node.as_object().expect("object").clone();
    for member in [
        "type",
        "content_hash",
        "page_id",
        "source_span",
        "source_binding",
        "effective_status",
        "effective_reason",
        "evidence_quality",
    ] {
        map.remove(member);
    }
    Value::Object(map)
}

fn round_trip(file: &str, input: &Value, append: bool) {
    let workspace = TestWorkspace::new("render-into-roundtrip");
    let into = workspace.write("docs/page.adoc", file);
    let output = run(&workspace, input, &into, append);
    assert!(output.status.success(), "{}", stderr(&output));
    fs::copy(workspace.root.join("out.adoc"), &into).expect("apply");
    fs::remove_file(workspace.root.join("out.adoc")).expect("rm");
    let nodes = build_nodes(&workspace);
    let node = nodes.iter().find(|n| n["id"] == input["id"]).expect("node");
    let mut expected = input.clone();
    expected.as_object_mut().unwrap().remove("schema_version");
    // Compare on every member the input authors.
    for (key, value) in expected.as_object().unwrap() {
        assert_eq!(&authored(node)[key], value, "member {key}");
    }
    assert_eq!(nodes.len(), if append { 3 } else { 2 });
}

#[test]
fn patched_block_round_trips_through_check_and_build() {
    round_trip(FILE, &input("t.second", "New body."), false);
}

#[test]
fn append_adds_an_absent_block_at_the_end_and_round_trips() {
    round_trip(FILE, &input("t.third", "Third body."), true);
    let text = patched(FILE, &input("t.third", "Third body."), true);
    assert_eq!(
        text,
        format!("{FILE}\n::claim t.third\nowner: you\nstatus: draft\n--\nThird body.\n::\n")
    );
}
