//! `agent_instruction` Knowledge Object aggregate (PRD §13.13, ADR-0025).
//!
//! Required fields: `id`, `scope`, `trust`, `allowed_actions`,
//! `forbidden_actions`, `body`. Optional fields (e.g. `owner`) pass through to
//! the `OptionalFields` bag without error.
//!
//! Validation is aggregate-owned (V5.4 precedent, ADR-0031 §27):
//! - presence/format checks happen in `build_from_parsed`,
//! - disjointness of action sets is enforced by `DisjointActionSets::try_new`.

#[cfg(test)]
use std::collections::BTreeMap;

use crate::domain::ast::ParsedTypedBlock;
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, SourceSpan};
use crate::domain::identity::{OBJECT_ID_GRAMMAR_HELP, ObjectId, ObjectIdError};
use crate::domain::knowledge_object::Relations;
use crate::domain::value_objects::action::AllowedAction;
use crate::domain::value_objects::action::ForbiddenAction;
use crate::domain::value_objects::action_set::DisjointActionSets;
use crate::domain::value_objects::scope::Scope;
use crate::domain::value_objects::trust::{Trust, TrustError};
use crate::domain::values::{Body, OptionalFields, trim_ascii_edges};

const SCOPE_FIELD: &str = "scope";
const TRUST_FIELD: &str = "trust";
const ALLOWED_ACTIONS_FIELD: &str = "allowed_actions";
const FORBIDDEN_ACTIONS_FIELD: &str = "forbidden_actions";

const AGENT_INSTRUCTION_MISSING_SCOPE_HELP: &str =
    "Agent instructions require a non-empty `scope` field (e.g. `scope: docs/auth/*`).";
const AGENT_INSTRUCTION_MISSING_TRUST_HELP: &str = "Agent instructions require a `trust` field. Valid values: informal, team, authoritative, regulated, system.";
const AGENT_INSTRUCTION_INVALID_TRUST_HELP: &str =
    "Use a valid trust level: informal, team, authoritative, regulated, system.";
const AGENT_INSTRUCTION_MISSING_ALLOWED_ACTIONS_HELP: &str = "Agent instructions require `allowed_actions` listing at least one action. Use scalar (`allowed_actions: summarize`) or list (`allowed_actions: [summarize, cite]`) form.";
const AGENT_INSTRUCTION_MISSING_FORBIDDEN_ACTIONS_HELP: &str = "Agent instructions require `forbidden_actions` listing at least one action. Use scalar (`forbidden_actions: execute_shell`) or list (`forbidden_actions: [execute_shell, access_secrets]`) form.";
const AGENT_INSTRUCTION_MISSING_BODY_HELP: &str =
    "Agent instructions require non-empty body text describing the instruction.";

/// An agent instruction Knowledge Object (PRD §13.13).
///
/// Required fields: `id`, `scope`, `trust`, `allowed_actions`,
/// `forbidden_actions`, `body`. The `impacts:` field is not supported; any
/// other unrecognised fields go to the `OptionalFields` bag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentInstruction {
    id: ObjectId,
    scope: Scope,
    trust: Trust,
    action_set: DisjointActionSets,
    body: Body,
    fields: OptionalFields,
    relations: Relations,
    span: SourceSpan,
}

/// Why an `agent_instruction` failed to build from parsed input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AgentInstructionError {
    InvalidId(ObjectIdError),
    MissingScope,
    MissingTrust,
    InvalidTrust(String),
    MissingAllowedActions,
    MissingForbiddenActions,
    ActionsNotDisjoint(Vec<String>),
    MissingBody,
}

