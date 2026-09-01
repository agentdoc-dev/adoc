//! Immutable source placement and atomic assertion contracts (E4.1).

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::hashing::sha256_prefixed;

pub const SOURCE_ASSERTION_SCHEMA_VERSION: &str = "adoc.source_assertion.v0";
pub const SOURCE_BINDING_SCHEMA_VERSION: &str = "adoc.source_binding.v0";

/// The graph-v6 Source Binding shape, shared by graph nodes and the
/// standalone envelope used by Cloud's source-observation store.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceBindingCoordinates {
    pub connector: String,
    pub source: String,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_revision",
        skip_serializing_if = "Option::is_none"
    )]
    pub revision: Option<String>,
    pub path: String,
    pub anchor: String,
    pub source_revision_digest: String,
}

fn deserialize_optional_revision<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    String::deserialize(deserializer).map(Some)
}

#[derive(Debug, Clone)]
pub struct SourceBindingInput {
    pub source_binding_id: String,
    pub workspace_id: String,
    pub source_record_id: String,
    pub coordinates: SourceBindingCoordinates,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceBinding {
    schema_version: String,
    source_binding_id: String,
    workspace_id: String,
    source_record_id: String,
    binding: SourceBindingCoordinates,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSourceBinding {
    #[serde(rename = "schema_version")]
    _schema_version: String,
    source_binding_id: String,
    workspace_id: String,
    source_record_id: String,
    binding: SourceBindingCoordinates,
}

#[derive(Debug, Clone)]
pub struct SourceAssertionInput<'a> {
    pub source_assertion_id: String,
    pub workspace_id: String,
    pub source_record_id: String,
    pub source_binding_id: String,
    pub source_acl_snapshot_id: String,
    pub extractor: String,
    pub extractor_version: String,
    pub media_type: String,
    pub exact_bytes: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceAssertion {
    schema_version: String,
    source_assertion_id: String,
    workspace_id: String,
    source_record_id: String,
    source_binding_id: String,
    source_acl_snapshot_id: String,
    extractor: String,
    extractor_version: String,
    media_type: String,
    content_digest: String,
    content_length_bytes: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSourceAssertion {
    #[serde(rename = "schema_version")]
    _schema_version: String,
    source_assertion_id: String,
    workspace_id: String,
    source_record_id: String,
    source_binding_id: String,
    source_acl_snapshot_id: String,
    extractor: String,
    extractor_version: String,
    media_type: String,
    content_digest: String,
    content_length_bytes: u64,
}

#[derive(Debug, Deserialize)]
struct RawEnvelopeVersion {
    schema_version: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SourceProvenanceError {
    #[error("source provenance document is invalid: {message}")]
    InvalidDocument { message: String },
    #[error("unsupported source provenance version '{version}'")]
    UnsupportedVersion { version: String },
    #[error("source assertion content digest does not match the exact bytes")]
    DigestMismatch,
    #[error("source assertion content length does not match the exact bytes")]
    ContentLengthMismatch,
    #[error("source provenance serialization failed: {message}")]
    Serialization { message: String },
}

impl SourceBinding {
    pub fn to_canonical_json(&self) -> Result<String, SourceProvenanceError> {
        serialize(self)
    }
}

impl SourceAssertion {
    pub fn to_canonical_json(&self) -> Result<String, SourceProvenanceError> {
        serialize(self)
    }

    pub fn content_digest(&self) -> &str {
        &self.content_digest
    }
}

pub fn build_source_binding(
    input: SourceBindingInput,
) -> Result<SourceBinding, SourceProvenanceError> {
    for (field, value) in [
        ("source_binding_id", input.source_binding_id.as_str()),
        ("workspace_id", input.workspace_id.as_str()),
        ("source_record_id", input.source_record_id.as_str()),
        ("binding.connector", input.coordinates.connector.as_str()),
        ("binding.source", input.coordinates.source.as_str()),
        ("binding.path", input.coordinates.path.as_str()),
        ("binding.anchor", input.coordinates.anchor.as_str()),
    ] {
        validate_text(field, value)?;
    }
    if let Some(revision) = &input.coordinates.revision {
        validate_text("binding.revision", revision)?;
    }
    validate_sha256(
        "binding.source_revision_digest",
        &input.coordinates.source_revision_digest,
    )?;

    Ok(SourceBinding {
        schema_version: SOURCE_BINDING_SCHEMA_VERSION.to_string(),
        source_binding_id: input.source_binding_id,
        workspace_id: input.workspace_id,
        source_record_id: input.source_record_id,
        binding: input.coordinates,
    })
}

pub fn validate_source_binding(document: &[u8]) -> Result<SourceBinding, SourceProvenanceError> {
    require_version(document, SOURCE_BINDING_SCHEMA_VERSION)?;
    let raw: RawSourceBinding =
        serde_json::from_slice(document).map_err(|error| invalid(error.to_string()))?;
    build_source_binding(SourceBindingInput {
        source_binding_id: raw.source_binding_id,
        workspace_id: raw.workspace_id,
        source_record_id: raw.source_record_id,
        coordinates: raw.binding,
    })
}

pub fn build_source_assertion(
    input: SourceAssertionInput<'_>,
) -> Result<SourceAssertion, SourceProvenanceError> {
    for (field, value) in [
        ("source_assertion_id", input.source_assertion_id.as_str()),
        ("workspace_id", input.workspace_id.as_str()),
        ("source_record_id", input.source_record_id.as_str()),
        ("source_binding_id", input.source_binding_id.as_str()),
        (
            "source_acl_snapshot_id",
            input.source_acl_snapshot_id.as_str(),
        ),
        ("extractor", input.extractor.as_str()),
        ("extractor_version", input.extractor_version.as_str()),
        ("media_type", input.media_type.as_str()),
    ] {
        validate_text(field, value)?;
    }
    let content_length_bytes = u64::try_from(input.exact_bytes.len())
        .map_err(|_| invalid("exact byte length exceeds the contract limit"))?;

    Ok(SourceAssertion {
        schema_version: SOURCE_ASSERTION_SCHEMA_VERSION.to_string(),
        source_assertion_id: input.source_assertion_id,
        workspace_id: input.workspace_id,
        source_record_id: input.source_record_id,
        source_binding_id: input.source_binding_id,
        source_acl_snapshot_id: input.source_acl_snapshot_id,
        extractor: input.extractor,
        extractor_version: input.extractor_version,
        media_type: input.media_type,
        content_digest: sha256_prefixed(input.exact_bytes),
        content_length_bytes,
    })
}

pub fn validate_source_assertion(
    document: &[u8],
    exact_bytes: &[u8],
) -> Result<SourceAssertion, SourceProvenanceError> {
    require_version(document, SOURCE_ASSERTION_SCHEMA_VERSION)?;
    let raw: RawSourceAssertion =
        serde_json::from_slice(document).map_err(|error| invalid(error.to_string()))?;
    let assertion = build_source_assertion(SourceAssertionInput {
        source_assertion_id: raw.source_assertion_id,
        workspace_id: raw.workspace_id,
        source_record_id: raw.source_record_id,
        source_binding_id: raw.source_binding_id,
        source_acl_snapshot_id: raw.source_acl_snapshot_id,
        extractor: raw.extractor,
        extractor_version: raw.extractor_version,
        media_type: raw.media_type,
        exact_bytes,
    })?;
    if raw.content_digest != assertion.content_digest {
        return Err(SourceProvenanceError::DigestMismatch);
    }
    if raw.content_length_bytes != assertion.content_length_bytes {
        return Err(SourceProvenanceError::ContentLengthMismatch);
    }
    Ok(assertion)
}

fn require_version(document: &[u8], expected: &str) -> Result<(), SourceProvenanceError> {
    let version: RawEnvelopeVersion = serde_json::from_slice(document)
        .map_err(|error| invalid(format!("invalid JSON or schema version: {error}")))?;
    if version.schema_version != expected {
        return Err(SourceProvenanceError::UnsupportedVersion {
            version: version.schema_version,
        });
    }
    Ok(())
}

fn validate_text(field: &str, value: &str) -> Result<(), SourceProvenanceError> {
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

fn validate_sha256(field: &str, value: &str) -> Result<(), SourceProvenanceError> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(invalid(format!(
            "{field} must be a lowercase SHA-256 digest"
        )));
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(invalid(format!(
            "{field} must be a lowercase SHA-256 digest"
        )));
    }
    Ok(())
}

fn serialize(value: &impl Serialize) -> Result<String, SourceProvenanceError> {
    serde_json::to_string(value).map_err(|error| SourceProvenanceError::Serialization {
        message: error.to_string(),
    })
}

fn invalid(message: impl Into<String>) -> SourceProvenanceError {
    SourceProvenanceError::InvalidDocument {
        message: message.into(),
    }
}
