use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use serde_json::Value;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn contract() -> Value {
    serde_saphyr::from_str(
        &fs::read_to_string(root().join("docs/pilots/g1a/evidence-contract-v1.yaml"))
            .expect("G1A evidence contract is readable"),
    )
    .expect("G1A evidence contract is valid YAML")
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
    let instance = contract();
    let validator = jsonschema::validator_for(&schema).expect("evidence contract schema compiles");
    let errors = validator
        .iter_errors(&instance)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn frozen_at_precedes_the_earliest_eligible_observation() {
    let instance = contract();
    let evidence = &instance["evidence_contract"];
    let frozen_at = evidence["frozen_at"].as_str().expect("frozen_at");
    let earliest_eligible_observation = evidence["eligible_from"].as_str().expect("eligible_from");

    assert!(frozen_at < earliest_eligible_observation);
}

#[test]
fn every_metric_has_one_named_denominator_floor_and_threshold() {
    let instance = contract();
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
    let instance = contract();
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