impl AgentInstruction {
    pub(crate) fn build_from_parsed(
        mut parsed: ParsedTypedBlock,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Self> {
        if super::reject_duplicate_fields(&parsed, "agent_instruction", diagnostics) {
            return None;
        }

        // Parse id first (needed for diagnostics).
        let id = match ObjectId::new(&parsed.id_text) {
            Ok(id) => Some(id),
            Err(error) => {
                emit_error(
                    &parsed,
                    AgentInstructionError::InvalidId(error),
                    diagnostics,
                );
                None
            }
        };

        // Parse scope.
        let scope_raw = parsed.raw_fields.remove(SCOPE_FIELD);
        let scope = match scope_raw.as_deref().and_then(Scope::try_new) {
            Some(s) => Some(s),
            None => {
                emit_error(&parsed, AgentInstructionError::MissingScope, diagnostics);
                None
            }
        };

        // Parse trust.
        let trust_raw = parsed.raw_fields.remove(TRUST_FIELD);
        let trust = match Trust::try_new(trust_raw.as_deref().unwrap_or("")) {
            Ok(t) => Some(t),
            Err(TrustError::Missing) => {
                emit_error(&parsed, AgentInstructionError::MissingTrust, diagnostics);
                None
            }
            Err(TrustError::Invalid(s)) => {
                emit_error(&parsed, AgentInstructionError::InvalidTrust(s), diagnostics);
                None
            }
        };

        // Parse allowed_actions via the shared list helper.
        let allowed_raw: Option<Vec<AllowedAction>> = super::extract_action_list(
            &mut parsed,
            ALLOWED_ACTIONS_FIELD,
            AllowedAction::try_new,
            diagnostics,
        );
        if allowed_raw.is_none() {
            emit_error(
                &parsed,
                AgentInstructionError::MissingAllowedActions,
                diagnostics,
            );
        }

        // Parse forbidden_actions via the shared list helper.
        let forbidden_raw: Option<Vec<ForbiddenAction>> = super::extract_action_list(
            &mut parsed,
            FORBIDDEN_ACTIONS_FIELD,
            ForbiddenAction::try_new,
            diagnostics,
        );
        if forbidden_raw.is_none() {
            emit_error(
                &parsed,
                AgentInstructionError::MissingForbiddenActions,
                diagnostics,
            );
        }

        // Enforce disjointness only when both lists are present.
        let action_set = match (allowed_raw, forbidden_raw) {
            (Some(allowed), Some(forbidden)) => {
                match DisjointActionSets::try_new(allowed, forbidden) {
                    Ok(set) => Some(set),
                    Err(overlap_err) => {
                        emit_error(
                            &parsed,
                            AgentInstructionError::ActionsNotDisjoint(
                                overlap_err.overlapping.clone(),
                            ),
                            diagnostics,
                        );
                        None
                    }
                }
            }
            _ => None,
        };

        // Parse body.
        let body = match super::body_from_parsed(&parsed) {
            Some(b) => Some(b),
            None => {
                emit_error(&parsed, AgentInstructionError::MissingBody, diagnostics);
                None
            }
        };

        // All required fields must be present.
        if id.is_none()
            || scope.is_none()
            || trust.is_none()
            || action_set.is_none()
            || body.is_none()
        {
            return None;
        }

        let relations = super::extract_relations(&mut parsed, diagnostics);
        // `impacts:` is not supported — any `impacts:` field goes to optional.
        let optional_fields = std::mem::take(&mut parsed.raw_fields);

        Some(Self {
            id: id.expect("checked above"),
            scope: scope.expect("checked above"),
            trust: trust.expect("checked above"),
            action_set: action_set.expect("checked above"),
            body: body.expect("checked above"),
            fields: OptionalFields::from_map(optional_fields),
            relations,
            span: parsed.span.clone(),
        })
    }

    pub(crate) fn id(&self) -> &ObjectId {
        &self.id
    }

    pub(crate) fn scope(&self) -> &Scope {
        &self.scope
    }

    pub(crate) fn trust(&self) -> &Trust {
        &self.trust
    }

    pub(crate) fn action_set(&self) -> &DisjointActionSets {
        &self.action_set
    }

    pub(crate) fn body(&self) -> &Body {
        &self.body
    }

    pub(crate) fn body_mut(&mut self) -> &mut Body {
        &mut self.body
    }

    pub(crate) fn fields(&self) -> &OptionalFields {
        &self.fields
    }

    pub(crate) fn relations(&self) -> &Relations {
        &self.relations
    }

    pub(crate) fn span(&self) -> &SourceSpan {
        &self.span
    }

    /// Test-only constructor that bypasses the parsed-block pipeline.
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn try_new(
        id_text: &str,
        scope_text: &str,
        trust_text: &str,
        allowed: Vec<&str>,
        forbidden: Vec<&str>,
        body_text: &str,
        optional_fields: BTreeMap<String, String>,
        span: SourceSpan,
    ) -> Result<Self, AgentInstructionError> {
        let id = ObjectId::new(id_text).map_err(AgentInstructionError::InvalidId)?;
        let scope = Scope::try_new(scope_text).ok_or(AgentInstructionError::MissingScope)?;
        let trust = match Trust::try_new(trust_text) {
            Ok(t) => t,
            Err(TrustError::Missing) => return Err(AgentInstructionError::MissingTrust),
            Err(TrustError::Invalid(s)) => return Err(AgentInstructionError::InvalidTrust(s)),
        };
        let allowed_actions: Vec<AllowedAction> = allowed
            .iter()
            .filter_map(|s| AllowedAction::try_new(s))
            .collect();
        if allowed_actions.is_empty() {
            return Err(AgentInstructionError::MissingAllowedActions);
        }
        let forbidden_actions: Vec<ForbiddenAction> = forbidden
            .iter()
            .filter_map(|s| ForbiddenAction::try_new(s))
            .collect();
        if forbidden_actions.is_empty() {
            return Err(AgentInstructionError::MissingForbiddenActions);
        }
        let action_set = DisjointActionSets::try_new(allowed_actions, forbidden_actions)
            .map_err(|e| AgentInstructionError::ActionsNotDisjoint(e.overlapping))?;
        let body = Body::from_plain_text(body_text).ok_or(AgentInstructionError::MissingBody)?;
        Ok(Self {
            id,
            scope,
            trust,
            action_set,
            body,
            fields: OptionalFields::from_map(optional_fields),
            relations: Relations::empty(),
            span,
        })
    }
}

fn emit_error(
    parsed: &ParsedTypedBlock,
    error: AgentInstructionError,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match error {
        AgentInstructionError::InvalidId(error) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::IdInvalid,
                format!(
                    "invalid agent_instruction id `{}`: {error}",
                    parsed.id_text
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(OBJECT_ID_GRAMMAR_HELP),
        ),
        AgentInstructionError::MissingScope => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaAgentInstructionMissingScope,
                format!(
                    "agent_instruction `{}` is missing required field `scope`",
                    parsed.id_text
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(AGENT_INSTRUCTION_MISSING_SCOPE_HELP),
        ),
        AgentInstructionError::MissingTrust => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaAgentInstructionMissingTrust,
                format!(
                    "agent_instruction `{}` is missing required field `trust`",
                    parsed.id_text
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(AGENT_INSTRUCTION_MISSING_TRUST_HELP),
        ),
        AgentInstructionError::InvalidTrust(value) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaAgentInstructionInvalidTrust,
                format!(
                    "agent_instruction `{}` has invalid `trust` value `{}`; valid values: informal, team, authoritative, regulated, system",
                    parsed.id_text, trim_ascii_edges(&value)
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(AGENT_INSTRUCTION_INVALID_TRUST_HELP),
        ),
        AgentInstructionError::MissingAllowedActions => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaAgentInstructionMissingAllowedActions,
                format!(
                    "agent_instruction `{}` is missing required field `allowed_actions`",
                    parsed.id_text
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(AGENT_INSTRUCTION_MISSING_ALLOWED_ACTIONS_HELP),
        ),
        AgentInstructionError::MissingForbiddenActions => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaAgentInstructionMissingForbiddenActions,
                format!(
                    "agent_instruction `{}` is missing required field `forbidden_actions`",
                    parsed.id_text
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(AGENT_INSTRUCTION_MISSING_FORBIDDEN_ACTIONS_HELP),
        ),
        AgentInstructionError::ActionsNotDisjoint(overlapping) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaAgentInstructionActionsNotDisjoint,
                format!(
                    "agent_instruction `{}` has overlapping `allowed_actions` and `forbidden_actions`: {}",
                    parsed.id_text,
                    overlapping.join(", ")
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help("Remove the overlapping actions so `allowed_actions` and `forbidden_actions` are disjoint."),
        ),
        AgentInstructionError::MissingBody => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaMissingField,
                format!(
                    "agent_instruction `{}` is missing required body",
                    parsed.id_text
                ),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(AGENT_INSTRUCTION_MISSING_BODY_HELP),
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use super::*;
    use crate::domain::ast::ParsedTypedBlock;
    use crate::domain::diagnostic::{DiagnosticCode, SourcePosition, SourceSpan};

    fn span() -> SourceSpan {
        SourceSpan {
            file: PathBuf::from("test.adoc"),
            start: SourcePosition {
                line: 1,
                column: 1,
                offset: 0,
            },
            end: SourcePosition {
                line: 1,
                column: 8,
                offset: 7,
            },
        }
    }

    fn parsed_agent_instruction(
        fields: BTreeMap<String, String>,
        body_text: &str,
    ) -> ParsedTypedBlock {
        ParsedTypedBlock {
            kind_word: "agent_instruction".to_string(),
            kind_word_span: span(),
            id_text: "auth.docs-answering-policy".to_string(),
            raw_fields: fields,
            raw_field_spans: BTreeMap::new(),
            duplicate_keys: Vec::new(),
            body_text: body_text.to_string(),
            body_inlines: ParsedTypedBlock::test_body_inlines_from_text(body_text),
            body_spans: Vec::new(),
            content_spans: Vec::new(),
            span: span(),
        }
    }

    fn valid_fields() -> BTreeMap<String, String> {
        BTreeMap::from([
            (SCOPE_FIELD.to_string(), "docs/auth/*".to_string()),
            (TRUST_FIELD.to_string(), "team".to_string()),
            (
                ALLOWED_ACTIONS_FIELD.to_string(),
                "[summarize, cite]".to_string(),
            ),
            (
                FORBIDDEN_ACTIONS_FIELD.to_string(),
                "[execute_shell, access_secrets]".to_string(),
            ),
        ])
    }

    const BODY: &str = "Prefer verified claims over draft notes when answering auth questions.";

    // ── try_new tests ───────────────────────────────────────────────────────

    #[test]
    fn try_new_accepts_valid_agent_instruction() {
        let ai = AgentInstruction::try_new(
            "auth.docs-answering-policy",
            "docs/auth/*",
            "team",
            vec!["summarize", "cite"],
            vec!["execute_shell", "access_secrets"],
            BODY,
            BTreeMap::new(),
            span(),
        )
        .expect("valid agent instruction");

        assert_eq!(ai.id().as_str(), "auth.docs-answering-policy");
        assert_eq!(ai.scope().as_str(), "docs/auth/*");
        assert_eq!(ai.trust().as_str(), "team");
        assert_eq!(ai.body().to_source(), BODY);
    }

    #[test]
    fn try_new_rejects_overlapping_actions() {
        let err = AgentInstruction::try_new(
            "auth.docs-answering-policy",
            "docs/auth/*",
            "team",
            vec!["summarize", "cite"],
            vec!["execute_shell", "cite"],
            BODY,
            BTreeMap::new(),
            span(),
        )
        .expect_err("overlapping actions");
        assert!(matches!(err, AgentInstructionError::ActionsNotDisjoint(_)));
    }

    #[test]
    fn try_new_rejects_invalid_trust() {
        let err = AgentInstruction::try_new(
            "auth.docs-answering-policy",
            "docs/auth/*",
            "internal",
            vec!["summarize"],
            vec!["execute_shell"],
            BODY,
            BTreeMap::new(),
            span(),
        )
        .expect_err("invalid trust");
        assert!(matches!(err, AgentInstructionError::InvalidTrust(_)));
    }

    // ── build_from_parsed — missing required fields ────────────────────────

    #[test]
    fn build_from_parsed_reports_missing_scope() {
        let mut fields = valid_fields();
        fields.remove(SCOPE_FIELD);
        let parsed = parsed_agent_instruction(fields, BODY);
        let mut diagnostics = Vec::new();

        let result = AgentInstruction::build_from_parsed(parsed, &mut diagnostics);

        assert!(result.is_none());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::SchemaAgentInstructionMissingScope),
            "expected MissingScope, got: {diagnostics:?}"
        );
    }

