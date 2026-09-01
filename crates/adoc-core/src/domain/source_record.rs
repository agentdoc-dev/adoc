//! Digest-bound immutable source observations (`adoc.source_record.v0/v1`, E4.1).

use chrono::{DateTime, Datelike, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::hashing::sha256_prefixed;

pub const SOURCE_RECORD_SCHEMA_VERSION_V0: &str = "adoc.source_record.v0";
pub const SOURCE_RECORD_SCHEMA_VERSION: &str = "adoc.source_record.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceArtifact {
    pub provider: String,
    pub kind: String,
    pub external_id: String,
    pub external_version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceAclResourceKind {
    Repository,
    Project,
    Space,
    Channel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceAclResource {
    pub kind: SourceAclResourceKind,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceAclScope {
    pub snapshot_id: String,
    pub source_container_id: String,
    pub source: SourceAclResource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetentionClass {
    DigestOnly,
    BoundedEvidence,
    ExactCandidateInput,
    TemporaryProcessing,
    FullSourceSnapshot,
}

#[derive(Debug, Clone)]
pub struct SourceRecordInput<'a> {
    pub source_record_id: String,
    pub workspace_id: String,
    pub connector_id: String,
    pub source: SourceArtifact,
    pub source_acl_scope: SourceAclScope,
    pub observed_at: DateTime<Utc>,
    pub media_type: String,
    pub retention_class: RetentionClass,
    pub exact_bytes: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceRecord {
    schema_version: String,
    source_record_id: String,
    workspace_id: String,
    connector_id: String,
    source: SourceArtifact,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_acl_scope: Option<SourceAclScope>,
    observed_at: DateTime<Utc>,
    media_type: String,
    retention_class: RetentionClass,
    content_digest: String,
    content_length_bytes: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSourceRecord {
    schema_version: String,
    source_record_id: String,
    workspace_id: String,
    connector_id: String,
    source: SourceArtifact,
    #[serde(default, deserialize_with = "deserialize_source_acl_scope")]
    source_acl_scope: Option<SourceAclScope>,
    observed_at: String,
    media_type: String,
    retention_class: RetentionClass,
    content_digest: String,
    content_length_bytes: u64,
}

#[derive(Debug, Deserialize)]
struct RawEnvelopeVersion {
    schema_version: String,
}

fn deserialize_source_acl_scope<'de, D>(deserializer: D) -> Result<Option<SourceAclScope>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    SourceAclScope::deserialize(deserializer).map(Some)
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SourceRecordError {
    #[error("source record document is invalid: {message}")]
    InvalidDocument { message: String },
    #[error("unsupported source record version '{version}'")]
    UnsupportedVersion { version: String },
    #[error("source record content digest does not match the exact bytes")]
    DigestMismatch,
    #[error("source record content length does not match the exact bytes")]
    ContentLengthMismatch,
    #[error("source record serialization failed: {message}")]
    Serialization { message: String },
}

impl SourceRecordError {
    pub fn remediation(&self) -> &'static str {
        match self {
            Self::UnsupportedVersion { .. } => {
                "Regenerate the document with the supported source-record version."
            }
            Self::DigestMismatch | Self::ContentLengthMismatch => {
                "Submit metadata and exact bytes from the same immutable observation."
            }
            Self::InvalidDocument { .. } | Self::Serialization { .. } => {
                "Regenerate a complete valid source-record document."
            }
        }
    }
}

impl SourceRecord {
    pub fn to_canonical_json(&self) -> Result<String, SourceRecordError> {
        serde_json::to_string(self).map_err(|error| SourceRecordError::Serialization {
            message: error.to_string(),
        })
    }

    pub fn content_digest(&self) -> &str {
        &self.content_digest
    }

    pub fn content_length_bytes(&self) -> u64 {
        self.content_length_bytes
    }
}

pub fn build_source_record(
    input: SourceRecordInput<'_>,
) -> Result<SourceRecord, SourceRecordError> {
    let content_length_bytes = u64::try_from(input.exact_bytes.len())
        .map_err(|_| invalid("exact byte length exceeds the contract limit"))?;
    source_record_from_raw(
        RawSourceRecord {
            schema_version: SOURCE_RECORD_SCHEMA_VERSION.to_string(),
            source_record_id: input.source_record_id,
            workspace_id: input.workspace_id,
            connector_id: input.connector_id,
            source: input.source,
            source_acl_scope: Some(input.source_acl_scope),
            observed_at: input.observed_at.to_rfc3339_opts(SecondsFormat::Secs, true),
            media_type: input.media_type,
            retention_class: input.retention_class,
            content_digest: sha256_prefixed(input.exact_bytes),
            content_length_bytes,
        },
        input.observed_at,
        input.exact_bytes,
    )
}

pub fn validate_source_record(
    document: &[u8],
    exact_bytes: &[u8],
) -> Result<SourceRecord, SourceRecordError> {
    let version: RawEnvelopeVersion = serde_json::from_slice(document)
        .map_err(|error| invalid(format!("invalid JSON or schema version: {error}")))?;
    if !matches!(
        version.schema_version.as_str(),
        SOURCE_RECORD_SCHEMA_VERSION_V0 | SOURCE_RECORD_SCHEMA_VERSION
    ) {
        return Err(SourceRecordError::UnsupportedVersion {
            version: version.schema_version,
        });
    }
    let raw: RawSourceRecord =
        serde_json::from_slice(document).map_err(|error| invalid(error.to_string()))?;
    match (
        version.schema_version.as_str(),
        raw.source_acl_scope.is_some(),
    ) {
        (SOURCE_RECORD_SCHEMA_VERSION_V0, false) | (SOURCE_RECORD_SCHEMA_VERSION, true) => {}
        _ => {
            return Err(invalid(
                "source_acl_scope does not match the schema version",
            ));
        }
    }
    let observed_at = DateTime::parse_from_rfc3339(&raw.observed_at)
        .map_err(|error| invalid(format!("observed_at is invalid: {error}")))?
        .with_timezone(&Utc);
    if raw.observed_at != observed_at.to_rfc3339_opts(SecondsFormat::Secs, true) {
        return Err(invalid("observed_at must use whole UTC seconds"));
    }

    source_record_from_raw(raw, observed_at, exact_bytes)
}

fn source_record_from_raw(
    raw: RawSourceRecord,
    observed_at: DateTime<Utc>,
    exact_bytes: &[u8],
) -> Result<SourceRecord, SourceRecordError> {
    validate_text("source_record_id", &raw.source_record_id)?;
    validate_text("workspace_id", &raw.workspace_id)?;
    validate_text("connector_id", &raw.connector_id)?;
    validate_text("source.provider", &raw.source.provider)?;
    validate_text("source.kind", &raw.source.kind)?;
    validate_text("source.external_id", &raw.source.external_id)?;
    validate_text("source.external_version", &raw.source.external_version)?;
    if let Some(scope) = &raw.source_acl_scope {
        validate_text("source_acl_scope.snapshot_id", &scope.snapshot_id)?;
        validate_text(
            "source_acl_scope.source_container_id",
            &scope.source_container_id,
        )?;
        validate_text("source_acl_scope.source.id", &scope.source.id)?;
    }
    validate_text("media_type", &raw.media_type)?;
    if !(0..=9999).contains(&observed_at.year()) {
        return Err(invalid("observed_at must use a four-digit UTC year"));
    }
    if observed_at.timestamp_subsec_nanos() != 0 {
        return Err(invalid("observed_at must use whole UTC seconds"));
    }
    let content_length_bytes = u64::try_from(exact_bytes.len())
        .map_err(|_| invalid("exact byte length exceeds the contract limit"))?;
    let content_digest = sha256_prefixed(exact_bytes);
    if raw.content_digest != content_digest {
        return Err(SourceRecordError::DigestMismatch);
    }
    if raw.content_length_bytes != content_length_bytes {
        return Err(SourceRecordError::ContentLengthMismatch);
    }
    Ok(SourceRecord {
        schema_version: raw.schema_version,
        source_record_id: raw.source_record_id,
        workspace_id: raw.workspace_id,
        connector_id: raw.connector_id,
        source: raw.source,
        source_acl_scope: raw.source_acl_scope,
        observed_at,
        media_type: raw.media_type,
        retention_class: raw.retention_class,
        content_digest,
        content_length_bytes,
    })
}

fn validate_text(field: &str, value: &str) -> Result<(), SourceRecordError> {
    if value.is_empty()
        || value.trim() != value
        || value.contains(['\r', '\n', '\u{2028}', '\u{2029}'])
    {
        return Err(invalid(format!(
            "{field} must be non-empty single-line text"
        )));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> SourceRecordError {
    SourceRecordError::InvalidDocument {
        message: message.into(),
    }
}
