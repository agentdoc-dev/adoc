//! Provider-neutral semantic executor protocol (`E3.4`).
//!
//! Adapters invoke providers; core accepts only exact, digest-bound requests
//! and validator-accepted `adoc.semantic_assessment.v0` candidates.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use super::hashing::sha256_prefixed;
use super::semantic_assessment::{
    HumanReviewExpectedBindings, SemanticAssessment, SemanticExecutorIdentity,
};
use super::semantic_context::{
    SemanticContext, SemanticContextError, SemanticContextOutcome, is_semantic_context_text,
    is_sha256_digest, validate_semantic_context_integrity,
};

pub const SEMANTIC_EXECUTOR_REQUEST_SCHEMA_VERSION: &str = "adoc.semantic_executor_request.v0";
pub const SEMANTIC_EXECUTOR_RECEIPT_SCHEMA_VERSION: &str = "adoc.semantic_executor_receipt.v0";

const MAX_REQUEST_BYTES: usize = 4 * 1024 * 1024;
const MAX_PROMPT_CHARS: usize = 256 * 1024;
const MAX_ASSESSMENT_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticAdapterKind {
    ClaudeCode,
    Codex,
    Generic,
    Human,
}

impl SemanticAdapterKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude_code",
            Self::Codex => "codex",
            Self::Generic => "generic",
            Self::Human => "human",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticEndpointClass {
    PublicProvider,
    CustomerHosted,
    Local,
    Human,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticAdapterDescriptor {
    pub kind: SemanticAdapterKind,
    pub provider: String,
    pub model: String,
    pub endpoint_class: SemanticEndpointClass,
    pub endpoint_id: String,
    pub executor_digest: String,
    pub model_digest: String,
    pub config_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticPromptContract {
    pub contract_version: String,
    pub digest: String,
    pub instructions: String,
}

#[derive(Serialize)]
struct SemanticPromptDigestInput<'a> {
    contract_version: &'a str,
    instructions: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSemanticExecutorRequest {
    schema_version: String,
    request_id: String,
    capability: String,
    adapter: SemanticAdapterDescriptor,
    human_review: Option<HumanReviewExpectedBindings>,
    task_digest: String,
    prompt: SemanticPromptContract,
    timeout_seconds: u16,
    context: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct SemanticExecutorRequest {
    schema_version: String,
    request_id: String,
    capability: String,
    adapter: SemanticAdapterDescriptor,
    #[serde(skip_serializing_if = "Option::is_none")]
    human_review: Option<HumanReviewExpectedBindings>,
    task_digest: String,
    prompt: SemanticPromptContract,
    timeout_seconds: u16,
    context: SemanticContext,
}

impl SemanticExecutorRequest {
    pub fn context(&self) -> &SemanticContext {
        &self.context
    }

    pub fn adapter(&self) -> &SemanticAdapterDescriptor {
        &self.adapter
    }

    pub fn human_review(&self) -> Option<&HumanReviewExpectedBindings> {
        self.human_review.as_ref()
    }

    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    pub fn capability(&self) -> &str {
        &self.capability
    }

    pub fn task_digest(&self) -> &str {
        &self.task_digest
    }

    pub fn prompt_digest(&self) -> &str {
        &self.prompt.digest
    }

    pub fn timeout_seconds(&self) -> u16 {
        self.timeout_seconds
    }

    pub fn to_canonical_json(&self) -> Result<String, SemanticExecutorError> {
        pretty_json(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticExecutorOutcome {
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
pub struct SemanticExecutorReceipt {
    schema_version: String,
    request_id: String,
    request_digest: String,
    capability: String,
    adapter: SemanticAdapterDescriptor,
    task_digest: String,
    prompt_digest: String,
    context_digest: String,
    outcome: SemanticExecutorOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    assessment_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure_code: Option<String>,
}

impl SemanticExecutorReceipt {
    pub fn outcome(&self) -> SemanticExecutorOutcome {
        self.outcome
    }

    pub fn adapter(&self) -> &SemanticAdapterDescriptor {
        &self.adapter
    }

    pub fn context_digest(&self) -> &str {
        &self.context_digest
    }

    pub fn assessment_digest(&self) -> Option<&str> {
        self.assessment_digest.as_deref()
    }

    pub fn to_canonical_json(&self) -> Result<String, SemanticExecutorError> {
        pretty_json(self)
    }
}

#[derive(Debug, Error)]
pub enum SemanticExecutorError {
    #[error("semantic executor request is invalid: {message}")]
    InvalidRequest { message: String },
    #[error("unsupported semantic executor request version '{version}'")]
    UnsupportedVersion { version: String },
    #[error("semantic executor context is invalid: {0}")]
    InvalidContext(#[from] SemanticContextError),
    #[error("semantic executor assessment identity does not match the declared adapter")]
    IdentityMismatch,
    #[error("semantic executor serialization failed: {message}")]
    Serialization { message: String },
}

pub fn validate_semantic_executor_request(
    bytes: &[u8],
) -> Result<SemanticExecutorRequest, SemanticExecutorError> {
    if bytes.len() > MAX_REQUEST_BYTES {
        return Err(invalid("request exceeds 4 MiB"));
    }
    let raw: RawSemanticExecutorRequest =
        serde_json::from_slice(bytes).map_err(|error| invalid(error.to_string()))?;
    if raw.schema_version != SEMANTIC_EXECUTOR_REQUEST_SCHEMA_VERSION {
        return Err(SemanticExecutorError::UnsupportedVersion {
            version: raw.schema_version,
        });
    }
    for (field, value) in [
        ("request_id", raw.request_id.as_str()),
        ("capability", raw.capability.as_str()),
        ("adapter.provider", raw.adapter.provider.as_str()),
        ("adapter.model", raw.adapter.model.as_str()),
        ("adapter.endpoint_id", raw.adapter.endpoint_id.as_str()),
        (
            "prompt.contract_version",
            raw.prompt.contract_version.as_str(),
        ),
    ] {
        if !is_semantic_context_text(value) {
            return Err(invalid(format!("{field} must be non-blank semantic text")));
        }
    }
    if raw.prompt.instructions.trim().is_empty() {
        return Err(invalid("prompt.instructions must be non-blank"));
    }
    for (field, digest) in [
        (
            "adapter.executor_digest",
            raw.adapter.executor_digest.as_str(),
        ),
        ("adapter.model_digest", raw.adapter.model_digest.as_str()),
        ("adapter.config_digest", raw.adapter.config_digest.as_str()),
        ("task_digest", raw.task_digest.as_str()),
        ("prompt.digest", raw.prompt.digest.as_str()),
    ] {
        if !is_sha256_digest(digest) {
            return Err(invalid(format!("{field} must be a sha256 digest")));
        }
    }
    if raw.prompt.digest
        != semantic_prompt_digest(&raw.prompt.contract_version, &raw.prompt.instructions)?
    {
        return Err(invalid(
            "prompt.digest does not bind the exact contract version and instructions",
        ));
    }
    if !(60..=3600).contains(&raw.timeout_seconds) {
        return Err(invalid("timeout_seconds must be between 60 and 3600"));
    }
    if raw.prompt.instructions.chars().count() > MAX_PROMPT_CHARS {
        return Err(invalid("prompt instructions exceed 262144 characters"));
    }
    let human_review = match (
        raw.adapter.kind,
        raw.adapter.endpoint_class,
        raw.adapter.provider.as_str(),
        raw.human_review,
    ) {
        (SemanticAdapterKind::Human, SemanticEndpointClass::Human, "human", binding) => {
            if let Some(binding) = &binding
                && (!is_semantic_context_text(&binding.reviewing_principal_id)
                    || !is_semantic_context_text(&binding.requesting_principal_id))
            {
                return Err(invalid(
                    "human review request bindings must name both trusted principals",
                ));
            }
            binding
        }
        (SemanticAdapterKind::Human, _, _, _)
        | (_, SemanticEndpointClass::Human, _, _)
        | (_, _, "human", _)
        | (_, _, _, Some(_)) => {
            return Err(invalid(
                "human adapter, provider, endpoint class, and review bindings must be paired",
            ));
        }
        _ => None,
    };
    let context_bytes =
        serde_json::to_vec(&raw.context).map_err(|error| SemanticExecutorError::Serialization {
            message: error.to_string(),
        })?;
    let context = validate_semantic_context_integrity(&context_bytes)?;
    if context.outcome() != SemanticContextOutcome::Ready || !context.has_diff_hunk() {
        return Err(invalid(
            "semantic context must be ready and contain a diff-hunk citation",
        ));
    }
    Ok(SemanticExecutorRequest {
        schema_version: SEMANTIC_EXECUTOR_REQUEST_SCHEMA_VERSION.to_string(),
        request_id: raw.request_id,
        capability: raw.capability,
        adapter: raw.adapter,
        human_review,
        task_digest: raw.task_digest,
        prompt: raw.prompt,
        timeout_seconds: raw.timeout_seconds,
        context,
    })
}

pub fn complete_semantic_execution(
    request: &SemanticExecutorRequest,
    assessment: &SemanticAssessment,
) -> Result<SemanticExecutorReceipt, SemanticExecutorError> {
    let assessment_json =
        assessment
            .to_canonical_json()
            .map_err(|error| SemanticExecutorError::Serialization {
                message: error.to_string(),
            })?;
    if assessment_json.len() > MAX_ASSESSMENT_BYTES {
        return Err(invalid("assessment exceeds 1 MiB"));
    }
    let SemanticExecutorIdentity { provider, model } = assessment.identity();
    if provider != &request.adapter.provider || model != &request.adapter.model {
        return Err(SemanticExecutorError::IdentityMismatch);
    }
    match (
        request.adapter.kind,
        request.human_review.as_ref(),
        assessment.human_review(),
    ) {
        (SemanticAdapterKind::Human, Some(expected), Some(review))
            if review.reviewing_principal_id == expected.reviewing_principal_id
                && review.requesting_principal_id == expected.requesting_principal_id => {}
        (SemanticAdapterKind::Human, _, _) => {
            return Err(invalid(
                "human adapter completion requires the request's trusted human-review facts",
            ));
        }
        (_, None, None) => {}
        _ => {
            return Err(invalid(
                "model adapter completion cannot carry human-review facts",
            ));
        }
    }
    receipt(
        request,
        SemanticExecutorOutcome::Completed,
        Some(sha256_prefixed(assessment_json.as_bytes())),
        None,
    )
}

pub fn semantic_prompt_digest(
    contract_version: &str,
    instructions: &str,
) -> Result<String, SemanticExecutorError> {
    let bytes = serde_json::to_vec(&SemanticPromptDigestInput {
        contract_version,
        instructions,
    })
    .map_err(|error| SemanticExecutorError::Serialization {
        message: error.to_string(),
    })?;
    Ok(sha256_prefixed(&bytes))
}

pub fn fail_semantic_execution(
    request: &SemanticExecutorRequest,
    failure_code: &str,
) -> Result<SemanticExecutorReceipt, SemanticExecutorError> {
    if !is_semantic_context_text(failure_code) {
        return Err(invalid("failure_code must be non-blank semantic text"));
    }
    receipt(
        request,
        SemanticExecutorOutcome::Failed,
        None,
        Some(failure_code.to_string()),
    )
}

fn receipt(
    request: &SemanticExecutorRequest,
    outcome: SemanticExecutorOutcome,
    assessment_digest: Option<String>,
    failure_code: Option<String>,
) -> Result<SemanticExecutorReceipt, SemanticExecutorError> {
    let request_json =
        serde_json::to_vec(request).map_err(|error| SemanticExecutorError::Serialization {
            message: error.to_string(),
        })?;
    Ok(SemanticExecutorReceipt {
        schema_version: SEMANTIC_EXECUTOR_RECEIPT_SCHEMA_VERSION.to_string(),
        request_id: request.request_id.clone(),
        request_digest: sha256_prefixed(&request_json),
        capability: request.capability.clone(),
        adapter: request.adapter.clone(),
        task_digest: request.task_digest.clone(),
        prompt_digest: request.prompt.digest.clone(),
        context_digest: request.context.context_digest().to_string(),
        outcome,
        assessment_digest,
        failure_code,
    })
}

fn invalid(message: impl Into<String>) -> SemanticExecutorError {
    SemanticExecutorError::InvalidRequest {
        message: message.into(),
    }
}

fn pretty_json(value: &impl Serialize) -> Result<String, SemanticExecutorError> {
    let mut json = serde_json::to_string_pretty(value).map_err(|error| {
        SemanticExecutorError::Serialization {
            message: error.to_string(),
        }
    })?;
    json.push('\n');
    Ok(json)
}
