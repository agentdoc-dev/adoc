//! Capability-specific semantic executor qualification (E3.3).
//!
//! The record is provider-neutral. It can influence a required gate only
//! after validation here and an exact comparison with the current executor
//! configuration and operation.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
    hashing::sha256_prefixed,
    semantic_context::{is_semantic_context_text, is_sha256_digest},
};

pub const EXECUTOR_QUALIFICATION_SCHEMA_VERSION: &str = "adoc.executor_qualification.v0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExecutorConfiguration {
    Model {
        provider: String,
        executor_digest: String,
        model_digest: String,
        config_digest: String,
        configuration: Box<ModelConfiguration>,
    },
    Human {
        principal_id: String,
        executor_digest: String,
        config_digest: String,
        permission_policy_digest: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelConfiguration {
    pub model_revision_digest: String,
    pub quantization_digest: String,
    pub system_prompt_task_digest: String,
    pub context_strategy_digest: String,
    pub output_constraints_digest: String,
    pub toolset_digest: String,
    pub inference_parameters_digest: String,
    pub safety_configuration_digest: String,
    pub adapter_implementation_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QualificationLayer {
    ProtocolValid,
    AgentDocEvaluated,
    OrganizationApproved,
    RuntimePolicyEligible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutorAuthority {
    GateAuthoritative,
    AdvisoryOnly,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RequalificationTrigger {
    ModelRevision,
    Quantization,
    SystemPromptOrTaskDefinition,
    ContextStrategy,
    OutputConstraints,
    ToolAvailability,
    InferenceParameters,
    SafetyConfiguration,
    AdapterImplementation,
    ConfigurationDigest,
    AuthenticatedPrincipal,
    PermissionPolicy,
    ExecutorKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExecutorEligibility {
    authority: ExecutorAuthority,
    missing_layers: Vec<QualificationLayer>,
    requalification_triggers: Vec<RequalificationTrigger>,
}

/// Current bindings obtained from the trusted qualification store/policy
/// state, never from the executor-supplied qualification document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutorQualificationExpectedBindings {
    pub qualification_id: String,
    pub record_digest: String,
    pub capability_name: String,
    pub capability_version: String,
    pub protocol_version: String,
    pub requested_scope: String,
    pub requested_risk: String,
    pub requested_deployment: String,
    pub organization_policy_digest: String,
    pub runtime_policy_digest: String,
}

impl ExecutorEligibility {
    pub fn authority(&self) -> ExecutorAuthority {
        self.authority
    }

    pub fn missing_layers(&self) -> &[QualificationLayer] {
        &self.missing_layers
    }

    pub fn requalification_triggers(&self) -> &[RequalificationTrigger] {
        &self.requalification_triggers
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Capability {
    name: String,
    version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProtocolQualification {
    valid: bool,
    version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum AgentDocEvaluation {
    Capability {
        qualified: bool,
        evidence_ref: String,
    },
    AuthenticatedPermission {
        qualified: bool,
        principal_id: String,
        permission_policy_digest: String,
    },
}

impl AgentDocEvaluation {
    fn qualified(&self) -> bool {
        match self {
            Self::Capability { qualified, .. }
            | Self::AuthenticatedPermission { qualified, .. } => *qualified,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OrganizationApproval {
    approved: bool,
    scope: Vec<String>,
    risk: Vec<String>,
    deployment: Vec<String>,
    policy_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimePolicy {
    eligible: bool,
    operation_digest: String,
    policy_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawExecutorQualification {
    schema_version: String,
    qualification_id: String,
    capability: Capability,
    subject: ExecutorConfiguration,
    protocol: ProtocolQualification,
    agentdoc_evaluation: AgentDocEvaluation,
    organization_approval: OrganizationApproval,
    runtime_policy: RuntimePolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutorQualification {
    record: RawExecutorQualification,
    source_digest: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ExecutorQualificationError {
    #[error("executor qualification document is invalid: {message}")]
    InvalidDocument { message: String },
    #[error("unsupported executor qualification version '{version}'")]
    UnsupportedVersion { version: String },
}

impl ExecutorQualification {
    pub fn evaluate(
        &self,
        current: &ExecutorConfiguration,
        operation_digest: &str,
        expected: &ExecutorQualificationExpectedBindings,
    ) -> ExecutorEligibility {
        let mut missing_layers = Vec::new();
        if !self.record.protocol.valid || self.record.protocol.version != expected.protocol_version
        {
            missing_layers.push(QualificationLayer::ProtocolValid);
        }
        if !self.record.agentdoc_evaluation.qualified() {
            missing_layers.push(QualificationLayer::AgentDocEvaluated);
        }
        if !self.record.organization_approval.approved
            || !self
                .record
                .organization_approval
                .scope
                .contains(&expected.requested_scope)
            || !self
                .record
                .organization_approval
                .risk
                .contains(&expected.requested_risk)
            || !self
                .record
                .organization_approval
                .deployment
                .contains(&expected.requested_deployment)
        {
            missing_layers.push(QualificationLayer::OrganizationApproved);
        }
        if !self.record.runtime_policy.eligible
            || self.record.runtime_policy.operation_digest != operation_digest
        {
            missing_layers.push(QualificationLayer::RuntimePolicyEligible);
        }
        if self.source_digest != expected.record_digest
            || self.record.qualification_id != expected.qualification_id
            || self.record.capability.name != expected.capability_name
            || self.record.capability.version != expected.capability_version
        {
            push_missing(&mut missing_layers, QualificationLayer::AgentDocEvaluated);
        }
        if self.record.organization_approval.policy_digest != expected.organization_policy_digest {
            push_missing(
                &mut missing_layers,
                QualificationLayer::OrganizationApproved,
            );
        }
        if self.record.runtime_policy.policy_digest != expected.runtime_policy_digest {
            push_missing(
                &mut missing_layers,
                QualificationLayer::RuntimePolicyEligible,
            );
        }
        let requalification_triggers = configuration_changes(&self.record.subject, current);
        let authority = if missing_layers.is_empty() && requalification_triggers.is_empty() {
            ExecutorAuthority::GateAuthoritative
        } else if self.record.protocol.valid {
            ExecutorAuthority::AdvisoryOnly
        } else {
            ExecutorAuthority::Rejected
        };
        ExecutorEligibility {
            authority,
            missing_layers,
            requalification_triggers,
        }
    }

    pub fn to_canonical_json(&self) -> Result<String, ExecutorQualificationError> {
        let mut json = serde_json::to_string_pretty(&self.record)
            .map_err(|error| invalid(error.to_string()))?;
        json.push('\n');
        Ok(json)
    }
}

pub fn validate_executor_qualification(
    bytes: &[u8],
) -> Result<ExecutorQualification, ExecutorQualificationError> {
    let mut record: RawExecutorQualification =
        serde_json::from_slice(bytes).map_err(|error| invalid(error.to_string()))?;
    if record.schema_version != EXECUTOR_QUALIFICATION_SCHEMA_VERSION {
        return Err(ExecutorQualificationError::UnsupportedVersion {
            version: record.schema_version,
        });
    }
    require_text("qualification_id", &record.qualification_id)?;
    require_text("capability.name", &record.capability.name)?;
    require_text("capability.version", &record.capability.version)?;
    require_text("protocol.version", &record.protocol.version)?;
    validate_subject(&record.subject, &record.agentdoc_evaluation)?;
    validate_approval(&record.organization_approval)?;
    record.organization_approval.scope.sort();
    record.organization_approval.risk.sort();
    record.organization_approval.deployment.sort();
    require_digest(
        "runtime_policy.operation_digest",
        &record.runtime_policy.operation_digest,
    )?;
    require_digest(
        "runtime_policy.policy_digest",
        &record.runtime_policy.policy_digest,
    )?;
    Ok(ExecutorQualification {
        record,
        source_digest: sha256_prefixed(bytes),
    })
}

fn validate_subject(
    subject: &ExecutorConfiguration,
    evaluation: &AgentDocEvaluation,
) -> Result<(), ExecutorQualificationError> {
    match (subject, evaluation) {
        (
            ExecutorConfiguration::Model {
                provider,
                executor_digest,
                model_digest,
                config_digest,
                configuration,
            },
            AgentDocEvaluation::Capability { evidence_ref, .. },
        ) => {
            require_text("subject.provider", provider)?;
            require_text("agentdoc_evaluation.evidence_ref", evidence_ref)?;
            for (field, digest) in [
                ("subject.executor_digest", executor_digest),
                ("subject.model_digest", model_digest),
                ("subject.config_digest", config_digest),
                (
                    "configuration.model_revision_digest",
                    &configuration.model_revision_digest,
                ),
                (
                    "configuration.quantization_digest",
                    &configuration.quantization_digest,
                ),
                (
                    "configuration.system_prompt_task_digest",
                    &configuration.system_prompt_task_digest,
                ),
                (
                    "configuration.context_strategy_digest",
                    &configuration.context_strategy_digest,
                ),
                (
                    "configuration.output_constraints_digest",
                    &configuration.output_constraints_digest,
                ),
                (
                    "configuration.toolset_digest",
                    &configuration.toolset_digest,
                ),
                (
                    "configuration.inference_parameters_digest",
                    &configuration.inference_parameters_digest,
                ),
                (
                    "configuration.safety_configuration_digest",
                    &configuration.safety_configuration_digest,
                ),
                (
                    "configuration.adapter_implementation_digest",
                    &configuration.adapter_implementation_digest,
                ),
            ] {
                require_digest(field, digest)?;
            }
        }
        (
            ExecutorConfiguration::Human {
                principal_id,
                executor_digest,
                config_digest,
                permission_policy_digest,
            },
            AgentDocEvaluation::AuthenticatedPermission {
                principal_id: evaluated_principal,
                permission_policy_digest: evaluated_policy,
                ..
            },
        ) => {
            require_text("subject.principal_id", principal_id)?;
            require_digest("subject.executor_digest", executor_digest)?;
            require_digest("subject.config_digest", config_digest)?;
            require_digest("subject.permission_policy_digest", permission_policy_digest)?;
            if principal_id != evaluated_principal || permission_policy_digest != evaluated_policy {
                return Err(invalid(
                    "human evaluation must bind the exact authenticated principal and permission policy",
                ));
            }
        }
        _ => {
            return Err(invalid(
                "model executors require capability evaluation and human executors require authenticated permission evaluation",
            ));
        }
    }
    Ok(())
}

fn validate_approval(approval: &OrganizationApproval) -> Result<(), ExecutorQualificationError> {
    for (field, values) in [
        ("organization_approval.scope", &approval.scope),
        ("organization_approval.risk", &approval.risk),
        ("organization_approval.deployment", &approval.deployment),
    ] {
        if values.is_empty()
            || values.iter().any(|value| !is_semantic_context_text(value))
            || values
                .iter()
                .enumerate()
                .any(|(index, value)| values[index + 1..].contains(value))
        {
            return Err(invalid(format!("{field} must contain unique bound values")));
        }
    }
    require_digest(
        "organization_approval.policy_digest",
        &approval.policy_digest,
    )
}

fn configuration_changes(
    qualified: &ExecutorConfiguration,
    current: &ExecutorConfiguration,
) -> Vec<RequalificationTrigger> {
    match (qualified, current) {
        (
            ExecutorConfiguration::Model {
                provider,
                executor_digest,
                model_digest,
                config_digest,
                configuration,
            },
            ExecutorConfiguration::Model {
                provider: current_provider,
                executor_digest: current_executor,
                model_digest: current_model,
                config_digest: current_config,
                configuration: current,
            },
        ) => {
            let mut triggers = Vec::new();
            push_if(
                &mut triggers,
                provider != current_provider
                    || model_digest != current_model
                    || configuration.model_revision_digest != current.model_revision_digest,
                RequalificationTrigger::ModelRevision,
            );
            push_if(
                &mut triggers,
                configuration.quantization_digest != current.quantization_digest,
                RequalificationTrigger::Quantization,
            );
            push_if(
                &mut triggers,
                configuration.system_prompt_task_digest != current.system_prompt_task_digest,
                RequalificationTrigger::SystemPromptOrTaskDefinition,
            );
            push_if(
                &mut triggers,
                configuration.context_strategy_digest != current.context_strategy_digest,
                RequalificationTrigger::ContextStrategy,
            );
            push_if(
                &mut triggers,
                configuration.output_constraints_digest != current.output_constraints_digest,
                RequalificationTrigger::OutputConstraints,
            );
            push_if(
                &mut triggers,
                configuration.toolset_digest != current.toolset_digest,
                RequalificationTrigger::ToolAvailability,
            );
            push_if(
                &mut triggers,
                configuration.inference_parameters_digest != current.inference_parameters_digest,
                RequalificationTrigger::InferenceParameters,
            );
            push_if(
                &mut triggers,
                configuration.safety_configuration_digest != current.safety_configuration_digest,
                RequalificationTrigger::SafetyConfiguration,
            );
            push_if(
                &mut triggers,
                executor_digest != current_executor
                    || configuration.adapter_implementation_digest
                        != current.adapter_implementation_digest,
                RequalificationTrigger::AdapterImplementation,
            );
            let aggregate_only_changed = config_digest != current_config && triggers.is_empty();
            push_if(
                &mut triggers,
                aggregate_only_changed,
                RequalificationTrigger::ConfigurationDigest,
            );
            triggers
        }
        (
            ExecutorConfiguration::Human {
                principal_id,
                executor_digest,
                config_digest,
                permission_policy_digest,
            },
            ExecutorConfiguration::Human {
                principal_id: current_principal,
                executor_digest: current_executor,
                config_digest: current_config,
                permission_policy_digest: current_policy,
            },
        ) => {
            let mut triggers = Vec::new();
            push_if(
                &mut triggers,
                principal_id != current_principal,
                RequalificationTrigger::AuthenticatedPrincipal,
            );
            push_if(
                &mut triggers,
                permission_policy_digest != current_policy,
                RequalificationTrigger::PermissionPolicy,
            );
            push_if(
                &mut triggers,
                executor_digest != current_executor,
                RequalificationTrigger::AdapterImplementation,
            );
            let aggregate_only_changed = config_digest != current_config && triggers.is_empty();
            push_if(
                &mut triggers,
                aggregate_only_changed,
                RequalificationTrigger::ConfigurationDigest,
            );
            triggers
        }
        _ => vec![RequalificationTrigger::ExecutorKind],
    }
}

fn push_if<T>(values: &mut Vec<T>, condition: bool, value: T) {
    if condition {
        values.push(value);
    }
}

fn push_missing(values: &mut Vec<QualificationLayer>, value: QualificationLayer) {
    if !values.contains(&value) {
        values.push(value);
    }
}

fn require_text(field: &str, value: &str) -> Result<(), ExecutorQualificationError> {
    if is_semantic_context_text(value) {
        Ok(())
    } else {
        Err(invalid(format!("{field} must be non-blank")))
    }
}

fn require_digest(field: &str, value: &str) -> Result<(), ExecutorQualificationError> {
    if is_sha256_digest(value) {
        Ok(())
    } else {
        Err(invalid(format!("{field} must be a sha256 digest")))
    }
}

fn invalid(message: impl Into<String>) -> ExecutorQualificationError {
    ExecutorQualificationError::InvalidDocument {
        message: message.into(),
    }
}
