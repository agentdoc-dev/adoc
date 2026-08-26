use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

const ACTIVE_CONTRACT: &str = "docs/pilots/g1a/evidence-contract-v2.yaml";

fn contract(path: &str) -> Value {
    serde_saphyr::from_str(
        &fs::read_to_string(root().join(path)).expect("G1A evidence contract is readable"),
    )
    .expect("G1A evidence contract is valid YAML")
}

fn active_contract() -> Value {
    contract(ACTIVE_CONTRACT)
}

#[test]
fn g1a_contract_validates_against_the_single_evidence_contract_schema() {
    let schema: Value = serde_json::from_str(
        &fs::read_to_string(
            root().join("docs/agent/v0/schema/agentdoc.evidence_contract.v0.schema.json"),
        )
        .expect("evidence contract schema is readable"),
    )
    .expect("evidence contract schema is JSON");
    let validator = jsonschema::validator_for(&schema).expect("evidence contract schema compiles");
    for path in ["docs/pilots/g1a/evidence-contract-v1.yaml", ACTIVE_CONTRACT] {
        let errors = validator
            .iter_errors(&contract(path))
            .map(|error| error.to_string())
            .collect::<Vec<_>>();
        assert!(errors.is_empty(), "{path}: {}", errors.join("\n"));
    }
}

#[test]
fn frozen_at_precedes_the_earliest_eligible_observation() {
    let instance = active_contract();
    let evidence = &instance["evidence_contract"];
    let frozen_at = evidence["frozen_at"].as_str().expect("frozen_at");
    let earliest_eligible_observation = evidence["eligible_from"].as_str().expect("eligible_from");

    assert!(frozen_at < earliest_eligible_observation);
}

#[test]
fn every_metric_has_one_named_denominator_floor_and_threshold() {
    let instance = active_contract();
    let evidence = &instance["evidence_contract"];
    let metrics = evidence["metrics"].as_array().expect("metrics");
    let ids = metrics
        .iter()
        .map(|metric| metric["id"].as_str().expect("metric id"))
        .collect::<HashSet<_>>();
    assert_eq!(ids.len(), metrics.len(), "metric ids must be unique");
    assert!(metrics.iter().all(|metric| {
        metric["denominator"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    }));

    let rules = evidence["numerator_denominator_rules"]
        .as_array()
        .expect("rules");
    let thresholds = evidence["thresholds"].as_array().expect("thresholds");
    let floors = evidence["minimum_population"]["metric_denominators"]
        .as_object()
        .expect("metric denominator floors");

    for id in ids {
        let matching_rules = rules
            .iter()
            .filter(|rule| rule["metric_id"] == id)
            .collect::<Vec<_>>();
        assert_eq!(
            matching_rules.len(),
            1,
            "each metric has exactly one denominator rule"
        );
        let rule = matching_rules[0];
        assert_eq!(
            rule["denominator_floor"], floors[id],
            "rule and minimum population floors must agree for {id}"
        );
        assert_eq!(
            thresholds
                .iter()
                .filter(|threshold| threshold["metric_id"] == id)
                .count(),
            1,
            "each metric has exactly one frozen threshold"
        );
    }
}

#[test]
fn real_run_set_is_precommitted_at_the_population_floor() {
    let instance = active_contract();
    let evidence = &instance["evidence_contract"];
    let runs = evidence["cohort_definition"]["run_set"]
        .as_array()
        .expect("run set");
    let repositories = runs
        .iter()
        .map(|run| run["repository"].as_str().expect("repository"))
        .collect::<HashSet<_>>();
    let run_ids = runs
        .iter()
        .map(|run| run["id"].as_str().expect("run id"))
        .collect::<HashSet<_>>();

    assert_eq!(run_ids.len(), runs.len(), "run ids must be unique");
    assert!(runs.iter().all(|run| {
        let rule = run["selection_rule"].as_str().expect("selection rule");
        rule.contains("select by repository, run, and attempt before outcome")
            && rule.contains("failed, or incomplete evidence as a denominator failure")
    }));
    assert_eq!(
        runs.len() as u64,
        evidence["minimum_population"]["real_internal_assessments"]
            .as_u64()
            .expect("assessment floor")
    );
    assert_eq!(
        repositories.len() as u64,
        evidence["minimum_population"]["repositories"]
            .as_u64()
            .expect("repository floor")
    );
}

#[test]
fn frozen_contract_bytes_match_the_commit_that_introduced_this_version() {
    let repository = root();
    let log = Command::new("git")
        .args([
            "log",
            "--diff-filter=A",
            "--format=%H",
            "--",
            ACTIVE_CONTRACT,
        ])
        .current_dir(&repository)
        .output()
        .expect("git log runs");
    assert!(log.status.success(), "git log failed");
    let Some(introduced_by) = String::from_utf8(log.stdout)
        .expect("git log is UTF-8")
        .lines()
        .next_back()
        .map(str::to_owned)
    else {
        return; // Pre-commit local check; hosted CI always sees the introducing commit.
    };
    let frozen = Command::new("git")
        .args(["show", &format!("{introduced_by}:{ACTIVE_CONTRACT}")])
        .current_dir(&repository)
        .output()
        .expect("git show runs");
    assert!(frozen.status.success(), "git show failed");
    assert_eq!(
        fs::read(repository.join(ACTIVE_CONTRACT)).expect("active contract bytes"),
        frozen.stdout,
        "frozen contract bytes changed; close this cohort and add a new version"
    );
}
