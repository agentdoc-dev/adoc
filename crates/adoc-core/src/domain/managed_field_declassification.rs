//! Immutable authorized field-lowering detail (E6.2.T4).
//! Shape validity never grants authority. Native admission links this record to
//! one exact state event and reevaluates current permissions and source ACLs.
use super::{
    managed_field_provenance::{
        ManagedFieldContributionInput, ManagedFieldProvenanceInput, build_managed_field_provenance,
        is_canonical_uuid,
    },
    retrieval::canonical_visibility,
    semantic_context::is_sha256_digest,
    value_objects::effective_date::EffectiveDate,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MANAGED_FIELD_DECLASSIFICATION_SCHEMA_VERSION: &str =
    "adoc.managed_field_declassification.v0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedFieldDeclassificationFieldInput {
    pub selector: String,
    pub prior_classification: String,
    pub new_classification: String,
    pub assertion_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ManagedFieldDeclassificationInput {
    pub request_id: String,
    pub workspace_id: String,
    pub canonical_id: String,
    pub version_id: String,
    pub content_digest: String,
    pub provenance_record_digest: String,
    pub state_event_ordinal: u64,
    pub state_event_digest: String,
    pub fields: Vec<ManagedFieldDeclassificationFieldInput>,
    pub principal_id: String,
    pub auth_session_id: String,
    pub authorization_decision_id: String,
    pub policy_version: String,
    pub rationale: String,
    pub effective_date: String,
    pub restricted_evidence_remains_hidden: bool,
}

/// Construct through the validating functions; immutable records do not expose
/// mutable input or implement Deserialize.
/// ```compile_fail
/// use adoc_core::ManagedFieldDeclassification;
/// let record: ManagedFieldDeclassification = serde_json::from_str("{}").unwrap();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ManagedFieldDeclassification {
    schema_version: String,
    #[serde(flatten)]
    input: ManagedFieldDeclassificationInput,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManagedFieldDeclassification {
    schema_version: String,
    request_id: String,
    workspace_id: String,
    canonical_id: String,
    version_id: String,
    content_digest: String,
    provenance_record_digest: String,
    state_event_ordinal: u64,
    state_event_digest: String,
    fields: Vec<ManagedFieldDeclassificationFieldInput>,
    principal_id: String,
    auth_session_id: String,
    authorization_decision_id: String,
    policy_version: String,
    rationale: String,
    effective_date: String,
    restricted_evidence_remains_hidden: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ManagedFieldDeclassificationError {
    #[error("Managed field declassification has an invalid closed document.")]
    InvalidDocument,
    #[error("Managed field declassification has an unsupported schema version.")]
    UnsupportedVersion,
    #[error("Managed field declassification has invalid exact coordinates or event binding.")]
    InvalidCoordinates,
    #[error(
        "Managed field declassification requires distinct manifest selectors/assertions and strict classification lowering."
    )]
    InvalidFields,
    #[error(
        "Managed field declassification has invalid policy, rationale, date, or evidence posture."
    )]
    InvalidApproval,
    #[error("Managed field declassification serialization failed.")]
    Serialization,
}

impl ManagedFieldDeclassification {
    pub fn to_canonical_json(&self) -> Result<String, ManagedFieldDeclassificationError> {
        serde_json::to_string(self).map_err(|_| ManagedFieldDeclassificationError::Serialization)
    }
}

pub fn build_managed_field_declassification(
    mut input: ManagedFieldDeclassificationInput,
) -> Result<ManagedFieldDeclassification, ManagedFieldDeclassificationError> {
    use ManagedFieldDeclassificationError as E;
    if [
        &input.request_id,
        &input.principal_id,
        &input.auth_session_id,
        &input.authorization_decision_id,
    ]
    .iter()
    .any(|id| !is_canonical_uuid(id))
        || !is_sha256_digest(&input.provenance_record_digest)
        || !is_sha256_digest(&input.state_event_digest)
        || input.state_event_ordinal > 9_007_199_254_740_991
    {
        return Err(E::InvalidCoordinates);
    }
    // Reuse the exact manifest coordinate, selector and assertion invariant.
    build_managed_field_provenance(ManagedFieldProvenanceInput {
        workspace_id: input.workspace_id.clone(),
        canonical_id: input.canonical_id.clone(),
        version_id: input.version_id.clone(),
        content_digest: input.content_digest.clone(),
        fields: input
            .fields
            .iter()
            .map(|field| ManagedFieldContributionInput {
                selector: field.selector.clone(),
                assertion_ids: field.assertion_ids.clone(),
            })
            .collect(),
    })
    .map_err(|_| E::InvalidFields)?;
    for field in &mut input.fields {
        let prior = canonical_visibility(&field.prior_classification).ok_or(E::InvalidFields)?;
        let new = canonical_visibility(&field.new_classification).ok_or(E::InvalidFields)?;
        if new >= prior {
            return Err(E::InvalidFields);
        }
        field.assertion_ids.sort();
    }
    input
        .fields
        .sort_by(|left, right| left.selector.cmp(&right.selector));
    if input.policy_version.is_empty()
        || input.policy_version.trim() != input.policy_version
        || input.rationale.trim().is_empty()
        || input.rationale.chars().count() > 4096
        || !input.restricted_evidence_remains_hidden
        || input.effective_date.len() != 10
        || input.effective_date.starts_with("0000")
        || !EffectiveDate::try_new(&input.effective_date)
            .is_ok_and(|date| date.as_str() == input.effective_date)
    {
        return Err(E::InvalidApproval);
    }
    Ok(ManagedFieldDeclassification {
        schema_version: MANAGED_FIELD_DECLASSIFICATION_SCHEMA_VERSION.into(),
        input,
    })
}

