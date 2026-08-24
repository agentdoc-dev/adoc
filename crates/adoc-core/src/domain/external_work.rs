//! Replay-safe external work request/result contracts (`adoc.work_*.v0`, E3.7).

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
    hashing::sha256_prefixed,
    semantic_context::{ExactRevision, is_semantic_context_text, is_sha256_digest},
};

pub const WORK_REQUEST_SCHEMA_VERSION: &str = "adoc.work_request.v0";
pub const WORK_RESULT_SCHEMA_VERSION: &str = "adoc.work_result.v0";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractRequirement {
    pub schema_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityRequirement {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkSource {
    pub provider: String,
    pub external_repository_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkChangeRequest {
    pub system: String,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkloadAuthorization {
    pub principal_id: String,
    pub subject: String,
    pub audience: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkRuntime {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone)]
pub struct WorkRequestInput {
    pub request_id: String,
    pub nonce: String,
    pub workspace_id: String,
    pub repository_id: String,
    pub source: WorkSource,
    pub revision: ExactRevision,
    pub change_request: WorkChangeRequest,
    pub contracts: Vec<ContractRequirement>,
    pub capabilities: Vec<CapabilityRequirement>,
    pub expires_at: DateTime<Utc>,
    pub workload: WorkloadAuthorization,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkRequest {
    schema_version: String,
    request_id: String,
    nonce: String,
    workspace_id: String,
    repository_id: String,
    source: WorkSource,
    revision: ExactRevision,
    change_request: WorkChangeRequest,
    contracts: Vec<ContractRequirement>,
    capabilities: Vec<CapabilityRequirement>,
    expires_at: DateTime<Utc>,
    workload: WorkloadAuthorization,
    request_digest: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWorkRequest {
    schema_version: String,
    request_id: String,
    nonce: String,
    workspace_id: String,
    repository_id: String,
    source: WorkSource,
    revision: ExactRevision,
    change_request: WorkChangeRequest,
    contracts: Vec<ContractRequirement>,
    capabilities: Vec<CapabilityRequirement>,
    expires_at: DateTime<Utc>,
    workload: WorkloadAuthorization,
    request_digest: String,
}

#[derive(Debug, Clone)]
pub struct WorkResultInput {
    pub request_id: String,
    pub request_digest: String,
    pub workspace_id: String,
    pub repository_id: String,
    pub revision: ExactRevision,
    pub completion_nonce: String,
    pub worker: WorkloadAuthorization,
    pub runtime: WorkRuntime,
    pub output_digests: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkResult {
    schema_version: String,
    request_id: String,
    request_digest: String,
    workspace_id: String,
    repository_id: String,
    revision: ExactRevision,
    completion_nonce: String,
    worker: WorkloadAuthorization,
    runtime: WorkRuntime,
    output_digests: BTreeMap<String, String>,
    result_digest: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWorkResult {
    schema_version: String,
    request_id: String,
    request_digest: String,
    workspace_id: String,
    repository_id: String,
    revision: ExactRevision,
    completion_nonce: String,
    worker: WorkloadAuthorization,
    runtime: WorkRuntime,
    output_digests: BTreeMap<String, String>,
    result_digest: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ExternalWorkError {
    #[error("external work document is invalid: {message}")]
    InvalidDocument { message: String },
    #[error("unsupported {envelope} version '{version}'")]
    UnsupportedVersion {
        envelope: &'static str,
        version: String,
    },
    #[error("external work digest does not match its canonical content")]
    DigestMismatch,
    #[error("work result does not match the exact request")]
    BindingMismatch,
    #[error("external work serialization failed: {message}")]
    Serialization { message: String },
}

impl ExternalWorkError {
    pub fn remediation(&self) -> &'static str {
        match self {
            Self::UnsupportedVersion { envelope, .. } if *envelope == "work-request" => {
                "Regenerate the document with the supported work-request version."
            }
            Self::UnsupportedVersion { .. } => {
                "Regenerate the document with the supported work-result version."
            }
            Self::DigestMismatch => {
                "Recreate the envelope and digest from the same canonical content."
            }
            Self::BindingMismatch => {
                "Submit the result only for its exact request, workspace, repository, and revision."
            }
            Self::InvalidDocument { .. } | Self::Serialization { .. } => {
                "Regenerate a complete valid external work envelope."
            }
        }
    }
}

impl WorkRequest {
    pub fn to_canonical_json(&self) -> Result<String, ExternalWorkError> {
        canonical_json(self)
    }

    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    pub fn request_digest(&self) -> &str {
        &self.request_digest
    }

    pub fn workspace_id(&self) -> &str {
        &self.workspace_id
    }

    pub fn repository_id(&self) -> &str {
        &self.repository_id
    }

    pub fn revision(&self) -> &ExactRevision {
        &self.revision
    }

    pub fn workload(&self) -> &WorkloadAuthorization {
        &self.workload
    }

    pub fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }
}

impl WorkResult {
    pub fn to_canonical_json(&self) -> Result<String, ExternalWorkError> {
        canonical_json(self)
    }

    pub fn completion_nonce(&self) -> &str {
        &self.completion_nonce
    }

    pub fn result_digest(&self) -> &str {
        &self.result_digest
    }
}

pub fn build_work_request(mut input: WorkRequestInput) -> Result<WorkRequest, ExternalWorkError> {
    require_texts(&[
        ("request_id", &input.request_id),
        ("nonce", &input.nonce),
        ("workspace_id", &input.workspace_id),
        ("repository_id", &input.repository_id),
        ("source.provider", &input.source.provider),
        (
            "source.external_repository_id",
            &input.source.external_repository_id,
        ),
        ("revision.system", &input.revision.system),
        ("revision.value", &input.revision.value),
        ("change_request.system", &input.change_request.system),
        ("change_request.id", &input.change_request.id),
        ("workload.principal_id", &input.workload.principal_id),
        ("workload.subject", &input.workload.subject),
        ("workload.audience", &input.workload.audience),
    ])?;
    if input.expires_at.timestamp_subsec_nanos() != 0 {
        return Err(invalid("expires_at must use canonical UTC whole seconds"));
    }
    if input.contracts.is_empty() || input.capabilities.is_empty() {
        return Err(invalid(
            "work request requires contract and capability requirements",
        ));
    }
    for contract in &input.contracts {
        require_text("contracts[].schema_version", &contract.schema_version)?;
    }
    for capability in &input.capabilities {
        require_text("capabilities[].name", &capability.name)?;
        require_text("capabilities[].version", &capability.version)?;
    }
    input.contracts.sort();
    input.capabilities.sort();
    reject_duplicates(&input.contracts, "contract requirements")?;
    reject_duplicates(&input.capabilities, "capability requirements")?;

    let digest_input = WorkRequestDigestInput::from(&input);
    let request_digest = digest(&digest_input)?;
    Ok(WorkRequest {
        schema_version: WORK_REQUEST_SCHEMA_VERSION.to_string(),
        request_id: input.request_id,
        nonce: input.nonce,
        workspace_id: input.workspace_id,
        repository_id: input.repository_id,
        source: input.source,
        revision: input.revision,
        change_request: input.change_request,
        contracts: input.contracts,
        capabilities: input.capabilities,
        expires_at: input.expires_at,
        workload: input.workload,
        request_digest,
    })
}

pub fn validate_work_request(bytes: &[u8]) -> Result<WorkRequest, ExternalWorkError> {
    let raw: RawWorkRequest =
        serde_json::from_slice(bytes).map_err(|error| invalid(error.to_string()))?;
    if raw.schema_version != WORK_REQUEST_SCHEMA_VERSION {
        return Err(ExternalWorkError::UnsupportedVersion {
            envelope: "work-request",
            version: raw.schema_version,
        });
    }
    let claimed = raw.request_digest;
    let request = build_work_request(WorkRequestInput {
        request_id: raw.request_id,
        nonce: raw.nonce,
        workspace_id: raw.workspace_id,
        repository_id: raw.repository_id,
        source: raw.source,
        revision: raw.revision,
        change_request: raw.change_request,
        contracts: raw.contracts,
        capabilities: raw.capabilities,
        expires_at: raw.expires_at,
        workload: raw.workload,
    })?;
    if request.request_digest != claimed {
        return Err(ExternalWorkError::DigestMismatch);
    }
    Ok(request)
}

pub fn build_work_result(
    input: WorkResultInput,
    request: &WorkRequest,
) -> Result<WorkResult, ExternalWorkError> {
    require_texts(&[
        ("completion_nonce", &input.completion_nonce),
        ("runtime.name", &input.runtime.name),
        ("runtime.version", &input.runtime.version),
    ])?;
    if input.completion_nonce == request.nonce || input.output_digests.is_empty() {
        return Err(invalid(
            "result requires a distinct completion nonce and output digests",
        ));
    }
    for (name, digest) in &input.output_digests {
        require_text("output_digests key", name)?;
        if !is_sha256_digest(digest) {
            return Err(invalid(format!("output digest '{name}' is invalid")));
        }
    }
    if input.request_id != request.request_id
        || input.request_digest != request.request_digest
        || input.workspace_id != request.workspace_id
        || input.repository_id != request.repository_id
        || input.revision != request.revision
        || input.worker != request.workload
    {
        return Err(ExternalWorkError::BindingMismatch);
    }

    let result_digest = digest(&WorkResultDigestInput::from(&input))?;
    Ok(WorkResult {
        schema_version: WORK_RESULT_SCHEMA_VERSION.to_string(),
        request_id: input.request_id,
        request_digest: input.request_digest,
        workspace_id: input.workspace_id,
        repository_id: input.repository_id,
        revision: input.revision,
        completion_nonce: input.completion_nonce,
        worker: input.worker,
        runtime: input.runtime,
        output_digests: input.output_digests,
        result_digest,
    })
}

pub fn validate_work_result(
    bytes: &[u8],
    request: &WorkRequest,
) -> Result<WorkResult, ExternalWorkError> {
    let raw: RawWorkResult =
        serde_json::from_slice(bytes).map_err(|error| invalid(error.to_string()))?;
    if raw.schema_version != WORK_RESULT_SCHEMA_VERSION {
        return Err(ExternalWorkError::UnsupportedVersion {
            envelope: "work-result",
            version: raw.schema_version,
        });
    }
    let claimed = raw.result_digest;
    let result = build_work_result(
        WorkResultInput {
            request_id: raw.request_id,
            request_digest: raw.request_digest,
            workspace_id: raw.workspace_id,
            repository_id: raw.repository_id,
            revision: raw.revision,
            completion_nonce: raw.completion_nonce,
            worker: raw.worker,
            runtime: raw.runtime,
            output_digests: raw.output_digests,
        },
        request,
    )?;
    if result.result_digest != claimed {
        return Err(ExternalWorkError::DigestMismatch);
    }
    Ok(result)
}

#[derive(Serialize)]
struct WorkRequestDigestInput<'a> {
    schema_version: &'static str,
    request_id: &'a str,
    nonce: &'a str,
    workspace_id: &'a str,
    repository_id: &'a str,
    source: &'a WorkSource,
    revision: &'a ExactRevision,
    change_request: &'a WorkChangeRequest,
    contracts: &'a [ContractRequirement],
    capabilities: &'a [CapabilityRequirement],
    expires_at: DateTime<Utc>,
    workload: &'a WorkloadAuthorization,
}

impl<'a> From<&'a WorkRequestInput> for WorkRequestDigestInput<'a> {
    fn from(input: &'a WorkRequestInput) -> Self {
        Self {
            schema_version: WORK_REQUEST_SCHEMA_VERSION,
            request_id: &input.request_id,
            nonce: &input.nonce,
            workspace_id: &input.workspace_id,
            repository_id: &input.repository_id,
            source: &input.source,
            revision: &input.revision,
            change_request: &input.change_request,
            contracts: &input.contracts,
            capabilities: &input.capabilities,
            expires_at: input.expires_at,
            workload: &input.workload,
        }
    }
}

#[derive(Serialize)]
struct WorkResultDigestInput<'a> {
    schema_version: &'static str,
    request_id: &'a str,
    request_digest: &'a str,
    workspace_id: &'a str,
    repository_id: &'a str,
    revision: &'a ExactRevision,
    completion_nonce: &'a str,
    worker: &'a WorkloadAuthorization,
    runtime: &'a WorkRuntime,
    output_digests: &'a BTreeMap<String, String>,
}

impl<'a> From<&'a WorkResultInput> for WorkResultDigestInput<'a> {
    fn from(input: &'a WorkResultInput) -> Self {
        Self {
            schema_version: WORK_RESULT_SCHEMA_VERSION,
            request_id: &input.request_id,
            request_digest: &input.request_digest,
            workspace_id: &input.workspace_id,
            repository_id: &input.repository_id,
            revision: &input.revision,
            completion_nonce: &input.completion_nonce,
            worker: &input.worker,
            runtime: &input.runtime,
            output_digests: &input.output_digests,
        }
    }
}

fn canonical_json(value: &impl Serialize) -> Result<String, ExternalWorkError> {
    let mut json =
        serde_json::to_string_pretty(value).map_err(|error| ExternalWorkError::Serialization {
            message: error.to_string(),
        })?;
    json.push('\n');
    Ok(json)
}

fn digest(value: &impl Serialize) -> Result<String, ExternalWorkError> {
    serde_json::to_vec(value)
        .map(|bytes| sha256_prefixed(&bytes))
        .map_err(|error| ExternalWorkError::Serialization {
            message: error.to_string(),
        })
}

fn require_texts(values: &[(&str, &str)]) -> Result<(), ExternalWorkError> {
    for (field, value) in values {
        require_text(field, value)?;
    }
    Ok(())
}

fn require_text(field: &str, value: &str) -> Result<(), ExternalWorkError> {
    if is_semantic_context_text(value) {
        return Ok(());
    }
    Err(invalid(format!("{field} must be non-blank text")))
}

fn reject_duplicates<T: PartialEq>(values: &[T], field: &str) -> Result<(), ExternalWorkError> {
    if values.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(invalid(format!("{field} contain duplicates")));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> ExternalWorkError {
    ExternalWorkError::InvalidDocument {
        message: message.into(),
    }
}
