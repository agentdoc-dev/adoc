use std::collections::HashSet;
use std::fmt::Write;
use std::fs;
use std::path::PathBuf;

use adoc_core::{EvidenceContractValidationError, validate_evidence_contract};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

const ACTIVE_CONTRACT: &str = "docs/pilots/g1a/evidence-contract-v4.yaml";
const FROZEN_CONTRACTS: &[(&str, &str)] = &[
    (
        "docs/pilots/g1a/evidence-contract-v1.yaml",
        "ab438c85bbfc5e8842a7bb4f547cce05235fa79160517f6f334a207ece6fc60c",
    ),
    (
        "docs/pilots/g1a/evidence-contract-v2.yaml",
        "bb58ff8dbc7c5ea1e0dce6398579d2af83b4ccb6bc246725cccf48040f54b79a",
    ),
    (
        "docs/pilots/g1a/evidence-contract-v3.yaml",
        "e3d70e6a16609bd595a14aa5246437307281090ecc8ab5aa116b50084fa00822",
    ),
    (
        ACTIVE_CONTRACT,
        "04baf920776603a538d66dc8214f27e624769b674527d3573b8bfcb1c75c3b5d",
    ),
];

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

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
    for (path, _) in FROZEN_CONTRACTS {
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
    for (path, _) in FROZEN_CONTRACTS {
        let validation = validate_evidence_contract(&contract(path));
        assert!(
            !validation
                .errors
                .contains(&EvidenceContractValidationError::FreezeOrderingInvalid),
            "{path} is eligible before its freeze"
        );
    }
}

#[test]
fn every_contract_has_unique_and_exactly_linked_metric_rules() {
    for (path, _) in FROZEN_CONTRACTS {
        let validation = validate_evidence_contract(&contract(path));
        assert!(
            validation.valid,
            "{path} has ambiguous metric links: {:?}",
            validation.errors
        );
    }
}

#[test]
fn semantic_validator_rejects_duplicate_and_orphan_metric_links() {
    let mut duplicate = active_contract();
    let mut metric = duplicate["evidence_contract"]["metrics"][0].clone();
    metric["source"] = json!("different_source");
    duplicate["evidence_contract"]["metrics"]
        .as_array_mut()
        .expect("metrics")
        .push(metric);
    assert!(
        validate_evidence_contract(&duplicate)
            .errors
            .contains(&EvidenceContractValidationError::MetricIdDuplicate)
    );

    let mut orphan = active_contract();
    orphan["evidence_contract"]["thresholds"][0]["metric_id"] = json!("orphan_metric");
    assert!(
        validate_evidence_contract(&orphan)
            .errors
            .contains(&EvidenceContractValidationError::ThresholdMetricIdsMismatch)
    );
}

#[test]
fn semantic_validator_rejects_eligibility_at_or_before_freeze() {
    for eligible_from in ["2026-08-26T21:10:00Z", "2026-08-26T21:09:59Z"] {
        let mut invalid = active_contract();
        invalid["evidence_contract"]["eligible_from"] = json!(eligible_from);
        assert!(
            validate_evidence_contract(&invalid)
                .errors
                .contains(&EvidenceContractValidationError::FreezeOrderingInvalid)
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
        rule.contains("(created_at, run_id, run_attempt)")
            && rule.contains("restricted to created_at at or after eligible_from")
            && rule.contains("before job scheduling or outcome")
            && rule.contains("unstarted")
            && rule.contains("incomplete evidence as a denominator failure")
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
fn every_frozen_contract_matches_its_sha256_seal() {
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
        "every published contract version must have an immutable digest seal"
    );

    for (path, expected_sha256) in FROZEN_CONTRACTS {
        assert_eq!(
            sha256_hex(&fs::read(repository.join(path)).expect("frozen contract bytes")),
            *expected_sha256,
            "{path} changed; close this cohort and add a new version"
        );
    }
}
