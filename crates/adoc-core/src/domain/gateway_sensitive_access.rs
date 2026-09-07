//! Immutable gateway-reported access metadata. Shape validity grants no authority.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
    hashing::sha256_prefixed, identity::ObjectId, managed_field_provenance::is_canonical_uuid,
    retrieval::RetrievalPolicy, semantic_context::is_sha256_digest,
    sensitive_access::SensitiveClassification,
};

pub const GATEWAY_SENSITIVE_ACCESS_SCHEMA_VERSION: &str = "adoc.sensitive_access.v1";
pub const MAX_GATEWAY_SENSITIVE_ACCESS_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewaySensitiveAccessCommand {
    Search,
    Why,
    Graph,
    Stale,
    Contradictions,
    ImpactedBy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatewaySensitiveAccessObject {
    pub object_id: String,
    pub content_hash: String,
    pub classification: SensitiveClassification,
}

#[derive(Debug, Clone)]
pub struct GatewaySensitiveAccessInput {
    pub event_id: String,
    pub workspace_id: String,
    pub repository_id: String,
    pub principal_id: String,
    pub auth_session_id: String,
    pub gateway_session_id: String,
    pub command: GatewaySensitiveAccessCommand,
    pub local_policy_digest: String,
    pub sequence: u64,
    pub objects: Vec<GatewaySensitiveAccessObject>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Context {
    kind: String,
    workspace_id: String,
    repository_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Caller {
    principal_id: String,
    auth_session_id: String,
    gateway_session_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEvent {
    schema_version: String,
    event_id: String,
    context: Context,
    caller: Caller,
    reporting_basis: String,
    command: GatewaySensitiveAccessCommand,
    local_policy_digest: String,
    sequence: u64,
    objects: Vec<GatewaySensitiveAccessObject>,
}

/// Construct only through the validating builder/consumer; fields are immutable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct GatewaySensitiveAccessEvent(RawEvent);

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GatewaySensitiveAccessError {
    #[error("Gateway sensitive access requires a closed object-shaped document.")]
    Shape,
    #[error("Gateway sensitive access has an unsupported schema version.")]
    Version,
    #[error("Gateway sensitive access has invalid exact coordinates or sequence.")]
    Coordinates,
    #[error("Gateway sensitive access requires 1..1000 ordered distinct valid subjects.")]
    Subjects,
    #[error("Gateway sensitive access exceeds its UTF-8 byte bound.")]
    Size,
    #[error("Gateway sensitive access serialization failed.")]
    Serialization,
    #[error("Gateway retrieval policy cannot establish a complete digest.")]
    Policy,
}

impl GatewaySensitiveAccessEvent {
    pub fn to_canonical_json(&self) -> Result<String, GatewaySensitiveAccessError> {
        let bytes =
            serde_json::to_string(self).map_err(|_| GatewaySensitiveAccessError::Serialization)?;
        if bytes.len() > MAX_GATEWAY_SENSITIVE_ACCESS_BYTES {
            return Err(GatewaySensitiveAccessError::Size);
        }
        Ok(bytes)
    }

    pub fn content_digest(&self) -> Result<String, GatewaySensitiveAccessError> {
        self.to_canonical_json()
            .map(|json| sha256_prefixed(json.as_bytes()))
    }
}

pub fn build_gateway_sensitive_access(
    mut input: GatewaySensitiveAccessInput,
) -> Result<GatewaySensitiveAccessEvent, GatewaySensitiveAccessError> {
    input.objects.sort_by(|a, b| a.object_id.cmp(&b.object_id));
    let raw = RawEvent {
        schema_version: GATEWAY_SENSITIVE_ACCESS_SCHEMA_VERSION.into(),
        event_id: input.event_id,
        context: Context {
            kind: "gateway_repository".into(),
            workspace_id: input.workspace_id,
            repository_id: input.repository_id,
        },
        caller: Caller {
            principal_id: input.principal_id,
            auth_session_id: input.auth_session_id,
            gateway_session_id: input.gateway_session_id,
        },
        reporting_basis: "gateway_reported".into(),
        command: input.command,
        local_policy_digest: input.local_policy_digest,
        sequence: input.sequence,
        objects: input.objects,
    };
    validate_raw(&raw)?;
    let event = GatewaySensitiveAccessEvent(raw);
    event.to_canonical_json()?;
    Ok(event)
}

pub fn validate_gateway_sensitive_access(
    bytes: &[u8],
) -> Result<GatewaySensitiveAccessEvent, GatewaySensitiveAccessError> {
    use GatewaySensitiveAccessError as E;
    if bytes.len() > MAX_GATEWAY_SENSITIVE_ACCESS_BYTES {
        return Err(E::Size);
    }
    let shape: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| E::Shape)?;
    if !shape.is_object()
        || !shape
            .get("context")
            .is_some_and(serde_json::Value::is_object)
        || !shape
            .get("caller")
            .is_some_and(serde_json::Value::is_object)
        || !shape
            .get("objects")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|objects| objects.iter().all(serde_json::Value::is_object))
    {
        return Err(E::Shape);
    }
    // Deserialize the original bytes so duplicate fields are also refused.
    let raw: RawEvent = serde_json::from_slice(bytes).map_err(|_| E::Shape)?;
    validate_raw(&raw)?;
    Ok(GatewaySensitiveAccessEvent(raw))
}

fn validate_raw(raw: &RawEvent) -> Result<(), GatewaySensitiveAccessError> {
    use GatewaySensitiveAccessError as E;
    if raw.schema_version != GATEWAY_SENSITIVE_ACCESS_SCHEMA_VERSION {
        return Err(E::Version);
    }
    if raw.context.kind != "gateway_repository"
        || raw.reporting_basis != "gateway_reported"
        || [
            &raw.event_id,
            &raw.context.workspace_id,
            &raw.context.repository_id,
            &raw.caller.principal_id,
            &raw.caller.auth_session_id,
            &raw.caller.gateway_session_id,
        ]
        .iter()
        .any(|value| !is_canonical_uuid(value))
        || !is_sha256_digest(&raw.local_policy_digest)
        || !(1..=9_007_199_254_740_991).contains(&raw.sequence)
    {
        return Err(E::Coordinates);
    }
    if !(1..=1000).contains(&raw.objects.len())
        || raw
            .objects
            .windows(2)
            .any(|pair| pair[0].object_id >= pair[1].object_id)
        || raw.objects.iter().any(|object| {
            ObjectId::new(object.object_id.as_str()).is_err()
                || !is_sha256_digest(&object.content_hash)
        })
    {
        return Err(E::Subjects);
    }
    Ok(())
}

pub fn gateway_policy_digest(
    policy: &RetrievalPolicy,
) -> Result<String, GatewaySensitiveAccessError> {
    policy
        .validate()
        .map_err(|_| GatewaySensitiveAccessError::Policy)?;
    serde_json::to_vec(policy)
        .map(|bytes| sha256_prefixed(&bytes))
        .map_err(|_| GatewaySensitiveAccessError::Serialization)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessedKnowledgeObject {
    pub object_id: String,
    pub content_hash: String,
    pub classification: Option<SensitiveClassification>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReadAccess {
    pub objects: Vec<AccessedKnowledgeObject>,
}

impl ReadAccess {
    pub fn max_classification(&self) -> Option<SensitiveClassification> {
        if self
            .objects
            .iter()
            .any(|object| object.classification == Some(SensitiveClassification::Restricted))
        {
            Some(SensitiveClassification::Restricted)
        } else {
            self.objects.iter().find_map(|object| object.classification)
        }
    }

    pub fn sensitive_objects(&self) -> Vec<GatewaySensitiveAccessObject> {
        self.objects
            .iter()
            .filter_map(|object| {
                object
                    .classification
                    .map(|classification| GatewaySensitiveAccessObject {
                        object_id: object.object_id.clone(),
                        content_hash: object.content_hash.clone(),
                        classification,
                    })
            })
            .collect()
    }
}
