//! `task` Knowledge Object aggregate (V6.5.4, PRD §13.11).
//!
//! A documentation action item. Required fields: `id`, `status`, `owner`,
//! `body` — task is the only kind beyond `policy` requiring `owner`
//! unconditionally (a task without an owner is a wish). Statuses are the
//! closed `open | done` set. Optional: `due` (a `YYYY-MM-DD` date reusing the
//! [`EffectiveDate`] value object). The clock-dependent `task.overdue`
//! lifecycle warning lives in `infrastructure/validate/task_overdue.rs`.

#[cfg(test)]
use std::collections::BTreeMap;

use crate::domain::ast::ParsedTypedBlock;
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, SourceSpan};
use crate::domain::identity::{OBJECT_ID_GRAMMAR_HELP, ObjectId, ObjectIdError};
use crate::domain::knowledge_object::Relations;
use crate::domain::knowledge_object::claim::{OWNER_FIELD, Owner};
use crate::domain::value_objects::effective_date::{EffectiveDate, EffectiveDateError};
use crate::domain::values::{Body, OptionalFields, trim_ascii_edges};

const STATUS_FIELD: &str = "status";
pub(crate) const DUE_FIELD: &str = "due";

const TASK_MISSING_BODY_HELP: &str = "Tasks require non-empty body text describing the action.";

/// A documentation action item (PRD §13.11, V6.5.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Task {
    id: ObjectId,
    status: TaskStatus,
    owner: Owner,
    due: Option<EffectiveDate>,
    body: Body,
    fields: OptionalFields,
    relations: Relations,
    span: SourceSpan,
}

/// Why a `task` failed to build from parsed input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TaskError {
    InvalidId(ObjectIdError),
    MissingStatus,
    InvalidStatus(String),
    MissingOwner,
    InvalidDue(String),
    MissingBody,
}

