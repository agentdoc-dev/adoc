use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use serde_json::{Value, json};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

const ACTIVE_CONTRACT: &str = "docs/pilots/g1a/evidence-contract-v2.yaml";
const FROZEN_CONTRACTS: &[(&str, &str)] = &[
    (
        "docs/pilots/g1a/evidence-contract-v1.yaml",
        "0a254fa4f160f61f8bf0551acf8588211aab6503",
    ),
    (ACTIVE_CONTRACT, "26d8108508945f0db58f4fea911f8517cbba931e"),
];

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
fn shared_schema_accepts_a_non_g1a_cohort_without_g1a_revisions() {
    let schema: Value = serde_json::from_str(
        &fs::read_to_string(
            root().join("docs/agent/v0/schema/agentdoc.evidence_contract.v0.schema.json"),
        )
        .expect("evidence contract schema is readable"),
    )
    .expect("evidence contract schema is JSON");
    let validator = jsonschema::validator_for(&schema).expect("evidence contract schema compiles");
    let instance = json!({"evidence_contract": {
        "id": "g1b.external", "version": 1,
        "frozen_at": "2026-09-01T00:00:00Z", "eligible_from": "2026-09-02T00:00:00Z",
        "cohort_definition": {
            "kind": "external_pilot_assessments",
            "implementation_revisions": {"collector": "v1"},
            "selection_rule": "Select attempts before their outcomes are observed.",
            "material_rule_change": "close_cohort_and_increment_version"
        },
        "minimum_population": {"metric_denominators": {"acceptance_rate": 25}},
        "minimum_duration": "P1D",
        "metrics": [{
            "id": "acceptance_rate", "source": "external_design_partner_runs",
            "value_kind": "rate", "numerator": "Accepted assessments.",
            "denominator": "All selected assessment attempts."
        }],
        "numerator_denominator_rules": [{
            "metric_id": "acceptance_rate", "denominator_floor": 25,
            "missing_or_invalid_attempt": "count_in_denominator_as_failure",
            "below_floor": "descriptive_insufficient_evidence_no_promotion"
        }],
        "exclusions": [{"id": "fixtures", "rule": "Fixture runs are ineligible."}],
        "thresholds": [{"metric_id": "acceptance_rate", "operator": "at_least", "value": 1}],
        "stop_ship_conditions": [{"id": "digest_mismatch", "rule": "Any mismatch stops ship."}],
        "approved_by": [{"principal": "Release owner", "roles": ["release_owner"]}]
    }});
    let errors = validator
        .iter_errors(&instance)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "{}", errors.join("\n"));
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
fn every_frozen_contract_matches_its_immutable_git_anchor() {
    let repository = root();
    let contracts = fs::read_dir(repository.join("docs/pilots/g1a"))
        .expect("G1A pilot directory is readable")
        .filter_map(Result::ok)
        .map(|entry| format!("docs/pilots/g1a/{}", entry.file_name().to_string_lossy()))
        .filter(|path| {
            path.rsplit('/').next().is_some_and(|name| {
                name.starts_with("evidence-contract-v") && name.ends_with(".yaml")
            })
        })
        .collect::<HashSet<_>>();
    assert_eq!(
        contracts,
        FROZEN_CONTRACTS
            .iter()
            .map(|(path, _)| (*path).to_owned())
            .collect(),
        "every published contract version must have an immutable Git anchor"
    );

    for (path, frozen_at_commit) in FROZEN_CONTRACTS {
        let frozen = Command::new("git")
            .args(["show", &format!("{frozen_at_commit}:{path}")])
            .current_dir(&repository)
            .output()
            .expect("git show runs");
        assert!(frozen.status.success(), "git show failed for {path}");
        assert_eq!(
            fs::read(repository.join(&path)).expect("frozen contract bytes"),
            frozen.stdout,
            "{path} changed; close this cohort and add a new version"
        );
    }
}
