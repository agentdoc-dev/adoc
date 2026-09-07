mod support;

use std::fs;
use std::process::{Command, Output};
use std::time::Instant;

use serde_json::{Value, json};
use support::{TestWorkspace, adoc_command};

const ARTIFACT: &str = "dist/docs.graph.json";
const CONFIG: &str = "version: 1\nmode: strict\ndocs_path: docs\nretrieval_policy:\n  audience: public\n  allowed_visibilities: [public, internal, restricted]\n  excluded_object_ids: [billing.excluded]\n";
const RELATIONS: [&str; 3] = ["depends_on", "supersedes", "related_to"];

fn object(id: &str, visibility: &str) -> Value {
    json!({
        "type": "knowledge_object", "id": id, "kind": "claim", "status": "draft",
        "content_hash": format!("sha256:{}", "0".repeat(64)),
        "body": "Credits ledger policy.", "page_id": "billing.page",
        "source_span": {"path": "docs/billing.adoc", "line": 1, "column": 1},
        "visibility": visibility, "fields": {"owner": "team-billing"},
        "relations": {"depends_on": [], "supersedes": [], "related_to": []}
    })
}

fn document(nodes: Vec<Value>, edges: Vec<Value>) -> Value {
    json!({"schema_version": "adoc.graph.v6", "repository_identity": null,
        "nodes": nodes, "edges": edges, "diagnostics": []})
}

fn run(workspace: &TestWorkspace, args: &[&str], format: &str) -> Output {
    run_cli(adoc_command(), workspace, args, format)
}

fn run_cli(mut command: Command, workspace: &TestWorkspace, args: &[&str], format: &str) -> Output {
    command
        .current_dir(&workspace.root)
        .env_remove("NO_COLOR")
        .args(args)
        .args([
            "--artifact",
            ARTIFACT,
            "--format",
            format,
            "--color",
            "always",
        ])
        .output()
        .expect("actual CLI runs")
}

