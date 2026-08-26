use std::collections::BTreeSet;

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceContractValidationError {
    EvidenceContractMissing,
    MetricsMissing,
    MetricIdMissing,
    MetricIdDuplicate,
    RulesMissing,
    RuleMetricIdMissing,
    RuleMetricIdDuplicate,
    ThresholdsMissing,
    ThresholdMetricIdMissing,
    ThresholdMetricIdDuplicate,
    DenominatorFloorsMissing,
    RuleMetricIdsMismatch,
    ThresholdMetricIdsMismatch,
    DenominatorFloorIdsMismatch,
    DenominatorFloorValueMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EvidenceContractValidation {
    pub valid: bool,
    pub errors: Vec<EvidenceContractValidationError>,
}

/// Semantic validation paired with `agentdoc.evidence_contract.v0`.
///
/// JSON Schema cannot compare identifier sets across sibling arrays. Every
/// consumer must run this after structural schema validation.
pub fn validate_evidence_contract(document: &Value) -> EvidenceContractValidation {
    let Some(evidence) = document.get("evidence_contract") else {
        return invalid(vec![
            EvidenceContractValidationError::EvidenceContractMissing,
        ]);
    };
    let mut errors = Vec::new();
    let metric_ids = collect_ids(
        &evidence["metrics"],
        "id",
        EvidenceContractValidationError::MetricsMissing,
        EvidenceContractValidationError::MetricIdMissing,
        EvidenceContractValidationError::MetricIdDuplicate,
        &mut errors,
    );
    let rule_ids = collect_ids(
        &evidence["numerator_denominator_rules"],
        "metric_id",
        EvidenceContractValidationError::RulesMissing,
        EvidenceContractValidationError::RuleMetricIdMissing,
        EvidenceContractValidationError::RuleMetricIdDuplicate,
        &mut errors,
    );
    let threshold_ids = collect_ids(
        &evidence["thresholds"],
        "metric_id",
        EvidenceContractValidationError::ThresholdsMissing,
        EvidenceContractValidationError::ThresholdMetricIdMissing,
        EvidenceContractValidationError::ThresholdMetricIdDuplicate,
        &mut errors,
    );
    let floors = evidence["minimum_population"]["metric_denominators"].as_object();
    if floors.is_none() {
        errors.push(EvidenceContractValidationError::DenominatorFloorsMissing);
    }

    if let (Some(metrics), Some(rules)) = (&metric_ids, &rule_ids)
        && metrics != rules
    {
        errors.push(EvidenceContractValidationError::RuleMetricIdsMismatch);
    }
    if let (Some(metrics), Some(thresholds)) = (&metric_ids, &threshold_ids)
        && metrics != thresholds
    {
        errors.push(EvidenceContractValidationError::ThresholdMetricIdsMismatch);
    }
    if let (Some(metrics), Some(floors)) = (&metric_ids, floors) {
        let floor_ids = floors.keys().cloned().collect::<BTreeSet<_>>();
        if metrics != &floor_ids {
            errors.push(EvidenceContractValidationError::DenominatorFloorIdsMismatch);
        }
        if evidence["numerator_denominator_rules"]
            .as_array()
            .is_some_and(|rules| {
                rules.iter().any(|rule| {
                    rule["metric_id"]
                        .as_str()
                        .and_then(|id| floors.get(id))
                        .is_none_or(|floor| rule["denominator_floor"] != *floor)
                })
            })
        {
            errors.push(EvidenceContractValidationError::DenominatorFloorValueMismatch);
        }
    }

    EvidenceContractValidation {
        valid: errors.is_empty(),
        errors,
    }
}

fn collect_ids(
    collection: &Value,
    id_field: &str,
    missing_collection: EvidenceContractValidationError,
    missing_id: EvidenceContractValidationError,
    duplicate_id: EvidenceContractValidationError,
    errors: &mut Vec<EvidenceContractValidationError>,
) -> Option<BTreeSet<String>> {
    let Some(items) = collection.as_array() else {
        errors.push(missing_collection);
        return None;
    };
    let mut ids = BTreeSet::new();
    for item in items {
        let Some(id) = item[id_field].as_str() else {
            errors.push(missing_id);
            return None;
        };
        if !ids.insert(id.to_owned()) && !errors.contains(&duplicate_id) {
            errors.push(duplicate_id);
        }
    }
    Some(ids)
}

fn invalid(errors: Vec<EvidenceContractValidationError>) -> EvidenceContractValidation {
    EvidenceContractValidation {
        valid: false,
        errors,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn contract() -> Value {
        json!({"evidence_contract": {
            "metrics": [{"id": "rate"}],
            "numerator_denominator_rules": [{"metric_id": "rate", "denominator_floor": 2}],
            "thresholds": [{"metric_id": "rate"}],
            "minimum_population": {"metric_denominators": {"rate": 2}}
        }})
    }

    #[test]
    fn accepts_exact_metric_links() {
        assert!(validate_evidence_contract(&contract()).valid);
    }

    #[test]
    fn rejects_duplicate_and_orphan_links() {
        let mut duplicate = contract();
        duplicate["evidence_contract"]["metrics"]
            .as_array_mut()
            .expect("metrics")
            .push(json!({"id": "rate", "source": "different"}));
        assert_eq!(
            validate_evidence_contract(&duplicate).errors,
            vec![EvidenceContractValidationError::MetricIdDuplicate]
        );

        let mut orphan = contract();
        orphan["evidence_contract"]["thresholds"][0]["metric_id"] = json!("orphan");
        assert_eq!(
            validate_evidence_contract(&orphan).errors,
            vec![EvidenceContractValidationError::ThresholdMetricIdsMismatch]
        );
    }
}
