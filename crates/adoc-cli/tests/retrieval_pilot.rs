mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde::Deserialize;
use serde_json::Value;
use support::TestWorkspace;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cli crate has workspace parent")
        .parent()
        .expect("workspace has repo root")
        .to_path_buf()
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum RetrievalMode {
    #[default]
    Hybrid,
    Lexical,
    Semantic,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum CaseFormat {
    #[default]
    Json,
    Plain,
}

/// V1.7.3: which record scope a case searches. `ObjectsOnly` keeps the
/// pre-V1.7 Knowledge-Object sequences reproducible; `Blended` exercises the
/// honest KO/prose blend; `ProseOnly` pins prose-scoped retrieval.
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum CaseScope {
    #[default]
    ObjectsOnly,
    Blended,
    ProseOnly,
}

#[derive(Debug, Clone, Copy)]
enum EmbeddingBackend {
    InMemory,
    #[cfg(feature = "fastembed-it")]
    FastEmbed,
}

impl EmbeddingBackend {
    fn configure(self, command: &mut Command) {
        match self {
            Self::InMemory => {
                command.env("ADOC_TEST_EMBEDDING_PROVIDER", "deterministic");
            }
            #[cfg(feature = "fastembed-it")]
            Self::FastEmbed => {
                command.env_remove("ADOC_TEST_EMBEDDING_PROVIDER");
            }
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct CaseFilters {
    kind: Option<String>,
    status: Option<String>,
    owner: Option<String>,
    source_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetrievalCase {
    #[serde(default)]
    name: String,
    query: String,
    #[serde(default)]
    mode: RetrievalMode,
    #[serde(default)]
    scope: CaseScope,
    #[serde(default)]
    format: CaseFormat,
    #[serde(default)]
    filters: CaseFilters,
    #[serde(default)]
    expected_ids: Vec<String>,
    #[serde(default)]
    expected_diagnostics: Vec<String>,
    #[serde(default)]
    expected_evidence: BTreeMap<String, String>,
    #[serde(default)]
    expect_stdout_contains: Option<String>,
    #[serde(default)]
    expected_exit: i32,
    #[serde(default = "default_must_appear_in_top")]
    must_appear_in_top: usize,
    /// V1.7.3: the `--top` requested from the CLI. Defaults to
    /// `must_appear_in_top`; blended cases widen it (hybrid RRF fuses the
    /// per-mode top-k lists, so `--top 1` would fuse two singletons instead
    /// of ranking a realistic pool) while still pinning the expected rank.
    #[serde(default)]
    top: Option<usize>,
}

struct PilotBuild {
    _workspace: TestWorkspace,
    artifact_path: PathBuf,
    search_artifact_path: PathBuf,
    graph_json: Value,
}

fn default_must_appear_in_top() -> usize {
    5
}

/// V1.7.3: the per-pilot retrieval-set parameters — the pilot to build, its
/// golden query set, and the benchmark-size window the set must stay inside.
struct PilotRetrievalSet {
    pilot_dir: &'static str,
    case_count: std::ops::RangeInclusive<usize>,
}

const BILLING_PILOT_SET: PilotRetrievalSet = PilotRetrievalSet {
    pilot_dir: "billing-pilot",
    case_count: 15..=25,
};

const MARKDOWN_PILOT_SET: PilotRetrievalSet = PilotRetrievalSet {
    pilot_dir: "markdown-pilot",
    case_count: 8..=20,
};

// The only authorized cross-revision difference on untouched pilot bytes.
// Baseline identities belong in the run report, not host-specific test constants.
fn paired_output_compatibility(
    pilot: &str,
    carried: &Value,
    baseline: &Output,
    candidate: &Output,
) -> Result<&'static str, &'static str> {
    if baseline.status != candidate.status || baseline.stderr != candidate.stderr {
        return Err("status or stderr changed");
    }
    let legacy: Value =
        serde_json::from_slice(&baseline.stdout).map_err(|_| "invalid baseline JSON")?;
    let current: Value =
        serde_json::from_slice(&candidate.stdout).map_err(|_| "invalid candidate JSON")?;
    let diagnostics = legacy["diagnostics"]
        .as_array()
        .ok_or("missing baseline diagnostics")?;
    if current["diagnostics"] != serde_json::json!([]) {
        return Err("candidate diagnostics are not empty");
    }
    if diagnostics.is_empty() {
        return if baseline.stdout == candidate.stdout {
            Ok("none")
        } else {
            Err("output bytes changed without carried diagnostics")
        };
    }
    if pilot != "markdown-pilot" || &legacy["diagnostics"] != carried {
        return Err("baseline diagnostics differ from the complete Markdown pilot diagnostics");
    }
    let mut counts = BTreeMap::new();
    for diagnostic in diagnostics {
        if diagnostic["severity"] != "warning" {
            return Err("carried diagnostic is not a warning");
        }
        let code = diagnostic["code"]
            .as_str()
            .ok_or("missing diagnostic code")?;
        *counts.entry(code).or_insert(0usize) += 1;
    }
    if counts
        != BTreeMap::from([
            ("compat.unknown_extension", 4),
            ("compat.raw_html_quarantined", 2),
            ("compat.unsafe_link_dropped", 1),
            ("compat.unsafe_image_src_dropped", 1),
        ])
    {
        return Err("unexpected Markdown compatibility warning budget");
    }
    // Use the production Diagnostic field order and existing pretty serializer;
    // do not round-trip the envelope and hide unrelated byte differences.
    let typed: Vec<adoc_core::Diagnostic> = serde_json::from_value(carried.clone())
        .map_err(|_| "invalid registered carried diagnostics")?;
    let rendered = serde_json::to_string_pretty(&typed)
        .map_err(|_| "cannot serialize carried diagnostics")?
        .replace('\n', "\n  ");
    let fragment = format!("\n  \"diagnostics\": {rendered}");
    let stdout =
        std::str::from_utf8(&baseline.stdout).map_err(|_| "baseline stdout is not UTF-8")?;
    if stdout.matches(&fragment).count() != 1 {
        return Err("expected exactly one complete top-level diagnostics fragment");
    }
    let expected = stdout.replacen(&fragment, "\n  \"diagnostics\": []", 1);
    if expected.as_bytes() != candidate.stdout {
        return Err("output bytes changed beyond the authorized diagnostic removal");
    }
    Ok("markdown_carried_compat_warnings_removed")
}

#[test]
#[cfg(unix)]
fn paired_output_accepts_only_complete_carried_diagnostic_removal() {
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;

    #[derive(serde::Serialize)]
    struct Envelope<'a> {
        records: Vec<Value>,
        diagnostics: &'a [adoc_core::Diagnostic],
    }
    let diagnostics: Vec<adoc_core::Diagnostic> = [
        ("compat.unknown_extension", 4),
        ("compat.raw_html_quarantined", 2),
        ("compat.unsafe_link_dropped", 1),
        ("compat.unsafe_image_src_dropped", 1),
    ]
    .into_iter()
    .flat_map(|(code, count)| {
        (0..count).map(move |index| {
            serde_json::from_value(serde_json::json!({
                "code": code, "severity": "warning", "message": format!("{code} {index}"),
                "span": null, "object_id": null, "help": "Inspect source with adoc check."
            }))
            .unwrap()
        })
    })
    .collect();
    let carried = serde_json::to_value(&diagnostics).unwrap();
    let output = |diagnostics: &[adoc_core::Diagnostic], id: &str| Output {
        status: ExitStatus::from_raw(0),
        stdout: format!(
            "{}\n",
            serde_json::to_string_pretty(&Envelope {
                records: vec![serde_json::json!({"id": id, "match": {"result_rank": 1}})],
                diagnostics,
            })
            .unwrap()
        )
        .into_bytes(),
        stderr: vec![],
    };
    let baseline = output(&diagnostics, "billing.visible");
    let candidate = output(&[], "billing.visible");
    assert_eq!(
        paired_output_compatibility("markdown-pilot", &carried, &baseline, &candidate),
        Ok("markdown_carried_compat_warnings_removed")
    );
    for pilot in ["billing-pilot", "markdown-pilot"] {
        assert_eq!(
            paired_output_compatibility(pilot, &carried, &candidate, &candidate),
            Ok("none")
        );
    }
    assert!(paired_output_compatibility("billing-pilot", &carried, &baseline, &candidate).is_err());
    for change in ["record", "diagnostic", "status", "stderr", "format"] {
        let mut changed = output(&[], "billing.visible");
        match change {
            "record" => changed = output(&[], "billing.other"),
            "diagnostic" => changed = output(&diagnostics[..1], "billing.visible"),
            "status" => changed.status = ExitStatus::from_raw(256),
            "stderr" => changed.stderr = b"unexpected warning".to_vec(),
            "format" => changed.stdout.push(b' '),
            _ => unreachable!(),
        }
        assert!(
            paired_output_compatibility("markdown-pilot", &carried, &baseline, &changed).is_err(),
            "{change}"
        );
        assert!(
            paired_output_compatibility("billing-pilot", &carried, &candidate, &changed).is_err(),
            "{change}"
        );
    }
    // Full payload equality and exact warning counts are separate requirements.
    let mut changed = diagnostics.clone();
    changed[0].help = Some("changed repair help".into());
    assert!(
        paired_output_compatibility(
            "markdown-pilot",
            &carried,
            &output(&changed, "billing.visible"),
            &candidate
        )
        .is_err()
    );
    for change in ["count", "code", "severity"] {
        let mut changed = diagnostics.clone();
        match change {
            "count" => {
                changed.pop();
            }
            "code" => changed[0].code = adoc_core::DiagnosticCode::IoArtifactMissing,
            "severity" => changed[0].severity = adoc_core::Severity::Error,
            _ => unreachable!(),
        }
        assert!(
            paired_output_compatibility(
                "markdown-pilot",
                &serde_json::to_value(&changed).unwrap(),
                &output(&changed, "billing.visible"),
                &candidate
            )
            .is_err(),
            "{change}"
        );
    }
}

/// E6.1 CLI overhead gate. Build both revisions with `cargo test --release
/// -p adoc-cli --test retrieval_pilot --no-run --locked`, then supply the saved
/// baseline executable through ADOC_RETRIEVAL_BASELINE_BIN. Both processes read
/// the same pilot artifact bytes. The checked legacy Markdown diagnostic removal
/// is reported explicitly; every other output byte must match. This includes
/// startup/I/O and does not prove isolated predicate cost or hidden-present/absent
/// timing privacy.
/// Set ADOC_RETRIEVAL_NO_POLICY_ONLY=1 when comparing to a pre-policy baseline.
#[test]
#[ignore = "paired release benchmark; requires ADOC_RETRIEVAL_BASELINE_BIN"]
#[allow(clippy::assertions_on_constants)] // Compile in debug, but refuse execution there.
fn permission_retrieval_paired_release_gate() {
    assert!(!cfg!(debug_assertions), "run this gate with --release");
    let baseline = PathBuf::from(
        std::env::var_os("ADOC_RETRIEVAL_BASELINE_BIN").expect("baseline release binary required"),
    );
    let candidate = PathBuf::from(env!("CARGO_BIN_EXE_adoc"));
    let median = |samples: &[f64]| {
        let mut sorted = samples.to_vec();
        sorted.sort_by(f64::total_cmp);
        sorted[sorted.len() / 2]
    };
    let mut failures = Vec::new();
    for set in [&BILLING_PILOT_SET, &MARKDOWN_PILOT_SET] {
        let pilot = build_pilot(
            &repo_root(),
            set.pilot_dir,
            "permission-perf",
            EmbeddingBackend::InMemory,
        );
        let nodes = pilot.graph_json["nodes"].as_array().unwrap();
        assert!(nodes.iter().all(|node| node["visibility"].is_null()));
        let object_id = nodes
            .iter()
            .find(|node| node["type"] == "knowledge_object")
            .expect("pilot contains Knowledge Objects")["id"]
            .as_str()
            .unwrap();
        let context = TestWorkspace::new("permission-perf-context");
        for policy in [false, true] {
            if policy && std::env::var_os("ADOC_RETRIEVAL_NO_POLICY_ONLY").is_some() {
                continue;
            }
            if policy {
                context.write("agentdoc.config.yaml", "version: 1\nmode: strict\ndocs_path: .\nretrieval_policy:\n  audience: public\n  allowed_visibilities: [public]\n  excluded_object_ids: []\n");
            }
            let mut commands = vec![
                vec!["search", "credits", "--lexical"],
                vec!["search", "credits", "--semantic"],
                vec!["search", "credits"],
                vec!["why", object_id],
                vec!["graph", object_id, "--direction", "both"],
                vec!["stale"],
                vec!["contradictions", "--all"],
                vec!["impacted-by", "src/billing.rs"],
            ];
            for args in &mut commands {
                args.extend([
                    "--artifact",
                    pilot.artifact_path.to_str().unwrap(),
                    "--format",
                    "json",
                ]);
                if args[0] == "search" {
                    args.extend([
                        "--search-artifact",
                        pilot.search_artifact_path.to_str().unwrap(),
                    ]);
                }
                let run = |binary: &Path| {
                    let mut command = Command::new(binary);
                    EmbeddingBackend::InMemory.configure(&mut command);
                    command
                        .current_dir(&context.root)
                        .args(&*args)
                        .output()
                        .unwrap()
                };
                let expected = run(&baseline);
                assert!(
                    matches!(expected.status.code(), Some(0 | 1)),
                    "{args:?}: {expected:?}"
                );
                let actual = run(&candidate);
                let compatibility_delta = paired_output_compatibility(
                    set.pilot_dir,
                    &pilot.graph_json["diagnostics"],
                    &expected,
                    &actual,
                )
                .unwrap_or_else(|error| panic!("{args:?}: {error}"));
                let measure = |binary: &Path, expected: &Output| {
                    let start = std::time::Instant::now();
                    let outputs: Vec<_> = (0..5).map(|_| run(binary)).collect();
                    let elapsed = start.elapsed().as_secs_f64() / 5.0;
                    for output in outputs {
                        assert_eq!(
                            (&output.status, &output.stdout, &output.stderr),
                            (&expected.status, &expected.stdout, &expected.stderr)
                        );
                    }
                    elapsed
                };
                let mut base_samples = Vec::new();
                let mut head_samples = Vec::new();
                let mut ratios = Vec::new();
                // Three warmup pairs, then 51 pairs; alternate AB/BA to limit order bias.
                for pair in 0..54 {
                    let (base, head) = if pair % 2 == 0 {
                        (measure(&baseline, &expected), measure(&candidate, &actual))
                    } else {
                        let head = measure(&candidate, &actual);
                        (measure(&baseline, &expected), head)
                    };
                    if pair >= 3 {
                        base_samples.push(base);
                        head_samples.push(head);
                        ratios.push(head / base);
                    }
                }
                let ratio = median(&ratios);
                println!(
                    "{}",
                    serde_json::json!({
                        "pilot": set.pilot_dir, "policy": policy, "args": args,
                        "compatibility_delta": compatibility_delta,
                        "measured_pairs": 51, "warmup_pairs": 3,
                        "nodes": nodes.len(), "base_seconds": base_samples,
                        "head_seconds": head_samples, "paired_ratios": ratios,
                        "base_median_ms": median(&base_samples) * 1000.0,
                        "head_median_ms": median(&head_samples) * 1000.0,
                        "paired_median_ratio": ratio, "limit": 1.10, "passed": ratio <= 1.10,
                    })
                );
                if ratio > 1.10 {
                    failures.push(format!(
                        "{} policy={policy} {args:?}: {ratio:.4}",
                        set.pilot_dir
                    ));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "CLI overhead exceeds 1.10: {failures:#?}"
    );
}

#[test]
fn retrieval_set_queries_pass_against_billing_pilot() {
    run_retrieval_set(
        &BILLING_PILOT_SET,
        EmbeddingBackend::InMemory,
        "billing-pilot-retrieval-set",
    );
}

#[test]
fn retrieval_set_queries_pass_against_markdown_pilot() {
    run_retrieval_set(
        &MARKDOWN_PILOT_SET,
        EmbeddingBackend::InMemory,
        "markdown-pilot-retrieval-set",
    );
}

#[cfg(feature = "fastembed-it")]
#[test]
fn retrieval_set_queries_pass_against_fastembed_provider() {
    run_retrieval_set(
        &BILLING_PILOT_SET,
        EmbeddingBackend::FastEmbed,
        "billing-pilot-fastembed-retrieval-set",
    );
}

#[cfg(feature = "fastembed-it")]
#[test]
fn markdown_retrieval_set_queries_pass_against_fastembed_provider() {
    run_retrieval_set(
        &MARKDOWN_PILOT_SET,
        EmbeddingBackend::FastEmbed,
        "markdown-pilot-fastembed-retrieval-set",
    );
}

fn run_retrieval_set(set: &PilotRetrievalSet, backend: EmbeddingBackend, workspace_name: &str) {
    let repo_root = repo_root();
    let retrieval_set_path = repo_root
        .join("examples")
        .join(set.pilot_dir)
        .join("retrieval-set.yaml");
    let retrieval_set = fs::read_to_string(&retrieval_set_path)
        .unwrap_or_else(|_| panic!("{} retrieval-set.yaml exists", set.pilot_dir));
    let cases = parse_retrieval_set(&retrieval_set).expect("retrieval-set.yaml parses");

    assert!(
        set.case_count.contains(&cases.len()),
        "{} retrieval set should carry {:?} benchmark cases, got {}",
        set.pilot_dir,
        set.case_count,
        cases.len()
    );

    let pilot = build_pilot(&repo_root, set.pilot_dir, workspace_name, backend);
    let mut expected_id_assertions = 0usize;

    for case in &cases {
        let output = run_search_case(
            case,
            &repo_root,
            &pilot.artifact_path,
            &pilot.search_artifact_path,
            backend,
        );
        assert_eq!(
            output.status.code(),
            Some(case.expected_exit),
            "unexpected exit for `{}`\nstdout:\n{}\nstderr:\n{}",
            case.name,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        match case.format {
            CaseFormat::Json => {
                assert!(
                    output.stderr.is_empty(),
                    "JSON case `{}` should not emit stderr diagnostics\nstderr:\n{}",
                    case.name,
                    String::from_utf8_lossy(&output.stderr)
                );
                let envelope: Value = serde_json::from_slice(&output.stdout)
                    .unwrap_or_else(|error| panic!("case `{}` stdout is JSON: {error}", case.name));
                assert_eq!(envelope["schema_version"], "adoc.retrieval.v1");
                assert_record_contract(
                    &case.name,
                    &envelope,
                    case.mode,
                    &case.query,
                    case.top.unwrap_or(case.must_appear_in_top).max(1),
                );
                assert_expected_diagnostics(case, &envelope);
                expected_id_assertions += assert_expected_ids(case, &envelope);
                assert_expected_evidence(case, &envelope);
            }
            CaseFormat::Plain => {
                if case.expected_exit == 0 {
                    assert!(
                        output.stderr.is_empty(),
                        "plain success case `{}` should not emit stderr\nstderr:\n{}",
                        case.name,
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
                if let Some(expected) = &case.expect_stdout_contains {
                    assert!(
                        String::from_utf8_lossy(&output.stdout).contains(expected),
                        "plain case `{}` stdout should contain `{expected}`\nstdout:\n{}",
                        case.name,
                        String::from_utf8_lossy(&output.stdout)
                    );
                }
            }
        }
    }

    println!(
        "retrieval-set baseline: {} queries, {} succeeded, 0 failed; {} expected-id assertions passed",
        cases.len(),
        cases.len(),
        expected_id_assertions
    );
}

#[test]
fn retrieval_pilot_property_invariants_hold() {
    assert_property_invariants(EmbeddingBackend::InMemory, "billing-pilot-invariants");
}

#[cfg(feature = "fastembed-it")]
#[test]
fn retrieval_pilot_property_invariants_hold_against_fastembed_provider() {
    assert_property_invariants(
        EmbeddingBackend::FastEmbed,
        "billing-pilot-fastembed-invariants",
    );
}

fn assert_property_invariants(backend: EmbeddingBackend, workspace_name: &str) {
    let repo_root = repo_root();
    let pilot = build_pilot(&repo_root, "billing-pilot", workspace_name, backend);
    let objects = pilot.graph_json["nodes"]
        .as_array()
        .expect("graph JSON nodes is an array")
        .iter()
        .filter(|node| node["type"] == "knowledge_object")
        .collect::<Vec<_>>();
    assert!(!objects.is_empty(), "pilot artifact should contain objects");

    let mut owners: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for object in objects {
        let id = object["id"].as_str().expect("object id is a string");
        let body = object["body"]
            .as_str()
            .expect("object body is a string")
            .trim();

        let id_envelope = run_lexical_json(&repo_root, &pilot.artifact_path, id, 1, &[], backend);
        assert_record_contract(
            &format!("object-id invariant for {id}"),
            &id_envelope,
            RetrievalMode::Lexical,
            id,
            1,
        );
        assert_first_id(&format!("object-id invariant for {id}"), &id_envelope, id);

        let body_envelope =
            run_lexical_json(&repo_root, &pilot.artifact_path, body, 3, &[], backend);
        assert_record_contract(
            &format!("body invariant for {id}"),
            &body_envelope,
            RetrievalMode::Lexical,
            body,
            3,
        );
        let body_ids = record_ids(&body_envelope);
        assert!(
            body_ids.iter().any(|body_id| body_id == &id),
            "body invariant for `{id}` expected source object in top 3; got {body_ids:?}"
        );

        if let Some(owner) = object["fields"]["owner"].as_str() {
            owners
                .entry(owner.to_string())
                .or_default()
                .push(id.to_string());
        }
    }

    for (owner, expected_ids) in owners {
        let owner_filter = [("--owner", owner.as_str())];
        let envelope = run_lexical_json(
            &repo_root,
            &pilot.artifact_path,
            "",
            expected_ids.len(),
            &owner_filter,
            backend,
        );
        assert_record_contract(
            &format!("owner invariant for {owner}"),
            &envelope,
            RetrievalMode::Lexical,
            "",
            expected_ids.len(),
        );
        let actual_ids = record_ids(&envelope);
        for expected_id in &expected_ids {
            assert!(
                actual_ids.iter().any(|id| id == expected_id),
                "owner invariant for `{owner}` expected `{expected_id}` in {actual_ids:?}"
            );
        }
        for record in envelope["records"].as_array().expect("records is an array") {
            assert_eq!(
                record["owner"], owner,
                "owner invariant for `{owner}` returned wrong owner in {record}"
            );
        }
    }
}

/// V1.7.3 roadmap acceptance — the symmetry property as a CLI-level
/// invariant: identical prose compiled from a `.adoc` source and a `.md`
/// source ranks identically in blended hybrid and lexical search; only the
/// source path differs.
#[test]
fn prose_symmetry_property_holds_across_source_modes() {
    assert_prose_symmetry(EmbeddingBackend::InMemory, "prose-symmetry");
}

#[cfg(feature = "fastembed-it")]
#[test]
fn prose_symmetry_property_holds_against_fastembed_provider() {
    assert_prose_symmetry(EmbeddingBackend::FastEmbed, "prose-symmetry-fastembed");
}

const SYMMETRY_PROSE: &str = "# Billing basics\n\n\
Credits are consumed when a generation job completes, not when it starts.\n\n\
Refunds are handled manually by support.\n\n\
1. Rotate the signing key quarterly.\n\
2. Revoke the previous key after rotation.\n\n\
```shell\nadoc build --provider fastembed\n```\n";

const SYMMETRY_QUERIES: [&str; 4] = [
    "credits consumed",
    "refunds handled by support",
    "rotate the signing key",
    "billing basics",
];

/// The source-mode-independent projection of a search hit: everything a
/// symmetric pair must agree on (the source path is asserted separately —
/// it is the one field allowed to differ).
#[derive(Debug, PartialEq)]
struct SymmetryRecord {
    id: String,
    text: Value,
    search_match: Value,
}

fn assert_prose_symmetry(backend: EmbeddingBackend, workspace_prefix: &str) {
    let adoc_results = symmetry_search_results(backend, "adoc", workspace_prefix);
    let md_results = symmetry_search_results(backend, "md", workspace_prefix);

    for (query, (adoc, md)) in SYMMETRY_QUERIES
        .iter()
        .zip(adoc_results.iter().zip(&md_results))
    {
        assert_eq!(
            adoc, md,
            "ids, text, and match metadata must be source-mode-independent for `{query}`"
        );
    }
}

fn symmetry_search_results(
    backend: EmbeddingBackend,
    extension: &str,
    workspace_prefix: &str,
) -> Vec<Vec<SymmetryRecord>> {
    let repo_root = repo_root();
    let workspace = TestWorkspace::new(&format!("{workspace_prefix}-{extension}"));
    workspace.write(
        "agentdoc.config.yaml",
        "version: 1\nmode: strict\ndocs_path: .\noutputs:\n  dir: dist\nembeddings:\n  provider: local\n",
    );
    workspace.write(&format!("guides/basics.{extension}"), SYMMETRY_PROSE);

    let output_directory = workspace.root.join("dist");
    let mut build = Command::new(env!("CARGO_BIN_EXE_adoc"));
    backend.configure(&mut build);
    let build_output = build
        .current_dir(&workspace.root)
        .args([
            "build",
            ".",
            "--out",
            output_directory
                .to_str()
                .expect("output directory path is utf-8"),
        ])
        .output()
        .expect("adoc build runs");
    assert_success(&format!("symmetry {extension} build"), &build_output);

    let artifact = output_directory.join("docs.graph.json");
    let search_artifact = output_directory.join("docs.search.json");
    let mut query_results = Vec::new();
    for query in SYMMETRY_QUERIES {
        let mut command = Command::new(env!("CARGO_BIN_EXE_adoc"));
        backend.configure(&mut command);
        let output = command
            .current_dir(&repo_root)
            .args([
                "search",
                query,
                "--artifact",
                artifact.to_str().expect("artifact path is utf-8"),
                "--search-artifact",
                search_artifact
                    .to_str()
                    .expect("search artifact path is utf-8"),
                "--top",
                "10",
                "--format",
                "json",
            ])
            .output()
            .expect("adoc search runs");
        assert_success(&format!("symmetry {extension} search `{query}`"), &output);
        let envelope: Value =
            serde_json::from_slice(&output.stdout).expect("search stdout is JSON");
        let records = envelope["records"]
            .as_array()
            .expect("records is an array")
            .iter()
            .map(|record| {
                let source_path = record["source"]["path"]
                    .as_str()
                    .expect("prose record has a source path");
                assert!(
                    source_path.ends_with(&format!(".{extension}")),
                    "symmetry fixture is single-source: expected .{extension}, got {source_path}"
                );
                SymmetryRecord {
                    id: record["id"].as_str().expect("record id").to_string(),
                    text: record["text"].clone(),
                    search_match: record["match"].clone(),
                }
            })
            .collect::<Vec<_>>();
        assert!(
            !records.is_empty(),
            "symmetry query `{query}` must match the {extension} fixture"
        );
        query_results.push(records);
    }
    query_results
}

fn build_pilot(
    repo_root: &Path,
    pilot_dir: &str,
    workspace_name: &str,
    backend: EmbeddingBackend,
) -> PilotBuild {
    let workspace = TestWorkspace::new(workspace_name);
    let output_directory = workspace.root.join("dist");
    let mut command = Command::new(env!("CARGO_BIN_EXE_adoc"));
    backend.configure(&mut command);
    let build_output = command
        .current_dir(repo_root)
        .args([
            "build",
            &format!("examples/{pilot_dir}"),
            "--out",
            output_directory
                .to_str()
                .expect("output directory path is utf-8"),
        ])
        .output()
        .expect("adoc build runs");
    assert_success(&format!("{pilot_dir} build"), &build_output);

    let artifact_path = output_directory.join("docs.graph.json");
    let search_artifact_path = output_directory.join("docs.search.json");
    let graph_json_text = fs::read_to_string(&artifact_path).expect("pilot graph JSON is written");
    let graph_json: Value =
        serde_json::from_str(&graph_json_text).expect("graph JSON is valid JSON");

    PilotBuild {
        _workspace: workspace,
        artifact_path,
        search_artifact_path,
        graph_json,
    }
}

fn run_search_case(
    case: &RetrievalCase,
    repo_root: &PathBuf,
    artifact_path: &Path,
    search_artifact_path: &Path,
    backend: EmbeddingBackend,
) -> Output {
    let mut args = vec![
        "search".to_string(),
        case.query.clone(),
        "--artifact".to_string(),
        artifact_path.to_string_lossy().into_owned(),
        "--search-artifact".to_string(),
        search_artifact_path.to_string_lossy().into_owned(),
        "--top".to_string(),
        case.top
            .unwrap_or(case.must_appear_in_top)
            .max(1)
            .to_string(),
    ];

    match case.scope {
        // V1.7.1 roadmap acceptance: --objects-only reproduces the pre-V1.7
        // Knowledge-Object expected_ids sequences under blended defaults.
        CaseScope::ObjectsOnly => args.push("--objects-only".to_string()),
        // V1.7.3: blended is the shipped default — no scope flag.
        CaseScope::Blended => {}
        CaseScope::ProseOnly => args.push("--prose-only".to_string()),
    }
    match case.mode {
        RetrievalMode::Hybrid => {}
        RetrievalMode::Lexical => args.push("--lexical".to_string()),
        RetrievalMode::Semantic => args.push("--semantic".to_string()),
    }
    match case.format {
        CaseFormat::Json => args.extend(["--format".to_string(), "json".to_string()]),
        CaseFormat::Plain => args.extend(["--format".to_string(), "plain".to_string()]),
    }
    push_filter_arg(&mut args, "--kind", &case.filters.kind);
    push_filter_arg(&mut args, "--status", &case.filters.status);
    push_filter_arg(&mut args, "--owner", &case.filters.owner);
    push_filter_arg(&mut args, "--source-path", &case.filters.source_path);

    let mut command = Command::new(env!("CARGO_BIN_EXE_adoc"));
    backend.configure(&mut command);
    command
        .current_dir(repo_root)
        .args(args)
        .output()
        .expect("adoc search runs")
}

fn run_lexical_json(
    repo_root: &Path,
    artifact_path: &Path,
    query: &str,
    top: usize,
    filters: &[(&str, &str)],
    backend: EmbeddingBackend,
) -> Value {
    let mut args = vec![
        "search".to_string(),
        query.to_string(),
        "--artifact".to_string(),
        artifact_path.to_string_lossy().into_owned(),
        "--lexical".to_string(),
        "--format".to_string(),
        "json".to_string(),
        "--top".to_string(),
        top.max(1).to_string(),
        "--objects-only".to_string(),
    ];
    for (flag, value) in filters {
        args.push((*flag).to_string());
        args.push((*value).to_string());
    }

    let mut command = Command::new(env!("CARGO_BIN_EXE_adoc"));
    backend.configure(&mut command);
    let output = command
        .current_dir(repo_root)
        .args(args)
        .output()
        .expect("adoc search runs");
    assert_success("lexical JSON search", &output);
    assert!(
        output.stderr.is_empty(),
        "lexical JSON search should not emit stderr\nstderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "lexical JSON search stdout is JSON: {error}\nstdout:\n{}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn push_filter_arg(args: &mut Vec<String>, flag: &str, value: &Option<String>) {
    if let Some(value) = value {
        args.push(flag.to_string());
        args.push(value.clone());
    }
}

fn assert_success(label: &str, output: &Output) {
    assert!(
        output.status.success(),
        "expected {label} to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_record_contract(
    label: &str,
    envelope: &Value,
    expected_mode: RetrievalMode,
    query: &str,
    top: usize,
) {
    let records = envelope["records"].as_array().expect("records is an array");
    // Pins ride above the `top` budget (ADR-0040): `top` bounds scored hits,
    // so the record count may exceed it by the query-prefix pin count.
    let pinned = records
        .iter()
        .filter(|record| {
            !query.is_empty()
                && record["id"]
                    .as_str()
                    .is_some_and(|id| id.starts_with(query))
        })
        .count();
    assert!(
        records.len() <= top + pinned,
        "{label} returned more records than top {top} plus {pinned} pins: {records:?}"
    );

    let mut seen_ids = BTreeSet::new();
    for (index, record) in records.iter().enumerate() {
        let id = record["id"].as_str().expect("record id is a string");
        assert!(seen_ids.insert(id), "{label} returned duplicate id `{id}`");
        assert_eq!(
            record["match"]["result_rank"],
            (index + 1) as u64,
            "{label} should have contiguous result ranks"
        );

        let expected_mode_text = match expected_mode {
            RetrievalMode::Hybrid => "hybrid",
            RetrievalMode::Lexical => "lexical",
            RetrievalMode::Semantic => "semantic",
        };
        assert_eq!(
            record["match"]["mode"], expected_mode_text,
            "{label} should report requested mode"
        );
        match expected_mode {
            RetrievalMode::Hybrid => {
                assert!(
                    record["match"]["rrf_score"].is_number(),
                    "{label} hybrid record should include rrf_score: {record}"
                );
                assert!(
                    record["match"].get("cosine_score").is_none(),
                    "{label} hybrid record should not expose raw cosine score: {record}"
                );
            }
            RetrievalMode::Lexical => {}
            RetrievalMode::Semantic => {
                assert!(
                    record["match"]["vector_rank"].is_number(),
                    "{label} semantic record should include vector_rank: {record}"
                );
                assert!(
                    record["match"]["cosine_score"].is_number(),
                    "{label} semantic record should include cosine_score: {record}"
                );
            }
        }
    }
}

fn assert_first_id(label: &str, envelope: &Value, expected_id: &str) {
    let records = envelope["records"].as_array().expect("records is an array");
    let first = records
        .first()
        .unwrap_or_else(|| panic!("{label} expected one record"));
    assert_eq!(first["id"], expected_id, "{label} returned wrong top hit");
}

fn record_ids(envelope: &Value) -> Vec<&str> {
    envelope["records"]
        .as_array()
        .expect("records is an array")
        .iter()
        .filter_map(|record| record["id"].as_str())
        .collect()
}

fn assert_expected_diagnostics(case: &RetrievalCase, envelope: &Value) {
    let diagnostics = envelope["diagnostics"]
        .as_array()
        .expect("diagnostics is an array");
    // V1.7.3: exact-match diagnostic budgets — when a case declares expected
    // codes, the envelope's distinct code set must equal them exactly, so an
    // extra or vanished diagnostic is a red test, not drift.
    if !case.expected_diagnostics.is_empty() {
        let actual_codes: BTreeSet<&str> = diagnostics
            .iter()
            .filter_map(|diagnostic| diagnostic["code"].as_str())
            .collect();
        let expected_codes: BTreeSet<&str> = case
            .expected_diagnostics
            .iter()
            .map(String::as_str)
            .collect();
        assert_eq!(
            actual_codes, expected_codes,
            "case `{}` diagnostic code set mismatch",
            case.name
        );
    }
    if case.expected_exit == 0 && case.expected_diagnostics.is_empty() {
        assert!(
            diagnostics.is_empty(),
            "case `{}` expected no diagnostics, got {diagnostics:?}",
            case.name
        );
    }
}

fn assert_expected_ids(case: &RetrievalCase, envelope: &Value) -> usize {
    if case.expected_ids.is_empty() {
        return 0;
    }
    let records = envelope["records"].as_array().expect("records is an array");
    let ids: Vec<&str> = records
        .iter()
        .filter_map(|record| record["id"].as_str())
        .collect();
    let top_ids: Vec<&str> = ids.iter().take(case.must_appear_in_top).copied().collect();
    for expected_id in &case.expected_ids {
        assert!(
            top_ids.iter().any(|id| id == expected_id),
            "case `{}` expected `{expected_id}` in top {} IDs; got {ids:?}",
            case.name,
            case.must_appear_in_top
        );
    }
    case.expected_ids.len()
}

fn assert_expected_evidence(case: &RetrievalCase, envelope: &Value) {
    if case.expected_evidence.is_empty() {
        return;
    }
    let records = envelope["records"].as_array().expect("records is an array");
    let target = case
        .expected_ids
        .first()
        .and_then(|expected_id| records.iter().find(|record| record["id"] == *expected_id))
        .or_else(|| records.first())
        .unwrap_or_else(|| panic!("case `{}` expected at least one record", case.name));
    for (key, expected_value) in &case.expected_evidence {
        assert_eq!(
            target["evidence"][key], *expected_value,
            "case `{}` expected evidence `{key}`",
            case.name
        );
    }
}

fn parse_retrieval_set(input: &str) -> Result<Vec<RetrievalCase>, String> {
    let mut cases: Vec<RetrievalCase> = serde_saphyr::from_str(input)
        .map_err(|error| format!("retrieval-set.yaml parse error: {error}"))?;
    for (index, case) in cases.iter_mut().enumerate() {
        if case.name.is_empty() {
            case.name = format!("case-{}", index + 1);
        }
    }
    Ok(cases)
}

#[test]
fn retrieval_set_parser_accepts_yaml_scalars_and_block_lists() {
    let cases = parse_retrieval_set(
        r#"
- query: "key: value # literal"
  expected_ids:
    - billing.example
  expected_evidence:
    source: "runbook: v3 # literal"
  filters:
    source_path: "docs:billing.adoc"
"#,
    )
    .expect("retrieval set YAML parses");

    assert_eq!(cases.len(), 1);
    assert_eq!(cases[0].name, "case-1");
    assert_eq!(cases[0].query, "key: value # literal");
    assert_eq!(cases[0].expected_ids, ["billing.example"]);
    assert_eq!(
        cases[0].expected_evidence["source"],
        "runbook: v3 # literal"
    );
    assert_eq!(
        cases[0].filters.source_path.as_deref(),
        Some("docs:billing.adoc")
    );
}