impl Task {
    pub(crate) fn build_from_parsed(
        mut parsed: ParsedTypedBlock,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Self> {
        if super::reject_duplicate_fields(&parsed, "task", diagnostics) {
            return None;
        }

        let id = match ObjectId::new(&parsed.id_text) {
            Ok(id) => Some(id),
            Err(error) => {
                emit_task_error(&parsed, TaskError::InvalidId(error), diagnostics);
                None
            }
        };

        let status_raw = parsed.raw_fields.remove(STATUS_FIELD);
        let status = match TaskStatus::try_new(status_raw.as_deref().unwrap_or("")) {
            Ok(status) => Some(status),
            Err(error) => {
                emit_task_error(&parsed, error, diagnostics);
                None
            }
        };

        let owner_raw = parsed.raw_fields.remove(OWNER_FIELD);
        let owner = match owner_raw.as_deref().and_then(Owner::try_new) {
            Some(owner) => Some(owner),
            None => {
                emit_task_error(&parsed, TaskError::MissingOwner, diagnostics);
                None
            }
        };

        // Parse due (optional; blank value → treat as absent).
        let due = match super::take_optional_scalar(&mut parsed, DUE_FIELD, EffectiveDate::try_new)
        {
            Ok(due) => Some(due),
            // `Missing` is unreachable: `take_optional_scalar` filters blank
            // input before the ctor runs. If that invariant ever breaks,
            // surface a diagnostic instead of silently dropping the field.
            Err(error) => {
                let value = match error {
                    EffectiveDateError::Invalid(value) => value,
                    EffectiveDateError::Missing => String::new(),
                };
                emit_task_error(&parsed, TaskError::InvalidDue(value), diagnostics);
                None
            }
        };

        let body = match super::body_from_parsed(&parsed) {
            Some(body) => Some(body),
            None => {
                emit_task_error(&parsed, TaskError::MissingBody, diagnostics);
                None
            }
        };

        if id.is_none() || status.is_none() || owner.is_none() || due.is_none() || body.is_none() {
            return None;
        }

        let relations = super::extract_relations(&mut parsed, diagnostics);
        let optional_fields = std::mem::take(&mut parsed.raw_fields);

        Some(Self {
            id: id.expect("checked above"),
            status: status.expect("checked above"),
            owner: owner.expect("checked above"),
            due: due.expect("checked above"),
            body: body.expect("checked above"),
            fields: OptionalFields::from_map(optional_fields),
            relations,
            span: parsed.span.clone(),
        })
    }

    // ── Accessors ────────────────────────────────────────────────────────────

    pub(crate) fn id(&self) -> &ObjectId {
        &self.id
    }

    pub(crate) fn status(&self) -> &TaskStatus {
        &self.status
    }

    pub(crate) fn owner(&self) -> &Owner {
        &self.owner
    }

    pub(crate) fn due(&self) -> Option<&EffectiveDate> {
        self.due.as_ref()
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
        status_text: &str,
        owner_text: &str,
        due_text: Option<&str>,
        body_text: &str,
        optional_fields: BTreeMap<String, String>,
        span: SourceSpan,
    ) -> Result<Self, TaskError> {
        let id = ObjectId::new(id_text).map_err(TaskError::InvalidId)?;
        let status = TaskStatus::try_new(status_text)?;
        let owner = Owner::try_new(owner_text).ok_or(TaskError::MissingOwner)?;
        // Present-but-blank `due` is absent, matching the parser path
        // (`parse_due`).
        let due = match due_text {
            Some(raw) => match EffectiveDate::try_new(raw) {
                Ok(date) => Some(date),
                Err(EffectiveDateError::Missing) => None,
                Err(EffectiveDateError::Invalid(value)) => {
                    return Err(TaskError::InvalidDue(value));
                }
            },
            None => None,
        };
        let body = Body::from_plain_text(body_text).ok_or(TaskError::MissingBody)?;
        Ok(Self {
            id,
            status,
            owner,
            due,
            body,
            fields: OptionalFields::from_map(optional_fields),
            relations: Relations::empty(),
            span,
        })
    }
}

fn emit_task_error(parsed: &ParsedTypedBlock, error: TaskError, diagnostics: &mut Vec<Diagnostic>) {
    let diagnostic = match error {
        TaskError::InvalidId(error) => Diagnostic::error(
            DiagnosticCode::IdInvalid,
            format!("invalid task id `{}`: {error}", parsed.id_text),
        )
        .with_help(OBJECT_ID_GRAMMAR_HELP),
        TaskError::MissingStatus => Diagnostic::error(
            DiagnosticCode::SchemaTaskMissingStatus,
            format!(
                "task `{}` is missing required field `status`",
                parsed.id_text
            ),
        )
        .with_help(DiagnosticCode::SchemaTaskMissingStatus.default_help()),
        TaskError::InvalidStatus(status) => Diagnostic::error(
            DiagnosticCode::SchemaTaskInvalidStatus,
            format!("task `{}` has invalid status `{status}`", parsed.id_text),
        )
        .with_help(DiagnosticCode::SchemaTaskInvalidStatus.default_help()),
        TaskError::MissingOwner => Diagnostic::error(
            DiagnosticCode::SchemaTaskMissingOwner,
            format!(
                "task `{}` is missing required field `owner`",
                parsed.id_text
            ),
        )
        .with_help(DiagnosticCode::SchemaTaskMissingOwner.default_help()),
        TaskError::InvalidDue(value) => Diagnostic::error(
            DiagnosticCode::SchemaTaskInvalidDue,
            format!(
                "task `{}` has invalid `due` value `{value}`",
                parsed.id_text
            ),
        )
        .with_help(DiagnosticCode::SchemaTaskInvalidDue.default_help()),
        TaskError::MissingBody => Diagnostic::error(
            DiagnosticCode::SchemaMissingField,
            format!("task `{}` is missing required body", parsed.id_text),
        )
        .with_help(TASK_MISSING_BODY_HELP),
    };
    diagnostics.push(
        diagnostic
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text),
    );
}

/// Task lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskStatus {
    Open,
    Done,
}

impl TaskStatus {
    pub(crate) fn try_new(value: &str) -> Result<Self, TaskError> {
        let trimmed = trim_ascii_edges(value);
        if trimmed.is_empty() {
            return Err(TaskError::MissingStatus);
        }
        match trimmed {
            "open" => Ok(Self::Open),
            "done" => Ok(Self::Done),
            _ => Err(TaskError::InvalidStatus(trimmed.to_string())),
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Done => "done",
        }
    }