#[test]
#[ignore = "serial release timing gate; requires ADOC_PARITY_TIMING_BINARY"]
fn coarse_hidden_presence_timing_preserves_complete_observable_parity() {
    let workspace = TestWorkspace::new("permission-parity-timing");
    workspace.write("agentdoc.config.yaml", CONFIG);
    let (mut present, absent) = graph_pair();
    // Keep the denied set equal in both policy modes: the ordinary matrix's
    // explicitly excluded public control becomes classified for timing only.
    present["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|node| node["id"] == "billing.excluded")
        .unwrap()["visibility"] = json!("internal");
    let present = present.to_string();
    let absent = absent.to_string();
    // Like the existing release performance harness, this gate is invoked
    // explicitly outside the parallel unit suite. Missing inputs fail rather
    // than silently treating an unmeasured run as a timing waiver.
    let binary = std::env::var_os("ADOC_PARITY_TIMING_BINARY")
        .expect("saved release binary required; run with --ignored --test-threads=1");
    eprintln!("timing executable: {binary:?}");
    // Calibration on this 2-public/3-hidden-node fixture measured 6.6–8.7 ms
    // process medians, <=0.26 ms paired median gaps, and <=2.68 ms control p90
    // noise. A fixed 1 ms median budget catches coarse (~12–15% of startup)
    // separation without asserting constant time. Never adapt it upward to
    // a slow implementation; control p90 >3 ms fails as an inconclusive run.
    // Not coverage of cold starts, unbounded corpora, fine statistical attacks,
    // or successful semantic/model loading (the semantic case is a refusal).
    const MAX_MEDIAN_GAP_MS: f64 = 1.0;
    const MAX_CONTROL_P90_MS: f64 = 3.0;
    let percentile = |values: &[f64], percent: usize| {
        let mut sorted = values.to_vec();
        sorted.sort_by(f64::total_cmp);
        sorted[sorted.len() * percent / 100]
    };
    let mut failures = Vec::new();
    for policy_mode in ["default_public", "explicit_public"] {
        if policy_mode == "default_public" {
            fs::remove_file(workspace.root.join("agentdoc.config.yaml")).unwrap();
        } else {
            workspace.write("agentdoc.config.yaml", CONFIG);
        }
        for (args, format, expected_exit) in [
            (vec!["search", "credits", "--lexical"], "plain", 0),
            (vec!["search", "billing.internal", "--lexical"], "json", 0),
            (vec!["search", "credits"], "styled", 0),
            (vec!["search", "credits", "--semantic"], "json", 2),
            (
                vec![
                    "search",
                    "credits",
                    "--lexical",
                    "--related-to",
                    "billing.root",
                ],
                "json",
                0,
            ),
            (vec!["why", "billing.root"], "json", 0),
            (vec!["why", "billing.root"], "plain", 0),
            (vec!["why", "billing.root"], "styled", 0),
            (vec!["why", "billing.internal"], "styled", 3),
            (
                vec!["graph", "billing.root", "--direction", "incoming"],
                "plain",
                0,
            ),
            (
                vec!["graph", "billing.root", "--direction", "outgoing"],
                "json",
                0,
            ),
            (
                vec!["graph", "billing.root", "--direction", "both"],
                "styled",
                0,
            ),
            (vec!["stale"], "json", 0),
            (vec!["contradictions", "--all"], "plain", 0),
            (vec!["impacted-by", "src/billing.rs"], "markdown", 0),
        ] {
            let measure = |contents: &str| {
                // Fixture encoding/writes are outside the clock; the measured
                // interval includes process launch, artifact read, policy/filter,
                // query, presentation, captured stdout/stderr and process exit.
                workspace.write(ARTIFACT, contents);
                let mut command = Command::new(&binary);
                command.env("ADOC_TEST_EMBEDDING_PROVIDER", "deterministic");
                let started = Instant::now();
                let output = run_cli(command, &workspace, &args, format);
                (started.elapsed().as_secs_f64() * 1_000.0, output)
            };
            let same_output = |left: &Output, right: &Output| {
                assert_eq!(
                    left.status.code(),
                    Some(expected_exit),
                    "{args:?}: {left:?}"
                );
                assert_eq!(left.status, right.status, "{args:?} {format}");
                assert_eq!(left.stdout, right.stdout, "{args:?} {format}");
                assert_eq!(left.stderr, right.stderr, "{args:?} {format}");
            };
            // Warm both worlds, then interleave absent/absent controls with 31
            // hidden/absent pairs. Alternate order to balance cache/scheduler drift.
            for _ in 0..3 {
                same_output(&measure(&present).1, &measure(&absent).1);
            }
            let mut control_deltas = Vec::new();
            let mut paired_deltas = Vec::new();
            let mut absent_times = Vec::new();
            let mut present_times = Vec::new();
            for pair in 0..31 {
                let (control_first, first_output) = measure(&absent);
                let (control_second, second_output) = measure(&absent);
                same_output(&first_output, &second_output);
                control_deltas.push((control_first - control_second).abs());
                let ((present_time, present_output), (absent_time, absent_output)) =
                    if pair % 2 == 0 {
                        (measure(&present), measure(&absent))
                    } else {
                        let absent_run = measure(&absent);
                        (measure(&present), absent_run)
                    };
                same_output(&present_output, &absent_output);
                paired_deltas.push(present_time - absent_time);
                present_times.push(present_time);
                absent_times.push(absent_time);
            }
            eprintln!(
                "timing {policy_mode} {args:?} {format}: absent_median_ms={:.3} present_median_ms={:.3} paired_median_ms={:.3} control_p90_abs_ms={:.3}",
                percentile(&absent_times, 50),
                percentile(&present_times, 50),
                percentile(&paired_deltas, 50),
                percentile(&control_deltas, 90)
            );
            if percentile(&paired_deltas, 50).abs() > MAX_MEDIAN_GAP_MS {
                failures.push(format!("{policy_mode} {args:?} {format}: hidden/absent paired median gap exceeds {MAX_MEDIAN_GAP_MS} ms"));
            }
            if percentile(&control_deltas, 90) > MAX_CONTROL_P90_MS {
                failures.push(format!("{policy_mode} {args:?} {format}: control noise exceeds {MAX_CONTROL_P90_MS} ms; timing evidence inconclusive, rerun serially on a quiet host"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn parity(
    workspace: &TestWorkspace,
    present: &Value,
    absent: &Value,
    args: &[&str],
    format: &str,
) -> Output {
    // Same workspace, arguments, policy and path. Compare raw output bytes:
    // no ID replacement, JSON projection, ANSI stripping or path normalization.
    workspace.write(ARTIFACT, &present.to_string());
    let hidden = run(workspace, args, format);
    workspace.write(ARTIFACT, &absent.to_string());
    let missing = run(workspace, args, format);
    assert_eq!(
        hidden.status, missing.status,
        "status: {args:?} {format}; {hidden:?} vs {missing:?}"
    );
    assert!(
        hidden.stdout == missing.stdout,
        "stdout: {args:?} {format}\npresent: {}\nabsent: {}",
        String::from_utf8_lossy(&hidden.stdout),
        String::from_utf8_lossy(&missing.stdout)
    );
    assert!(
        hidden.stderr == missing.stderr,
        "stderr: {args:?} {format}\npresent: {}\nabsent: {}",
        String::from_utf8_lossy(&hidden.stderr),
        String::from_utf8_lossy(&missing.stderr)
    );
    missing
}

#[test]
fn malformed_decoder_location_cannot_reveal_a_hidden_record() {
    let workspace = TestWorkspace::new("permission-parity-decoder");
    workspace.write("agentdoc.config.yaml", CONFIG);
    let mut malformed = object("billing.visible", "public");
    malformed["body"] = json!(42);
    let present = document(
        vec![object("billing.hidden", "internal"), malformed.clone()],
        vec![],
    );
    let absent = document(vec![malformed], vec![]);
    // Both artifacts have the same public structural error. The extra hidden
    // record must not change exposed decoder offsets. No carried diagnostics.
    for command in ["graph", "why", "search"] {
        for format in ["json", "plain", "styled"] {
            let mut args = vec![command, "billing.visible"];
            if command == "search" {
                args.push("--lexical");
            }
            let output = parity(&workspace, &present, &absent, &args, format);
            assert_eq!(output.status.code(), Some(2));
        }
    }
}

#[test]
fn graph_and_signal_decoder_errors_do_not_serialize_secret_values() {
    let workspace = TestWorkspace::new("permission-parity-secret-decoder");
    workspace.write("agentdoc.config.yaml", CONFIG);
    let mut differences = Vec::new();
    for malformed_field in ["nodes", "edge_relation"] {
        let malformed = |value: &str| {
            let mut graph = document(vec![object("billing.visible", "public")], vec![]);
            if malformed_field == "nodes" {
                graph["nodes"] = json!(value);
            } else {
                graph["edges"] = json!([{
                    "kind": "relation", "source": "billing.visible",
                    "target": "billing.visible", "relation": value
                }]);
            }
            graph
        };
        // Both payloads are malformed, with no carried source diagnostics.
        // A decoder's unknown enum/string value is untrusted even before its
        // enclosing object can be classified. Changing it cannot change the
        // complete refusal exposed by a graph or signal command.
        let secret = malformed("private-sentinel billing.hidden payroll amount");
        let harmless = malformed("invalid-placeholder");
        workspace.write(ARTIFACT, &secret.to_string());
        let retrieval = run(&workspace, &["why", "billing.visible"], "json");
        assert_eq!(retrieval.status.code(), Some(2));
        let retrieval: Value = serde_json::from_slice(&retrieval.stdout).unwrap();
        let safe_diagnostics = &retrieval["diagnostics"];
        assert_eq!(safe_diagnostics[0]["code"], "io.artifact_malformed");
        for args in [
            vec!["graph", "billing.visible"],
            vec!["stale"],
            vec!["contradictions", "--all"],
            vec!["impacted-by", "src/billing.rs"],
        ] {
            let formats: &[&str] = if args[0] == "impacted-by" {
                &["json", "plain", "styled", "markdown"]
            } else {
                &["json", "plain", "styled"]
            };
            for &format in formats {
                workspace.write(ARTIFACT, &secret.to_string());
                let exposed = run(&workspace, &args, format);
                workspace.write(ARTIFACT, &harmless.to_string());
                let control = run(&workspace, &args, format);
                assert_eq!(exposed.status.code(), Some(2), "{args:?} {format}");
                assert_eq!(control.status.code(), Some(2), "{args:?} {format}");
                if exposed.status != control.status
                    || exposed.stdout != control.stdout
                    || exposed.stderr != control.stderr
                {
                    differences.push(format!("{malformed_field}: {args:?} {format} exposes payload-dependent output\nstdout: {}\nstderr: {}",
                        String::from_utf8_lossy(&exposed.stdout), String::from_utf8_lossy(&exposed.stderr)));
                }
                if format == "json" {
                    let envelope: Value = serde_json::from_slice(&exposed.stdout).unwrap();
                    if envelope["diagnostics"] != *safe_diagnostics {
                        differences.push(format!("{malformed_field}: {args:?} does not preserve retrieval's safe typed diagnostic/help"));
                    }
                }
            }
        }
    }
    assert!(differences.is_empty(), "{}", differences.join("\n"));
}

fn graph_pair() -> (Value, Value) {
    let mut root = object("billing.root", "public");
    for relation in RELATIONS {
        root["relations"][relation] = json!(["billing.peer"]);
    }
    root["evidence"] = json!([{"kind": "source_code", "value": "src/billing.rs"}]);
    let mut peer = object("billing.peer", "public");
    peer["status"] = json!("deprecated");
    let mut edges = Vec::new();
    for relation in RELATIONS {
        for (source, target) in [
            ("billing.root", "billing.peer"),
            ("billing.peer", "billing.root"),
        ] {
            edges.push(json!({"kind": "relation", "source": source, "target": target, "relation": relation}));
        }
    }
    let absent = document(vec![root, peer], edges.clone());
    let mut present = absent.clone();
    for (id, visibility) in [
        ("billing.internal", "internal"),
        ("billing.restricted", "restricted"),
        ("billing.excluded", "public"),
    ] {
        present["nodes"]
            .as_array_mut()
            .unwrap()
            .push(object(id, visibility));
        // Graph edges and source relation lists are separately serialized
        // artifact surfaces. Exercise edge admission without relying on source
        // carrier withholding to hide a public traversal root.
        for relation in RELATIONS {
            for (source, target) in [("billing.root", id), (id, "billing.root")] {
                edges.push(json!({"kind": "relation", "source": source, "target": target, "relation": relation}));
            }
        }
    }
    present["edges"] = json!(edges);
    (present, absent)
}

#[test]
fn adversarial_cli_matrix_compares_complete_observables() {
    let workspace = TestWorkspace::new("permission-parity-matrix");
    workspace.write("agentdoc.config.yaml", CONFIG);
    let (present, absent) = graph_pair();
    let mut attempts = 0;
    for format in ["json", "plain", "styled"] {
        for direction in ["incoming", "outgoing", "both"] {
            for relation in RELATIONS {
                for root in [
                    "billing.root",
                    "billing.internal",
                    "billing.restricted",
                    "billing.excluded",
                ] {
                    let output = parity(
                        &workspace,
                        &present,
                        &absent,
                        &[
                            "graph",
                            root,
                            "--direction",
                            direction,
                            "--relation",
                            relation,
                        ],
                        format,
                    );
                    assert_eq!(
                        output.status.code(),
                        Some(if root == "billing.root" { 0 } else { 3 })
                    );
                    if root == "billing.root" && format == "json" {
                        let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
                        assert_eq!(envelope["nodes"].as_array().unwrap().len(), 2);
                        assert_eq!(envelope["edges"].as_array().unwrap().len(), 2);
                    }
                    attempts += 1;
                }
                let output = parity(
                    &workspace,
                    &present,
                    &absent,
                    &[
                        "search",
                        "credits",
                        "--lexical",
                        "--related-to",
                        "billing.root",
                        "--relation",
                        relation,
                        "--direction",
                        direction,
                    ],
                    format,
                );
                assert_eq!(output.status.code(), Some(0));
                attempts += 1;
            }
        }
        for query in [
            "credits",
            "billing.internal",
            "billing.restricted",
            "billing.excluded",
            "billing.",
        ] {
            let output = parity(
                &workspace,
                &present,
                &absent,
                &["search", query, "--lexical", "--top", "1"],
                format,
            );
            assert_eq!(output.status.code(), Some(0));
            attempts += 1;
        }
        // Missing/excluded roots have no elapsed-time footer, so even plain
        // and styled error paths have an exact whole-response oracle.
        for root in [
            "billing.internal",
            "billing.restricted",
            "billing.excluded",
            "billing.missing",
            "bad",
        ] {
            let output = parity(&workspace, &present, &absent, &["why", root], format);
            assert_eq!(
                output.status.code(),
                Some(if root == "bad" { 1 } else { 3 })
            );
            attempts += 1;
        }
    }
    // Successful JSON why includes full source/evidence/citation metadata.
    // Text why must preserve the same whole-response parity without stripping
    // its footer. Actual execution timing has a separate coarse assertion.
    let why = parity(
        &workspace,
        &present,
        &absent,
        &["why", "billing.root"],
        "json",
    );
    let why: Value = serde_json::from_slice(&why.stdout).unwrap();
    assert_eq!(why["records"].as_array().unwrap().len(), 1);
    assert_eq!(why["records"][0]["id"], "billing.root");
    assert_eq!(
        why["records"][0]["relations"]["depends_on"],
        json!(["billing.peer"])
    );
    attempts += 1;
    for format in ["plain", "styled"] {
        let output = parity(
            &workspace,
            &present,
            &absent,
            &["why", "billing.root"],
            format,
        );
        assert_eq!(output.status.code(), Some(0));
        let stdout = String::from_utf8(output.stdout).unwrap();
        let visible = strip_ansi_escapes::strip_str(&stdout);
        assert_eq!(
            visible.lines().last(),
            Some("✓ rendered from docs.graph.json")
        );
        attempts += 1;
    }
    assert_eq!(
        attempts, 168,
        "distinct command/query/relation/direction/presenter pairs"
    );
    eprintln!("verified {attempts} complete stdout/stderr/status parity pairs");

    // Two authority controls: removing policy restores public exclusions;
    // an explicitly restricted audience restores classified objects.
    workspace.write(ARTIFACT, &present.to_string());
    fs::remove_file(workspace.root.join("agentdoc.config.yaml")).unwrap();
    let restored = run(&workspace, &["graph", "billing.excluded"], "json");
    assert_eq!(restored.status.code(), Some(0));
    let restored: Value = serde_json::from_slice(&restored.stdout).unwrap();
    assert!(
        restored["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["id"] == "billing.excluded")
    );
    workspace.write(
        "agentdoc.config.yaml",
        &CONFIG
            .replace("audience: public", "audience: restricted")
            .replace("[billing.excluded]", "[]"),
    );
    for id in ["billing.internal", "billing.restricted"] {
        let restored = run(&workspace, &["graph", id], "json");
        assert_eq!(restored.status.code(), Some(0));
        let restored: Value = serde_json::from_slice(&restored.stdout).unwrap();
        assert!(
            restored["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|node| node["id"] == id)
        );
    }
}

#[test]
fn hidden_reverse_citations_derived_statuses_and_signal_counts_match_absence() {
    let workspace = TestWorkspace::new("permission-parity-signals");
    workspace.write("agentdoc.config.yaml", CONFIG);
    let (mut present, absent) = graph_pair();
    let mut contradiction = object("billing.hidden-conflict", "internal");
    contradiction["kind"] = json!("contradiction");
    contradiction["status"] = json!("open");
    contradiction["severity"] = json!("high");
    contradiction["contradiction_claims"] = json!(["billing.peer"]);
    let mut question = object("billing.hidden-question", "restricted");
    question["kind"] = json!("question");
    question["status"] = json!("answered");
    question["fields"]["resolved_by"] = json!("billing.root");
    let mut stale = object("billing.hidden-expired", "internal");
    stale["status"] = json!("verified");
    stale["fields"]["expires_at"] = json!("2000-01-01");
    stale["impacts"] = json!(["src/billing.rs"]);
    present["nodes"]
        .as_array_mut()
        .unwrap()
        .extend([contradiction, question, stale]);
    present["nodes"][1]["effective_status"] = json!("contradicted");
    present["nodes"][1]["effective_reason"] = json!("contradiction:billing.hidden-conflict");

    for format in ["json", "plain", "styled"] {
        for args in [
            vec!["stale"],
            vec!["contradictions", "--all"],
            vec!["impacted-by", "src/billing.rs"],
            vec!["search", "credits", "--lexical"],
        ] {
            let output = parity(&workspace, &present, &absent, &args, format);
            assert_eq!(output.status.code(), Some(0));
        }
    }
    let output = parity(
        &workspace,
        &present,
        &absent,
        &["why", "billing.root"],
        "json",
    );
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["records"].as_array().unwrap().len(), 1);
    assert!(
        envelope["records"][0]
            .get("resolved_questions")
            .is_none_or(|ids| ids == &json!([]))
    );
    // Ensure each omitted signal really exists when the audience admits it.
    workspace.write(ARTIFACT, &present.to_string());
    workspace.write(
        "agentdoc.config.yaml",
        &CONFIG.replace("audience: public", "audience: restricted"),
    );
    for (args, field, id) in [
        (vec!["stale"], "records", "billing.hidden-expired"),
        (
            vec!["contradictions", "--all"],
            "contradictions",
            "billing.hidden-conflict",
        ),
        (
            vec!["impacted-by", "src/billing.rs"],
            "impacted",
            "billing.hidden-expired",
        ),
    ] {
        let output = run(&workspace, &args, "json");
        assert_eq!(output.status.code(), Some(0));
        let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(
            envelope[field]
                .as_array()
                .unwrap()
                .iter()
                .any(|record| record["id"] == id),
            "{envelope}"
        );
    }
    let output = run(&workspace, &["why", "billing.root"], "json");
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        envelope["records"][0]["resolved_questions"],
        json!(["billing.hidden-question"])
    );
}

#[test]
fn source_citation_carriers_are_withheld_whole_instead_of_serializing_hidden_metadata() {
    let workspace = TestWorkspace::new("permission-parity-carriers");
    workspace.write("agentdoc.config.yaml", CONFIG);
    for carrier in [
        "body",
        "owner",
        "evidence_reference",
        "evidence_value",
        "depends_on",
        "supersedes",
        "related_to",
        "source_path",
        "resolved_by",
    ] {
        let safe = object("billing.safe", "public");
        let mut tainted = object("billing.carrier", "public");
        match carrier {
            "body" => tainted["body"] = json!("Credits described by billing.hidden."),
            "owner" => tainted["fields"]["owner"] = json!("billing.hidden"),
            "evidence_reference" => {
                tainted["evidence"] =
                    json!([{"kind": "source_code", "reference": "billing.hidden"}])
            }
            "evidence_value" => {
                tainted["evidence"] =
                    json!([{"kind": "source_code", "value": "See billing.hidden"}])
            }
            "source_path" => tainted["source_span"]["path"] = json!("docs/billing.hidden.adoc"),
            "resolved_by" => {
                tainted["kind"] = json!("question");
                tainted["status"] = json!("answered");
                tainted["fields"]["resolved_by"] = json!("billing.hidden");
            }
            relation => tainted["relations"][relation] = json!(["billing.hidden"]),
        }
        // The established T1 contract removes entire source carriers (including
        // their hashes). The absent corpus therefore omits that carrier too.
        let present = document(
            vec![safe.clone(), tainted, object("billing.hidden", "internal")],
            vec![],
        );
        let absent = document(vec![safe], vec![]);
        for format in ["json", "plain", "styled"] {
            let output = parity(
                &workspace,
                &present,
                &absent,
                &["search", "credits", "--lexical"],
                format,
            );
            assert_eq!(output.status.code(), Some(0), "{carrier}");
            for command in ["why", "graph"] {
                let output = parity(
                    &workspace,
                    &present,
                    &absent,
                    &[command, "billing.carrier"],
                    format,
                );
                assert_eq!(output.status.code(), Some(3), "{carrier}");
            }
        }
        workspace.write(ARTIFACT, &present.to_string());
        workspace.write(
            "agentdoc.config.yaml",
            &CONFIG.replace("audience: public", "audience: restricted"),
        );
        let allowed = run(&workspace, &["why", "billing.carrier"], "json");
        assert_eq!(allowed.status.code(), Some(0), "{carrier}: {allowed:?}");
        let allowed: Value = serde_json::from_slice(&allowed.stdout).unwrap();
        assert_eq!(allowed["records"][0]["id"], "billing.carrier");
        workspace.write("agentdoc.config.yaml", CONFIG);
    }
}

#[test]
fn matching_manifest_and_stale_hidden_vector_have_complete_index_parity() {
    let workspace = TestWorkspace::new("permission-parity-vectors");
    // This adversarial read fixture needs a genuinely authorized wide index.
    workspace.write(
        "agentdoc.config.yaml",
        &CONFIG.replace("audience: public", "audience: internal"),
    );
    let public = "# Billing @doc(billing.page)\n\n::claim billing.visible\nstatus: draft\nvisibility: public\n--\nCredits flow through the public ledger.\n::\n";
    let hidden = "\n::claim billing.hidden\nstatus: draft\nvisibility: internal\n--\nPrivate credits and secret ledger policy.\n::\n";
    let build = |source: &str| {
        workspace.write("docs/billing.adoc", source);
        let output = adoc_command()
            .current_dir(&workspace.root)
            .args(["build", "docs", "--out", "dist"])
            .output()
            .expect("real deterministic embedding build runs");
        assert!(output.status.success(), "{output:?}");
        let graph: Value =
            serde_json::from_str(&fs::read_to_string(workspace.root.join(ARTIFACT)).unwrap())
                .unwrap();
        let search: Value = serde_json::from_str(
            &fs::read_to_string(workspace.root.join("dist/docs.search.json")).unwrap(),
        )
        .unwrap();
        (graph, search)
    };
    let (present, mut index) = build(&format!("{public}{hidden}"));
    let (absent, _) = build(public);
    workspace.write("agentdoc.config.yaml", CONFIG);
    let entry = index["embeddings"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|entry| entry["id"] == "billing.hidden")
        .expect("real hidden embedding exists");
    entry["content_hash"] = json!(format!("sha256:{}", "f".repeat(64)));
    // Keep the *same* real index in both worlds. Its manifest matches the
    // present graph; only a hidden carrier is absent in the other graph.
    // All surviving vectors still have their genuine composition bindings.
    workspace.write("dist/docs.search.json", &index.to_string());
    for format in ["json", "plain", "styled"] {
        for mode in ["--lexical", "--semantic", "hybrid"] {
            for query in ["credits", "billing.hidden", "billing."] {
                let mut args = vec![
                    "search",
                    query,
                    "--search-artifact",
                    "dist/docs.search.json",
                    "--top",
                    "1",
                    "--objects-only",
                ];
                if mode != "hybrid" {
                    args.push(mode);
                }
                let output = parity(&workspace, &present, &absent, &args, format);
                assert_eq!(output.status.code(), Some(0));
                if format == "json" {
                    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
                    assert_eq!(
                        envelope["diagnostics"],
                        json!([]),
                        "hidden-only drift cannot add a warning"
                    );
                    assert!(
                        envelope["records"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .all(|entry| entry["id"] == "billing.visible")
                    );
                    if mode == "--semantic" {
                        assert!(!envelope["records"].as_array().unwrap().is_empty());
                        assert!(envelope["records"][0]["match"]["vector_rank"].is_number());
                    }
                }
            }
        }
    }
}
