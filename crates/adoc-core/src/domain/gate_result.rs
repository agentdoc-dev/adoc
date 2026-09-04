//! Deterministic four-mode gate result (`adoc.gate_result.v0`, E5.3).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::semantic_context::{is_semantic_context_text, is_sha256_digest};

pub const GATE_RESULT_SCHEMA_VERSION: &str = "adoc.gate_result.v0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateMode {
    Advisory,
    AssessmentRequired,
    ProposalRequired,
    ApprovalRequired,
}

impl GateMode {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "advisory" => Some(Self::Advisory),
            "assessment_required" => Some(Self::AssessmentRequired),
            "proposal_required" => Some(Self::ProposalRequired),
            "approval_required" => Some(Self::ApprovalRequired),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateOutcome {
    Pass,
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum GateReason {
    #[serde(rename = "gate.approval_invalidated")]
    ApprovalInvalidated,
    #[serde(rename = "gate.approval_missing")]
    ApprovalMissing,
    #[serde(rename = "gate.assessment_missing")]
    AssessmentMissing,
    #[serde(rename = "gate.assessment_stale")]
    AssessmentStale,
    #[serde(rename = "gate.audit_persistence_failed")]
    AuditPersistenceFailed,
    #[serde(rename = "gate.cloud_unavailable")]
    CloudUnavailable,
    #[serde(rename = "gate.mode_unknown")]
    ModeUnknown,
    #[serde(rename = "gate.promotion_unapproved")]
    PromotionUnapproved,
    #[serde(rename = "gate.proposal_hash_mismatch")]
    ProposalHashMismatch,
    #[serde(rename = "gate.proposal_missing")]
    ProposalMissing,
    #[serde(rename = "gate.provider_failed_no_fallback")]
    ProviderFailedNoFallback,
    #[serde(rename = "gate.semantic_invalid")]
    SemanticInvalid,
}

/// A validated result. Wire bytes cannot bypass [`GateResult::new`] or
/// [`validate_gate_result`] because fields are private and this type does not
/// implement `Deserialize`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GateResult {
    schema_version: String,
    head_sha: String,
    policy_version: String,
    input_digests: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    configured_mode: Option<String>,
    effective_mode: Option<GateMode>,
    result: GateOutcome,
    reasons: Vec<GateReason>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GateResultError {
    #[error("gate result document is invalid: {message}")]
    InvalidDocument { message: String },
    #[error("unsupported gate result version '{version}'")]
    UnsupportedVersion { version: String },
    #[error("gate result field '{field}' is invalid")]
    InvalidField { field: String },
    #[error("unknown gate mode must block with gate.mode_unknown")]
    InvalidUnknownMode,
}

impl GateResult {
    pub fn new(
        head_sha: String,
        policy_version: String,
        input_digests: Vec<String>,
        configured_mode: Option<String>,
        result: GateOutcome,
        reasons: Vec<GateReason>,
    ) -> Result<Self, GateResultError> {
        if !is_git_sha(&head_sha) {
            return Err(invalid_field("head_sha"));
        }
        if !is_semantic_context_text(&policy_version) {
            return Err(invalid_field("policy_version"));
        }
        if input_digests.iter().any(|digest| !is_sha256_digest(digest)) {
            return Err(invalid_field("input_digests"));
        }
        if configured_mode
            .as_deref()
            .is_some_and(|mode| mode.chars().count() > 128 || mode.chars().any(char::is_control))
        {
            return Err(invalid_field("configured_mode"));
        }
        let effective_mode = configured_mode
            .as_deref()
            .map(GateMode::parse)
            .unwrap_or(Some(GateMode::Advisory));
        let input_digests: Vec<_> = input_digests
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let reasons: Vec<_> = reasons
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if (result == GateOutcome::Pass && !reasons.is_empty())
            || (result == GateOutcome::Block && reasons.is_empty())
        {
            return Err(invalid_field("reasons"));
        }
        if (effective_mode.is_none()
            && (result != GateOutcome::Block || reasons != [GateReason::ModeUnknown]))
            || (effective_mode.is_some() && reasons.contains(&GateReason::ModeUnknown))
        {
            return Err(GateResultError::InvalidUnknownMode);
        }
        if result == GateOutcome::Pass
            && matches!(
                effective_mode,
                Some(
                    GateMode::AssessmentRequired
                        | GateMode::ProposalRequired
                        | GateMode::ApprovalRequired
                )
            )
            && input_digests.is_empty()
        {
            return Err(invalid_field("input_digests"));
        }

        Ok(Self {
            schema_version: GATE_RESULT_SCHEMA_VERSION.to_string(),
            head_sha,
            policy_version,
            input_digests,
            configured_mode,
            effective_mode,
            result,
            reasons,
        })
    }

    pub fn effective_mode(&self) -> Option<GateMode> {
        self.effective_mode
    }

    pub fn head_sha(&self) -> &str {
        &self.head_sha
    }

    pub fn policy_version(&self) -> &str {
        &self.policy_version
    }

    pub fn input_digests(&self) -> &[String] {
        &self.input_digests
    }

    pub fn configured_mode(&self) -> Option<&str> {
        self.configured_mode.as_deref()
    }

    pub fn result(&self) -> GateOutcome {
        self.result
    }

    pub fn reasons(&self) -> &[GateReason] {
        &self.reasons
    }

    pub fn to_canonical_json(&self) -> Result<String, GateResultError> {
        let mut json = serde_json::to_string_pretty(self).map_err(|error| {
            GateResultError::InvalidDocument {
                message: error.to_string(),
            }
        })?;
        json.push('\n');
        Ok(json)
    }
}

fn is_git_sha(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn invalid_field(field: &str) -> GateResultError {
    GateResultError::InvalidField {
        field: field.to_string(),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGateResult {
    schema_version: String,
    head_sha: String,
    policy_version: String,
    input_digests: Vec<String>,
    configured_mode: Option<String>,
    effective_mode: Option<GateMode>,
    result: GateOutcome,
    reasons: Vec<GateReason>,
}

pub fn validate_gate_result(bytes: &[u8]) -> Result<GateResult, GateResultError> {
    // Deserialize the closed struct before normalizing to `Value` so duplicate
    // object members cannot collapse to a different consumer's interpretation.
    let raw: RawGateResult =
        serde_json::from_slice(bytes).map_err(|error| GateResultError::InvalidDocument {
            message: error.to_string(),
        })?;
    let received: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|error| GateResultError::InvalidDocument {
            message: error.to_string(),
        })?;
    if raw.schema_version != GATE_RESULT_SCHEMA_VERSION {
        return Err(GateResultError::UnsupportedVersion {
            version: raw.schema_version,
        });
    }
    let effective_mode = raw.effective_mode;
    let rebuilt = GateResult::new(
        raw.head_sha,
        raw.policy_version,
        raw.input_digests,
        raw.configured_mode,
        raw.result,
        raw.reasons,
    )?;
    if effective_mode != rebuilt.effective_mode {
        return Err(GateResultError::InvalidDocument {
            message: "effective_mode does not match configured_mode".to_string(),
        });
    }
    if serde_json::to_value(&rebuilt).map_err(|error| GateResultError::InvalidDocument {
        message: error.to_string(),
    })? != received
    {
        return Err(GateResultError::InvalidDocument {
            message: "record fields do not match their canonical derivation".to_string(),
        });
    }
    Ok(rebuilt)
}