    pub(crate) fn is_open(self) -> bool {
        matches!(self, Self::Open)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::domain::diagnostic::{SourcePosition, SourceSpan};

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

    const BODY: &str =
        "Update the support runbook to mention refund behavior after persistence failure.";

    fn parsed_task(fields: BTreeMap<String, String>, body_text: &str) -> ParsedTypedBlock {
        ParsedTypedBlock {
            kind_word: "task".to_string(),
            kind_word_span: span(),
            id_text: "billing.update-support-runbook".to_string(),
            raw_fields: fields,
            raw_field_spans: BTreeMap::new(),
            duplicate_keys: Vec::new(),
            body_text: body_text.to_string(),
            body_inlines: ParsedTypedBlock::test_body_inlines_from_text(body_text),
            body_spans: Vec::new(),
            content_spans: Vec::new(),
            span: span(),
            close_fence_span: span(),
            body_separator_span: None,
        }
    }

    fn valid_fields() -> BTreeMap<String, String> {
        BTreeMap::from([
            (STATUS_FIELD.to_string(), "open".to_string()),
            (OWNER_FIELD.to_string(), "support-ops".to_string()),
            (DUE_FIELD.to_string(), "2026-05-20".to_string()),
        ])
    }

    // ── TaskStatus tests ────────────────────────────────────────────────────

    #[test]
    fn status_try_new_rejects_empty() {
        assert_eq!(TaskStatus::try_new("  "), Err(TaskError::MissingStatus));
    }

    #[test]
    fn status_try_new_rejects_unknown_values() {
        assert_eq!(
            TaskStatus::try_new("closed"),
            Err(TaskError::InvalidStatus("closed".to_string()))
        );
    }

    #[test]
    fn status_try_new_accepts_closed_set_and_trims() {
        assert!(TaskStatus::try_new("  open  ").expect("open").is_open());
        assert_eq!(TaskStatus::try_new("done").expect("done"), TaskStatus::Done);
        assert!(!TaskStatus::Done.is_open());
    }

    // ── build_from_parsed — the PRD §13.11 example ──────────────────────────

    #[test]
    fn build_from_parsed_accepts_the_prd_example() {
        let mut fields = valid_fields();
        fields.insert(
            "depends_on".to_string(),
            "billing.credits.refund-on-failed-persistence".to_string(),
        );
        let parsed = parsed_task(fields, BODY);
        let mut diagnostics = Vec::new();

        let task = Task::build_from_parsed(parsed, &mut diagnostics).expect("valid task");

        assert!(diagnostics.is_empty(), "diagnostics: {diagnostics:?}");
        assert_eq!(task.id().as_str(), "billing.update-support-runbook");
        assert!(task.status().is_open());
        assert_eq!(task.owner().as_str(), "support-ops");
        assert_eq!(task.due().map(EffectiveDate::as_str), Some("2026-05-20"));
        assert_eq!(task.body().to_source(), BODY);
        let depends_on: Vec<&str> = task
            .relations()
            .targets(crate::domain::graph::GraphRelationKind::DependsOn)
            .iter()
            .map(|target| target.id().as_str())
            .collect();
        assert_eq!(
            depends_on,
            vec!["billing.credits.refund-on-failed-persistence"]
        );
    }

    #[test]
    fn build_from_parsed_accepts_done_task_without_due() {
        let mut fields = valid_fields();
        fields.insert(STATUS_FIELD.to_string(), "done".to_string());
        fields.remove(DUE_FIELD);
        let parsed = parsed_task(fields, BODY);
        let mut diagnostics = Vec::new();

        let task = Task::build_from_parsed(parsed, &mut diagnostics).expect("valid task");

        assert!(diagnostics.is_empty(), "diagnostics: {diagnostics:?}");
        assert_eq!(task.status(), &TaskStatus::Done);
        assert!(task.due().is_none());
    }

    // ── build_from_parsed — missing/invalid required fields ─────────────────

    #[test]
    fn build_from_parsed_reports_missing_status() {
        let mut fields = valid_fields();
        fields.remove(STATUS_FIELD);
        let parsed = parsed_task(fields, BODY);
        let mut diagnostics = Vec::new();

        let task = Task::build_from_parsed(parsed, &mut diagnostics);

        assert!(task.is_none());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::SchemaTaskMissingStatus),
            "expected SchemaTaskMissingStatus, got: {diagnostics:?}"
        );
    }

    #[test]
    fn build_from_parsed_reports_invalid_status() {
        let mut fields = valid_fields();
        fields.insert(STATUS_FIELD.to_string(), "blocked".to_string());
        let parsed = parsed_task(fields, BODY);
        let mut diagnostics = Vec::new();

        let task = Task::build_from_parsed(parsed, &mut diagnostics);

        assert!(task.is_none());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::SchemaTaskInvalidStatus),
            "expected SchemaTaskInvalidStatus, got: {diagnostics:?}"
        );
    }

    #[test]
    fn build_from_parsed_reports_missing_owner() {
        let mut fields = valid_fields();
        fields.remove(OWNER_FIELD);
        let parsed = parsed_task(fields, BODY);
        let mut diagnostics = Vec::new();

        let task = Task::build_from_parsed(parsed, &mut diagnostics);

        assert!(task.is_none());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::SchemaTaskMissingOwner),
            "expected SchemaTaskMissingOwner, got: {diagnostics:?}"
        );
    }

    #[test]
    fn build_from_parsed_reports_invalid_due() {
        let mut fields = valid_fields();
        fields.insert(DUE_FIELD.to_string(), "not-a-date".to_string());
        let parsed = parsed_task(fields, BODY);
        let mut diagnostics = Vec::new();

        let task = Task::build_from_parsed(parsed, &mut diagnostics);

        assert!(task.is_none());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::SchemaTaskInvalidDue
                    && d.message.contains("invalid `due` value")),
            "expected invalid-due diagnostic, got: {diagnostics:?}"
        );
    }

    #[test]
    fn build_from_parsed_treats_blank_due_as_absent() {
        let mut fields = valid_fields();
        fields.insert(DUE_FIELD.to_string(), String::new());
        let parsed = parsed_task(fields, BODY);
        let mut diagnostics = Vec::new();

        let task = Task::build_from_parsed(parsed, &mut diagnostics);

        let task = task.expect("blank `due:` builds like an absent field");
        assert!(
            diagnostics.is_empty(),
            "expected no diagnostics, got: {diagnostics:?}"
        );
        assert!(task.due().is_none());
    }

    #[test]
    fn try_new_treats_blank_due_as_absent() {
        // Mirrors the parser path (`parse_due`): present-but-blank `due`
        // behaves like an absent field.
        let task = Task::try_new(
            "billing.update-support-runbook",
            "open",
            "support-ops",
            Some("   "),
            BODY,
            BTreeMap::new(),
            span(),
        )
        .expect("blank due is absent, not invalid");

        assert!(task.due().is_none());
    }

    #[test]
    fn build_from_parsed_reports_missing_body() {
        let parsed = parsed_task(valid_fields(), "   ");
        let mut diagnostics = Vec::new();

        let task = Task::build_from_parsed(parsed, &mut diagnostics);

        assert!(task.is_none());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::SchemaMissingField
                    && d.message.contains("missing required body")),
            "expected missing-body diagnostic, got: {diagnostics:?}"
        );
    }

    #[test]
    fn build_from_parsed_collects_multiple_errors() {
        let parsed = parsed_task(BTreeMap::new(), BODY);
        let mut diagnostics = Vec::new();

        let task = Task::build_from_parsed(parsed, &mut diagnostics);

        assert!(task.is_none());
        let codes: Vec<_> = diagnostics.iter().map(|d| d.code).collect();
        assert!(
            codes.contains(&DiagnosticCode::SchemaTaskMissingStatus),
            "expected missing status, got: {codes:?}"
        );
        assert!(
            codes.contains(&DiagnosticCode::SchemaTaskMissingOwner),
            "expected missing owner, got: {codes:?}"
        );
    }
}