    #[test]
    fn build_from_parsed_reports_missing_trust() {
        let mut fields = valid_fields();
        fields.remove(TRUST_FIELD);
        let parsed = parsed_agent_instruction(fields, BODY);
        let mut diagnostics = Vec::new();

        let result = AgentInstruction::build_from_parsed(parsed, &mut diagnostics);

        assert!(result.is_none());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::SchemaAgentInstructionMissingTrust),
            "expected MissingTrust, got: {diagnostics:?}"
        );
    }

    #[test]
    fn build_from_parsed_reports_invalid_trust() {
        let mut fields = valid_fields();
        fields.insert(TRUST_FIELD.to_string(), "internal".to_string());
        let parsed = parsed_agent_instruction(fields, BODY);
        let mut diagnostics = Vec::new();

        let result = AgentInstruction::build_from_parsed(parsed, &mut diagnostics);

        assert!(result.is_none());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::SchemaAgentInstructionInvalidTrust),
            "expected InvalidTrust, got: {diagnostics:?}"
        );
    }

    #[test]
    fn build_from_parsed_reports_missing_allowed_actions() {
        let mut fields = valid_fields();
        fields.remove(ALLOWED_ACTIONS_FIELD);
        let parsed = parsed_agent_instruction(fields, BODY);
        let mut diagnostics = Vec::new();

        let result = AgentInstruction::build_from_parsed(parsed, &mut diagnostics);

        assert!(result.is_none());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::SchemaAgentInstructionMissingAllowedActions),
            "expected MissingAllowedActions, got: {diagnostics:?}"
        );
    }

    #[test]
    fn build_from_parsed_reports_missing_forbidden_actions() {
        let mut fields = valid_fields();
        fields.remove(FORBIDDEN_ACTIONS_FIELD);
        let parsed = parsed_agent_instruction(fields, BODY);
        let mut diagnostics = Vec::new();

        let result = AgentInstruction::build_from_parsed(parsed, &mut diagnostics);

        assert!(result.is_none());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::SchemaAgentInstructionMissingForbiddenActions),
            "expected MissingForbiddenActions, got: {diagnostics:?}"
        );
    }

    #[test]
    fn build_from_parsed_reports_actions_not_disjoint() {
        let mut fields = valid_fields();
        fields.insert(
            ALLOWED_ACTIONS_FIELD.to_string(),
            "[summarize, cite]".to_string(),
        );
        fields.insert(
            FORBIDDEN_ACTIONS_FIELD.to_string(),
            "[execute_shell, cite]".to_string(),
        );
        let parsed = parsed_agent_instruction(fields, BODY);
        let mut diagnostics = Vec::new();

        let result = AgentInstruction::build_from_parsed(parsed, &mut diagnostics);

        assert!(result.is_none());
        let disjoint_diag = diagnostics
            .iter()
            .find(|d| d.code == DiagnosticCode::SchemaAgentInstructionActionsNotDisjoint);
        assert!(
            disjoint_diag.is_some(),
            "expected ActionsNotDisjoint, got: {diagnostics:?}"
        );
        assert!(
            disjoint_diag.unwrap().message.contains("cite"),
            "diagnostic message must name the overlapping action"
        );
    }

    #[test]
    fn build_from_parsed_reports_missing_body() {
        let parsed = parsed_agent_instruction(valid_fields(), "   ");
        let mut diagnostics = Vec::new();

        let result = AgentInstruction::build_from_parsed(parsed, &mut diagnostics);

        assert!(result.is_none());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::SchemaMissingField),
            "expected SchemaMissingField for missing body, got: {diagnostics:?}"
        );
    }

    // ── build_from_parsed — valid ──────────────────────────────────────────

    #[test]
    fn build_from_parsed_accepts_full_valid_agent_instruction() {
        let parsed = parsed_agent_instruction(valid_fields(), BODY);
        let mut diagnostics = Vec::new();

        let ai = AgentInstruction::build_from_parsed(parsed, &mut diagnostics)
            .expect("valid agent instruction");

        assert!(diagnostics.is_empty(), "diagnostics: {diagnostics:?}");
        assert_eq!(ai.id().as_str(), "auth.docs-answering-policy");
        assert_eq!(ai.scope().as_str(), "docs/auth/*");
        assert_eq!(ai.trust().as_str(), "team");
        assert_eq!(ai.body().to_source(), BODY);
    }

    #[test]
    fn build_from_parsed_accepts_scalar_allowed_actions() {
        let mut fields = valid_fields();
        fields.insert(ALLOWED_ACTIONS_FIELD.to_string(), "summarize".to_string());
        let parsed = parsed_agent_instruction(fields, BODY);
        let mut diagnostics = Vec::new();

        let ai = AgentInstruction::build_from_parsed(parsed, &mut diagnostics)
            .expect("valid agent instruction");

        assert!(diagnostics.is_empty(), "diagnostics: {diagnostics:?}");
        assert_eq!(ai.action_set().allowed().len(), 1);
        assert_eq!(ai.action_set().allowed()[0].as_str(), "summarize");
    }

    #[test]
    fn build_from_parsed_collects_multiple_errors() {
        // Both scope and trust are missing — both diagnostics are emitted.
        let parsed = parsed_agent_instruction(BTreeMap::new(), BODY);
        let mut diagnostics = Vec::new();

        let result = AgentInstruction::build_from_parsed(parsed, &mut diagnostics);

        assert!(result.is_none());
        let codes: Vec<_> = diagnostics.iter().map(|d| d.code).collect();
        assert!(
            codes.contains(&DiagnosticCode::SchemaAgentInstructionMissingScope),
            "expected missing scope, got: {codes:?}"
        );
        assert!(
            codes.contains(&DiagnosticCode::SchemaAgentInstructionMissingTrust),
            "expected missing trust, got: {codes:?}"
        );
    }

    #[test]
    fn build_from_parsed_passes_unrecognised_fields_to_optional_bag() {
        let mut fields = valid_fields();
        fields.insert("owner".to_string(), "ai-platform".to_string());
        let parsed = parsed_agent_instruction(fields, BODY);
        let mut diagnostics = Vec::new();

        let ai = AgentInstruction::build_from_parsed(parsed, &mut diagnostics)
            .expect("valid agent instruction");

        assert!(diagnostics.is_empty(), "diagnostics: {diagnostics:?}");
        assert_eq!(
            ai.fields()
                .iter()
                .find(|(k, _)| k.as_str() == "owner")
                .map(|(_, v)| v.as_str()),
            Some("ai-platform")
        );
    }
}
