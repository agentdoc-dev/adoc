//! Clock-free, exact-version sensitive access metadata. Validation grants no authority.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{identity::ObjectId, semantic_context::is_sha256_digest};

pub const SENSITIVE_ACCESS_SCHEMA_VERSION: &str = "adoc.sensitive_access.v0";
const MAX_SEQUENCE: u64 = 9_007_199_254_740_991;

/// Public and absent visibility carry no sensitive classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensitiveClassification {
    Internal,
    Restricted,
}

impl SensitiveClassification {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Internal => "internal",
            Self::Restricted => "restricted",
        }
    }

    pub(crate) fn from_visibility(visibility: Option<&str>) -> Option<Self> {
        match visibility {
            Some("internal") => Some(Self::Internal),
            Some("restricted") => Some(Self::Restricted),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SensitiveAccessContext {
    ManagedWorkspace { workspace_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SensitiveAccessCaller {
    pub principal_id: String,
    pub session_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensitiveAccessCommand {
    Search,
    Why,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SensitiveAccessObject {
    pub canonical_id: String,
    pub version_id: String,
    pub object_id: String,
    pub content_hash: String,
    pub classification: SensitiveClassification,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SensitiveAccessEvent {
    schema_version: String,
    pub event_id: String,
    pub context: SensitiveAccessContext,
    pub caller: SensitiveAccessCaller,
    pub command: SensitiveAccessCommand,
    pub policy_version: String,
    pub sequence: u64,
    pub objects: Vec<SensitiveAccessObject>,
}

#[derive(Debug, Error)]
pub enum SensitiveAccessError {
    #[error("Sensitive access event has an invalid closed JSON shape.")]
    Shape(#[source] serde_json::Error),
    #[error("Sensitive access event has an unsupported schema version.")]
    Version,
    #[error("Sensitive access event requires valid UUID coordinates and a safe positive sequence.")]
    Coordinates,
    #[error("Sensitive access event requires nonempty, ordered, distinct valid subjects.")]
    Subjects,
}

impl SensitiveAccessEvent {
    pub fn new(
        event_id: String,
        context: SensitiveAccessContext,
        caller: SensitiveAccessCaller,
        command: SensitiveAccessCommand,
        policy_version: String,
        sequence: u64,
        objects: Vec<SensitiveAccessObject>,
    ) -> Result<Self, SensitiveAccessError> {
        let event = Self {
            schema_version: SENSITIVE_ACCESS_SCHEMA_VERSION.into(),
            event_id,
            context,
            caller,
            command,
            policy_version,
            sequence,
            objects,
        };
        event.validate()?;
        Ok(event)
    }

    pub fn validate(&self) -> Result<(), SensitiveAccessError> {
        if self.schema_version != SENSITIVE_ACCESS_SCHEMA_VERSION {
            return Err(SensitiveAccessError::Version);
        }
        let SensitiveAccessContext::ManagedWorkspace { workspace_id } = &self.context;
        if [
            &self.event_id,
            workspace_id,
            &self.caller.principal_id,
            &self.caller.session_id,
            &self.policy_version,
        ]
        .iter()
        .any(|value| !is_uuid(value))
            || !(1..=MAX_SEQUENCE).contains(&self.sequence)
        {
            return Err(SensitiveAccessError::Coordinates);
        }
        let mut canonical_ids = BTreeSet::new();
        let mut version_ids = BTreeSet::new();
        if self.objects.is_empty()
            || self
                .objects
                .windows(2)
                .any(|pair| pair[0].object_id >= pair[1].object_id)
            || self.objects.iter().any(|object| {
                !is_uuid(&object.canonical_id)
                    || !is_uuid(&object.version_id)
                    || ObjectId::new(object.object_id.as_str()).is_err()
                    || !is_sha256_digest(&object.content_hash)
                    || !canonical_ids.insert(object.canonical_id.to_ascii_lowercase())
                    || !version_ids.insert(object.version_id.to_ascii_lowercase())
            })
        {
            return Err(SensitiveAccessError::Subjects);
        }
        Ok(())
    }
}

pub fn validate_sensitive_access(
    bytes: &[u8],
) -> Result<SensitiveAccessEvent, SensitiveAccessError> {
    let event: SensitiveAccessEvent =
        serde_json::from_slice(bytes).map_err(SensitiveAccessError::Shape)?;
    event.validate()?;
    Ok(event)
}

fn is_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}
