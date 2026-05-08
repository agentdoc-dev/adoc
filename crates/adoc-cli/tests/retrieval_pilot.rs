mod support;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum RetrievalMode {
    #[default]
    Hybrid,
    Lexical,
    Semantic,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum CaseFormat {
    #[default]
    Json,
    Plain,
}

#[derive(Debug, Clone, Default)]
struct CaseFilters {
    kind: Option<String>,
    status: Option<String>,
    owner: Option<String>,
    source_path: Option<String>,
}

#[derive(Debug, Clone)]
struct RetrievalCase {
    name: String,
    query: String,
    mode: RetrievalMode,
    format: CaseFormat,
    filters: CaseFilters,
    expected_ids: Vec<String>,
    expected_diagnostics: Vec<String>,
    expected_evidence: BTreeMap<String, String>,
    expect_stdout_contains: Option<String>,
    expected_exit: i32,
    must_appear_in_top: usize,
}

#[derive(Debug, Clone)]
struct RetrievalCaseBuilder {
    name: Option<String>,
    query: Option<String>,
    mode: RetrievalMode,
    format: CaseFormat,
    filters: CaseFilters,
    expected_ids: Vec<String>,
    expected_diagnostics: Vec<String>,
    expected_evidence: BTreeMap<String, String>,
    expect_stdout_contains: Option<String>,
    expected_exit: i32,
    must_appear_in_top: usize,
}

impl Default for RetrievalCaseBuilder {
    fn default() -> Self {
        Self {
            name: None,
            query: None,
            mode: RetrievalMode::Hybrid,
            format: CaseFormat::Json,
            filters: CaseFilters::default(),
            expected_ids: Vec::new(),
            expected_diagnostics: Vec::new(),
            expected_evidence: BTreeMap::new(),
            expect_stdout_contains: None,
            expected_exit: 0,
            must_appear_in_top: 5,
        }
    }
}

impl RetrievalCaseBuilder {
    fn finish(self, index: usize) -> Result<RetrievalCase, String> {
        let query = self
            .query
            .ok_or_else(|| format!("retrieval case {index} is missing query"))?;
        Ok(RetrievalCase {
            name: self.name.unwrap_or_else(|| format!("case-{index}")),
            query,
            mode: self.mode,
            format: self.format,
            filters: self.filters,
            expected_ids: self.expected_ids,
            expected_diagnostics: self.expected_diagnostics,
            expected_evidence: self.expected_evidence,
            expect_stdout_contains: self.expect_stdout_contains,
            expected_exit: self.expected_exit,
            must_appear_in_top: self.must_appear_in_top,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NestedSection {
    Filters,
    ExpectedEvidence,
}

#[test]
fn retrieval_set_queries_pass_against_billing_pilot() {
    let repo_root = repo_root();
    let retrieval_set_path = repo_root
        .join("examples")
        .join("billing-pilot")
        .join("retrieval-set.yaml");
    let retrieval_set =
        fs::read_to_string(&retrieval_set_path).expect("billing pilot retrieval-set.yaml exists");
    let cases = parse_retrieval_set(&retrieval_set).expect("retrieval-set.yaml parses");

    assert!(
        (15..=20).contains(&cases.len()),
        "retrieval set should carry 15-20 benchmark cases, got {}",
        cases.len()
    );

    let workspace = TestWorkspace::new("billing-pilot-retrieval-set");
    let output_directory = workspace.root.join("dist");
    let build_output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&repo_root)
        .env("ADOC_TEST_EMBEDDING_PROVIDER", "in-memory")
        .args([
            "build",
            "examples/billing-pilot",
            "--out",
            output_directory
                .to_str()
                .expect("output directory path is utf-8"),
        ])
        .output()
        .expect("adoc build runs");
    assert_success("billing pilot build", &build_output);

    let artifact_path = output_directory.join("docs.agent.json");
    let search_artifact_path = output_directory.join("docs.search.json");
    let mut expected_id_assertions = 0usize;

    for case in &cases {
        let output = run_search_case(case, &repo_root, &artifact_path, &search_artifact_path);
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
                assert_eq!(envelope["schema_version"], "adoc.retrieval.v0");
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
        "retrieval-set baseline: {} queries, {} expected-id assertions passed",
        cases.len(),
        expected_id_assertions
    );
}

fn run_search_case(
    case: &RetrievalCase,
    repo_root: &PathBuf,
    artifact_path: &Path,
    search_artifact_path: &Path,
) -> Output {
    let mut args = vec![
        "search".to_string(),
        case.query.clone(),
        "--artifact".to_string(),
        artifact_path.to_string_lossy().into_owned(),
        "--search-artifact".to_string(),
        search_artifact_path.to_string_lossy().into_owned(),
        "--top".to_string(),
        case.must_appear_in_top.max(1).to_string(),
    ];

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

    Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(repo_root)
        .env("ADOC_TEST_EMBEDDING_PROVIDER", "in-memory")
        .args(args)
        .output()
        .expect("adoc search runs")
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

fn assert_expected_diagnostics(case: &RetrievalCase, envelope: &Value) {
    let diagnostics = envelope["diagnostics"]
        .as_array()
        .expect("diagnostics is an array");
    for code in &case.expected_diagnostics {
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic["code"] == *code),
            "case `{}` expected diagnostic `{code}` in {diagnostics:?}",
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
    let mut cases = Vec::new();
    let mut current: Option<RetrievalCaseBuilder> = None;
    let mut section: Option<NestedSection> = None;

    for (line_index, raw_line) in input.lines().enumerate() {
        let line_number = line_index + 1;
        let line_without_comment = raw_line.split_once('#').map_or(raw_line, |(line, _)| line);
        if line_without_comment.trim().is_empty() {
            continue;
        }
        let indent = line_without_comment
            .chars()
            .take_while(|character| *character == ' ')
            .count();
        let line = line_without_comment.trim();

        if let Some(rest) = line.strip_prefix("- ") {
            if let Some(builder) = current.take() {
                let index = cases.len() + 1;
                cases.push(builder.finish(index)?);
            }
            let mut builder = RetrievalCaseBuilder::default();
            parse_case_key_value(&mut builder, rest, line_number)?;
            current = Some(builder);
            section = None;
            continue;
        }

        let builder = current
            .as_mut()
            .ok_or_else(|| format!("line {line_number}: expected a list item"))?;
        if indent == 2 && line.ends_with(':') {
            section = Some(match line.trim_end_matches(':') {
                "filters" => NestedSection::Filters,
                "expected_evidence" => NestedSection::ExpectedEvidence,
                key => return Err(format!("line {line_number}: unknown section `{key}`")),
            });
            continue;
        }

        match (indent, section) {
            (2, _) => {
                section = None;
                parse_case_key_value(builder, line, line_number)?;
            }
            (4.., Some(NestedSection::Filters)) => {
                parse_filter_key_value(&mut builder.filters, line, line_number)?;
            }
            (4.., Some(NestedSection::ExpectedEvidence)) => {
                let (key, value) = parse_key_value(line, line_number)?;
                builder.expected_evidence.insert(key, parse_scalar(value));
            }
            _ => return Err(format!("line {line_number}: invalid indentation")),
        }
    }

    if let Some(builder) = current.take() {
        let index = cases.len() + 1;
        cases.push(builder.finish(index)?);
    }
    Ok(cases)
}

fn parse_case_key_value(
    builder: &mut RetrievalCaseBuilder,
    line: &str,
    line_number: usize,
) -> Result<(), String> {
    let (key, value) = parse_key_value(line, line_number)?;
    match key.as_str() {
        "name" => builder.name = Some(parse_scalar(value)),
        "query" => builder.query = Some(parse_scalar(value)),
        "mode" => {
            builder.mode = match parse_scalar(value).as_str() {
                "hybrid" => RetrievalMode::Hybrid,
                "lexical" => RetrievalMode::Lexical,
                "semantic" => RetrievalMode::Semantic,
                mode => return Err(format!("line {line_number}: unknown mode `{mode}`")),
            };
        }
        "format" => {
            builder.format = match parse_scalar(value).as_str() {
                "json" => CaseFormat::Json,
                "plain" => CaseFormat::Plain,
                format => return Err(format!("line {line_number}: unknown format `{format}`")),
            };
        }
        "expected_ids" => builder.expected_ids = parse_inline_list(value, line_number)?,
        "expected_diagnostics" => {
            builder.expected_diagnostics = parse_inline_list(value, line_number)?
        }
        "expected_exit" => {
            builder.expected_exit = parse_scalar(value)
                .parse::<i32>()
                .map_err(|error| format!("line {line_number}: invalid expected_exit: {error}"))?;
        }
        "must_appear_in_top" => {
            builder.must_appear_in_top = parse_scalar(value).parse::<usize>().map_err(|error| {
                format!("line {line_number}: invalid must_appear_in_top: {error}")
            })?;
        }
        "expect_stdout_contains" => builder.expect_stdout_contains = Some(parse_scalar(value)),
        key => return Err(format!("line {line_number}: unknown key `{key}`")),
    }
    Ok(())
}

fn parse_filter_key_value(
    filters: &mut CaseFilters,
    line: &str,
    line_number: usize,
) -> Result<(), String> {
    let (key, value) = parse_key_value(line, line_number)?;
    let value = Some(parse_scalar(value));
    match key.as_str() {
        "kind" => filters.kind = value,
        "status" => filters.status = value,
        "owner" => filters.owner = value,
        "source_path" => filters.source_path = value,
        key => return Err(format!("line {line_number}: unknown filter `{key}`")),
    }
    Ok(())
}

fn parse_key_value(line: &str, line_number: usize) -> Result<(String, &str), String> {
    let (key, value) = line
        .split_once(':')
        .ok_or_else(|| format!("line {line_number}: expected key: value"))?;
    Ok((key.trim().to_string(), value.trim()))
}

fn parse_inline_list(value: &str, line_number: usize) -> Result<Vec<String>, String> {
    let value = value.trim();
    let inner = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .ok_or_else(|| format!("line {line_number}: expected inline list"))?;
    if inner.trim().is_empty() {
        return Ok(Vec::new());
    }
    Ok(inner.split(',').map(parse_scalar).collect())
}

fn parse_scalar(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_string()
}