pub fn validate_managed_field_declassification(
    bytes: &[u8],
) -> Result<ManagedFieldDeclassification, ManagedFieldDeclassificationError> {
    use ManagedFieldDeclassificationError as E;
    let shape: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| E::InvalidDocument)?;
    if !shape.is_object()
        || !shape["fields"]
            .as_array()
            .is_some_and(|fields| fields.iter().all(serde_json::Value::is_object))
    {
        return Err(E::InvalidDocument);
    }
    // Deserialize original bytes, preserving duplicate-key rejection.
    let raw: RawManagedFieldDeclassification =
        serde_json::from_slice(bytes).map_err(|_| E::InvalidDocument)?;
    if raw.schema_version != MANAGED_FIELD_DECLASSIFICATION_SCHEMA_VERSION {
        return Err(E::UnsupportedVersion);
    }
    build_managed_field_declassification(ManagedFieldDeclassificationInput {
        request_id: raw.request_id,
        workspace_id: raw.workspace_id,
        canonical_id: raw.canonical_id,
        version_id: raw.version_id,
        content_digest: raw.content_digest,
        provenance_record_digest: raw.provenance_record_digest,
        state_event_ordinal: raw.state_event_ordinal,
        state_event_digest: raw.state_event_digest,
        fields: raw.fields,
        principal_id: raw.principal_id,
        auth_session_id: raw.auth_session_id,
        authorization_decision_id: raw.authorization_decision_id,
        policy_version: raw.policy_version,
        rationale: raw.rationale,
        effective_date: raw.effective_date,
        restricted_evidence_remains_hidden: raw.restricted_evidence_remains_hidden,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    fn valid() -> Value {
        let id = "00000000-0000-4000-8000-000000000001";
        let digest = format!("sha256:{}", "a".repeat(64));
        json!({"schema_version":MANAGED_FIELD_DECLASSIFICATION_SCHEMA_VERSION,
            "request_id":id,"workspace_id":id,"canonical_id":id,"version_id":id,
            "content_digest":digest,"provenance_record_digest":digest,
            "state_event_ordinal":0,"state_event_digest":digest,
            "fields":[{"selector":"/body","prior_classification":"restricted","new_classification":"public","assertion_ids":[id]}],
            "principal_id":id,"auth_session_id":id,"authorization_decision_id":id,
            "policy_version":"exact-policy-v1","rationale":"Authorized derived field release.",
            "effective_date":"2028-02-29","restricted_evidence_remains_hidden":true})
    }
    fn check(
        value: &Value,
    ) -> Result<ManagedFieldDeclassification, ManagedFieldDeclassificationError> {
        validate_managed_field_declassification(&serde_json::to_vec(value).unwrap())
    }
    #[test]
    fn declassification_detail_is_immutable_canonical_and_strict() {
        let record = check(&valid()).unwrap();
        assert_eq!(
            validate_managed_field_declassification(record.to_canonical_json().unwrap().as_bytes())
                .unwrap(),
            record
        );
        let mut value = valid();
        value["fields"][0]["selector"] = json!("/fields/ significant ~0~1 ");
        value["fields"][0]["prior_classification"] = json!("internal");
        value["rationale"] = json!("é".repeat(4096));
        assert!(check(&value).is_ok());
        for (key, bad) in [
            ("state_event_ordinal", json!(-1)),
            ("state_event_ordinal", json!(9007199254740992u64)),
            ("state_event_digest", json!("sha256:x")),
            ("request_id", json!("not-a-uuid")),
            ("rationale", json!(" ")),
            ("rationale", json!("é".repeat(4097))),
            ("effective_date", json!("2026-02-29")),
            ("effective_date", json!("0000-01-01")),
            ("effective_date", json!("2026-2-01")),
            ("policy_version", json!(" p ")),
            ("restricted_evidence_remains_hidden", json!(false)),
            ("foreign", json!(true)),
        ] {
            let mut bad_value = valid();
            bad_value[key] = bad;
            assert!(check(&bad_value).is_err(), "{key}");
        }
        for key in valid().as_object().unwrap().keys() {
            let mut bad = valid();
            bad.as_object_mut().unwrap().remove(key);
            assert!(check(&bad).is_err(), "missing {key}");
            let mut bad = valid();
            bad[key] = Value::Null;
            assert!(check(&bad).is_err(), "null {key}");
        }
        for (key, bad) in [
            ("selector", json!("/fields/x/y")),
            ("prior_classification", json!("public")),
            ("new_classification", json!("restricted")),
            ("assertion_ids", json!([])),
            ("extra", json!(true)),
        ] {
            let mut value = valid();
            value["fields"][0][key] = bad;
            assert!(check(&value).is_err(), "field {key}");
        }
    }
    #[test]
    fn declassification_detail_rejects_positional_arrays_and_duplicates() {
        assert!(check(&json!([])).is_err());
        let mut value = valid();
        value["fields"][0] = json!(["/body", "restricted", "public", []]);
        assert!(check(&value).is_err());
        let mut value = valid();
        let row = value["fields"][0].clone();
        value["fields"].as_array_mut().unwrap().push(row);
        assert!(check(&value).is_err());
        let mut value = valid();
        let id = value["fields"][0]["assertion_ids"][0].clone();
        value["fields"][0]["assertion_ids"]
            .as_array_mut()
            .unwrap()
            .push(id);
        assert!(check(&value).is_err());
        let bytes = valid().to_string().replacen(
            "\"rationale\":",
            "\"rationale\":\"duplicate\",\"rationale\":",
            1,
        );
        assert!(validate_managed_field_declassification(bytes.as_bytes()).is_err());
    }
}
