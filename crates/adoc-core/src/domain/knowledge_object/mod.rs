//! Aggregate family — populated by Slice 1.

use std::collections::BTreeSet;

use crate::domain::ast::ParsedTypedBlock;
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, SourcePosition, SourceSpan};
use crate::domain::graph::GraphRelationKind;
use crate::domain::identity::{OBJECT_ID_GRAMMAR_HELP, ObjectId};
use crate::domain::value_objects::approved_by::ApprovedBy;
use crate::domain::value_objects::rel_path::RelPath;
use crate::domain::values::{Body, NonEmpty};

pub(super) const IMPACTS_FIELD: &str = "impacts";
pub(crate) const APPROVED_BY_FIELD: &str = "approved_by";

pub(crate) mod agent_instruction;
pub(crate) mod claim;
pub(crate) mod constraint;
pub(crate) mod decision;
pub(crate) mod draft;
pub(crate) mod example;
pub(crate) mod glossary;
pub(crate) mod metadata;
pub(crate) mod policy;
pub(crate) mod procedure;
pub(crate) mod projection;
pub(crate) mod warning;

use agent_instruction::AgentInstruction;
use claim::Claim;
use constraint::Constraint;
use decision::Decision;
use example::Example;
use glossary::Glossary;
use policy::Policy;
use procedure::Procedure;
use warning::Warning;

pub(super) fn reject_duplicate_fields(
    parsed: &ParsedTypedBlock,
    kind_word: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if parsed.duplicate_keys.is_empty() {
        return false;
    }

    let mut emitted_keys = BTreeSet::new();
    for key in &parsed.duplicate_keys {
        if emitted_keys.insert(key.as_str()) {
            diagnostics.push(
                Diagnostic::error(
                    DiagnosticCode::SchemaDuplicateField,
                    format!("duplicate field `{key}` in {kind_word}"),
                )
                .with_span(parsed.span.clone()),
            );
        }
    }

    // Duplicate fields poison the raw field map: last-value-wins storage makes
    // missing-field validation ambiguous until the duplicates are fixed.
    true
}

pub(super) fn body_from_parsed(parsed: &ParsedTypedBlock) -> Option<Body> {
    Body::try_new(parsed.body_inlines.clone())
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Relations {
    depends_on: Vec<RelationTarget>,
    supersedes: Vec<RelationTarget>,
    related_to: Vec<RelationTarget>,
}

impl Relations {
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    pub(crate) fn targets(&self, relation: GraphRelationKind) -> &[RelationTarget] {
        match relation {
            GraphRelationKind::DependsOn => &self.depends_on,
            GraphRelationKind::Supersedes => &self.supersedes,
            GraphRelationKind::RelatedTo => &self.related_to,
        }
    }

    fn set_targets(&mut self, relation: GraphRelationKind, targets: Vec<RelationTarget>) {
        match relation {
            GraphRelationKind::DependsOn => self.depends_on = targets,
            GraphRelationKind::Supersedes => self.supersedes = targets,
            GraphRelationKind::RelatedTo => self.related_to = targets,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.depends_on.is_empty() && self.supersedes.is_empty() && self.related_to.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RelationTarget {
    id: ObjectId,
    span: SourceSpan,
}

impl RelationTarget {
    pub(crate) fn new(id: ObjectId, span: SourceSpan) -> Self {
        Self { id, span }
    }

    pub(crate) fn id(&self) -> &ObjectId {
        &self.id
    }

    pub(crate) fn span(&self) -> &SourceSpan {
        &self.span
    }
}

pub(super) fn extract_relations(
    parsed: &mut ParsedTypedBlock,
    diagnostics: &mut Vec<Diagnostic>,
) -> Relations {
    let mut relations = Relations::empty();

    for relation in GraphRelationKind::ALL {
        let key = relation.as_str();
        let Some(value) = parsed.raw_fields.remove(key) else {
            continue;
        };
        let value_span = parsed
            .raw_field_spans
            .get(key)
            .cloned()
            .unwrap_or_else(|| parsed.span.clone());
        let targets = parse_relation_targets(parsed, key, &value, &value_span, diagnostics);
        relations.set_targets(relation, targets);
    }

    relations
}

fn parse_relation_targets(
    parsed: &ParsedTypedBlock,
    key: &str,
    value: &str,
    value_span: &SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<RelationTarget> {
    if value.is_empty() {
        return Vec::new();
    }

    let Some((content_start, content_end)) =
        relation_content_range(parsed, key, value, value_span, diagnostics)
    else {
        return Vec::new();
    };

    let mut targets = Vec::new();
    let mut seen = BTreeSet::new();
    let mut segment_start = content_start;
    for (relative_comma_index, _) in value[content_start..content_end].match_indices(',') {
        let comma_index = content_start + relative_comma_index;
        push_relation_segment(
            parsed,
            key,
            value,
            segment_start,
            comma_index,
            value_span,
            &mut seen,
            &mut targets,
            diagnostics,
            false,
        );
        segment_start = comma_index + 1;
    }
    push_relation_segment(
        parsed,
        key,
        value,
        segment_start,
        content_end,
        value_span,
        &mut seen,
        &mut targets,
        diagnostics,
        true,
    );

    targets
}

fn relation_content_range(
    parsed: &ParsedTypedBlock,
    key: &str,
    value: &str,
    value_span: &SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(usize, usize)> {
    let Some((trimmed, trimmed_start, trimmed_end)) = trim_segment(value) else {
        return Some((0, 0));
    };

    match (trimmed.strip_prefix('['), trimmed.strip_suffix(']')) {
        (Some(_), Some(_)) => Some((trimmed_start + 1, trimmed_end - 1)),
        (Some(_), None) | (None, Some(_)) => {
            diagnostics.push(
                Diagnostic::error(
                    DiagnosticCode::IdInvalid,
                    format!("malformed relation array in `{key}` for `{}`", parsed.id_text),
                )
                .with_span(relation_segment_span(
                    value_span,
                    value,
                    trimmed_start,
                    trimmed_end,
                ))
                .with_object_id(&parsed.id_text)
                .with_help("Relation arrays must use `[object.id, other.id]`; each target must also be a valid Object ID."),
            );
            None
        }
        (None, None) => Some((0, value.len())),
    }
}

#[allow(clippy::too_many_arguments)]
fn push_relation_segment(
    parsed: &ParsedTypedBlock,
    key: &str,
    value: &str,
    start: usize,
    end: usize,
    value_span: &SourceSpan,
    seen: &mut BTreeSet<ObjectId>,
    targets: &mut Vec<RelationTarget>,
    diagnostics: &mut Vec<Diagnostic>,
    is_last: bool,
) {
    let raw = &value[start..end];
    let Some((trimmed, trimmed_start, trimmed_end)) = trim_segment(raw) else {
        if is_last {
            return;
        }
        diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::IdInvalid,
                format!("empty relation segment in `{key}` for `{}`", parsed.id_text),
            )
            .with_span(relation_segment_span(value_span, value, start, end))
            .with_object_id(&parsed.id_text)
            .with_help(OBJECT_ID_GRAMMAR_HELP),
        );
        return;
    };

    let span = relation_segment_span(
        value_span,
        value,
        start + trimmed_start,
        start + trimmed_end,
    );
    match ObjectId::new(trimmed) {
        Ok(id) => {
            if seen.insert(id.clone()) {
                targets.push(RelationTarget::new(id, span));
            }
        }
        Err(error) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::IdInvalid,
                format!(
                    "invalid relation id `{trimmed}` in `{key}` for `{}`: {error}",
                    parsed.id_text
                ),
            )
            .with_span(span)
            .with_object_id(trimmed)
            .with_help(OBJECT_ID_GRAMMAR_HELP),
        ),
    }
}

fn trim_segment(value: &str) -> Option<(&str, usize, usize)> {
    let start = value
        .char_indices()
        .find(|(_, character)| !character.is_ascii_whitespace())
        .map(|(index, _)| index)?;
    let end = value
        .char_indices()
        .rev()
        .find(|(_, character)| !character.is_ascii_whitespace())
        .map(|(index, character)| index + character.len_utf8())
        .expect("start proves a non-whitespace character exists");
    Some((&value[start..end], start, end))
}

/// Parse the opt-in `impacts:` field into a sorted, deduplicated, non-empty
/// list of repo-relative paths. Emits one [`DiagnosticCode::SchemaImpactsInvalidPath`]
/// per bad entry and [`DiagnosticCode::SchemaImpactsEmpty`] when the field is
/// authored but holds no path content (e.g. `impacts:` or `impacts: []`).
/// Returns `None` when the field is absent, structurally empty, or every entry
/// was invalid.
pub(super) fn extract_impacts(
    parsed: &mut ParsedTypedBlock,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<NonEmpty<RelPath>> {
    let value = parsed.raw_fields.remove(IMPACTS_FIELD)?;
    let value_span = parsed
        .raw_field_spans
        .get(IMPACTS_FIELD)
        .cloned()
        .unwrap_or_else(|| parsed.span.clone());

    let Some((content_start, content_end)) =
        relation_content_range(parsed, IMPACTS_FIELD, &value, &value_span, diagnostics)
    else {
        // Malformed `[...]` already reported by relation_content_range.
        return None;
    };

    let content = &value[content_start..content_end];
    if content
        .trim_matches(|character: char| character.is_ascii_whitespace())
        .is_empty()
    {
        diagnostics.push(empty_impacts_diagnostic(parsed, &value_span));
        return None;
    }

    let mut paths: std::collections::BTreeSet<RelPath> = std::collections::BTreeSet::new();
    let mut segment_start = content_start;
    for (relative_comma_index, _) in content.match_indices(',') {
        let comma_index = content_start + relative_comma_index;
        push_impact_segment(
            parsed,
            &value,
            segment_start,
            comma_index,
            &value_span,
            &mut paths,
            diagnostics,
            false,
        );
        segment_start = comma_index + 1;
    }
    push_impact_segment(
        parsed,
        &value,
        segment_start,
        content_end,
        &value_span,
        &mut paths,
        diagnostics,
        true,
    );

    NonEmpty::from_vec(paths.into_iter().collect())
}

#[allow(clippy::too_many_arguments)]
fn push_impact_segment(
    parsed: &ParsedTypedBlock,
    value: &str,
    start: usize,
    end: usize,
    value_span: &SourceSpan,
    paths: &mut std::collections::BTreeSet<RelPath>,
    diagnostics: &mut Vec<Diagnostic>,
    is_last: bool,
) {
    let raw = &value[start..end];
    let Some((trimmed, trimmed_start, trimmed_end)) = trim_segment(raw) else {
        if is_last {
            // Trailing comma is tolerated, matching relation parsing.
            return;
        }
        diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaImpactsInvalidPath,
                format!(
                    "empty `impacts` segment in `{}`; remove the extra comma or fill in the path",
                    parsed.id_text
                ),
            )
            .with_span(relation_segment_span(value_span, value, start, end))
            .with_object_id(&parsed.id_text),
        );
        return;
    };

    let span = relation_segment_span(
        value_span,
        value,
        start + trimmed_start,
        start + trimmed_end,
    );
    match RelPath::try_new(trimmed) {
        Ok(path) => {
            paths.insert(path);
        }
        Err(error) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaImpactsInvalidPath,
                format!(
                    "invalid `impacts` path `{trimmed}` for `{}`: {error}",
                    parsed.id_text
                ),
            )
            .with_span(span)
            .with_object_id(&parsed.id_text),
        ),
    }
}

/// Parse a comma-separated or scalar list field into a sorted, deduplicated
/// `Vec<T>` using the provided constructor closure. Accepts both scalar
/// (`field: value`) and bracket-list (`field: [a, b]`) syntax. Returns `None`
/// when the field is absent or yields no valid entries (the calling aggregate
/// is responsible for emitting the missing-field diagnostic).
///
/// This generalises the `extract_approved_by` logic so it can be reused for
/// `allowed_actions` and `forbidden_actions` on `agent_instruction`.
pub(super) fn extract_action_list<T, F>(
    parsed: &mut ParsedTypedBlock,
    field_name: &str,
    ctor: F,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Vec<T>>
where
    T: Ord,
    F: Fn(&str) -> Option<T>,
{
    let value = parsed.raw_fields.remove(field_name)?;
    let value_span = parsed
        .raw_field_spans
        .get(field_name)
        .cloned()
        .unwrap_or_else(|| parsed.span.clone());

    let (content_start, content_end) =
        relation_content_range(parsed, field_name, &value, &value_span, diagnostics)?;

    let content = &value[content_start..content_end];
    if content
        .trim_matches(|c: char| c.is_ascii_whitespace())
        .is_empty()
    {
        return None;
    }

    let mut items: BTreeSet<String> = BTreeSet::new();
    let mut segment_start = content_start;
    for (relative_comma_index, _) in content.match_indices(',') {
        let comma_index = content_start + relative_comma_index;
        push_action_list_segment(
            parsed,
            field_name,
            &value,
            segment_start,
            comma_index,
            &value_span,
            &mut items,
            diagnostics,
            false,
        );
        segment_start = comma_index + 1;
    }
    push_action_list_segment(
        parsed,
        field_name,
        &value,
        segment_start,
        content_end,
        &value_span,
        &mut items,
        diagnostics,
        true,
    );

    let result: Vec<T> = items.into_iter().filter_map(|s| ctor(&s)).collect();
    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

#[allow(clippy::too_many_arguments)]
fn push_action_list_segment(
    parsed: &ParsedTypedBlock,
    field_name: &str,
    value: &str,
    start: usize,
    end: usize,
    value_span: &SourceSpan,
    items: &mut BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    is_last: bool,
) {
    let raw = &value[start..end];
    let Some((trimmed, _trimmed_start, _trimmed_end)) = trim_segment(raw) else {
        if is_last {
            return;
        }
        diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaMissingField,
                format!(
                    "empty `{field_name}` segment in `{}`; remove the extra comma or fill in the action",
                    parsed.id_text
                ),
            )
            .with_span(relation_segment_span(value_span, value, start, end))
            .with_object_id(&parsed.id_text),
        );
        return;
    };

    items.insert(trimmed.to_string());
}

/// Parse the `approved_by:` field into a sorted, deduplicated, non-empty list
/// of [`ApprovedBy`] values. Accepts both scalar (`approved_by: name`) and
/// bracket-list (`approved_by: [a, b]`) syntax. Returns `None` when the field
/// is absent or yields no valid entries after per-segment validation (the
/// calling aggregate emits [`DiagnosticCode::SchemaPolicyMissingApprovedBy`]
/// in that case — this helper does NOT emit the missing-field diagnostic).
pub(super) fn extract_approved_by(
    parsed: &mut ParsedTypedBlock,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<NonEmpty<ApprovedBy>> {
    let value = parsed.raw_fields.remove(APPROVED_BY_FIELD)?;
    let value_span = parsed
        .raw_field_spans
        .get(APPROVED_BY_FIELD)
        .cloned()
        .unwrap_or_else(|| parsed.span.clone());

    let (content_start, content_end) =
        relation_content_range(parsed, APPROVED_BY_FIELD, &value, &value_span, diagnostics)?;

    let content = &value[content_start..content_end];
    if content
        .trim_matches(|c: char| c.is_ascii_whitespace())
        .is_empty()
    {
        return None;
    }

    let mut approvers: BTreeSet<String> = BTreeSet::new();
    let mut segment_start = content_start;
    for (relative_comma_index, _) in content.match_indices(',') {
        let comma_index = content_start + relative_comma_index;
        push_approved_by_segment(
            parsed,
            &value,
            segment_start,
            comma_index,
            &value_span,
            &mut approvers,
            diagnostics,
            false,
        );
        segment_start = comma_index + 1;
    }
    push_approved_by_segment(
        parsed,
        &value,
        segment_start,
        content_end,
        &value_span,
        &mut approvers,
        diagnostics,
        true,
    );

    NonEmpty::from_vec(
        approvers
            .into_iter()
            .filter_map(|s| ApprovedBy::try_new(&s))
            .collect(),
    )
}

#[allow(clippy::too_many_arguments)]
fn push_approved_by_segment(
    parsed: &ParsedTypedBlock,
    value: &str,
    start: usize,
    end: usize,
    value_span: &SourceSpan,
    approvers: &mut BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    is_last: bool,
) {
    let raw = &value[start..end];
    let Some((trimmed, _trimmed_start, _trimmed_end)) = trim_segment(raw) else {
        if is_last {
            return;
        }
        diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaMissingField,
                format!(
                    "empty `approved_by` segment in `{}`; remove the extra comma or fill in the approver",
                    parsed.id_text
                ),
            )
            .with_span(relation_segment_span(value_span, value, start, end))
            .with_object_id(&parsed.id_text),
        );
        return;
    };

    approvers.insert(trimmed.to_string());
}

fn empty_impacts_diagnostic(parsed: &ParsedTypedBlock, value_span: &SourceSpan) -> Diagnostic {
    Diagnostic::error(
        DiagnosticCode::SchemaImpactsEmpty,
        format!(
            "`impacts:` on `{}` is empty; omit the field instead of leaving it blank",
            parsed.id_text
        ),
    )
    .with_span(value_span.clone())
    .with_object_id(&parsed.id_text)
}

fn relation_segment_span(
    value_span: &SourceSpan,
    value: &str,
    start: usize,
    end: usize,
) -> SourceSpan {
    SourceSpan {
        file: value_span.file.clone(),
        start: SourcePosition {
            line: value_span.start.line,
            column: value_span.start.column + value[..start].chars().count() as u32,
            offset: value_span.start.offset + start as u32,
        },
        end: SourcePosition {
            line: value_span.start.line,
            column: value_span.start.column + value[..end].chars().count() as u32,
            offset: value_span.start.offset + end as u32,
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BlockKind {
    Claim,
    Decision,
    Glossary,
    Warning,
    Constraint,
    Policy,
    Procedure,
    Example,
    AgentInstruction,
}

impl BlockKind {
    pub(crate) const ALL: &'static [Self] = &[
        Self::Claim,
        Self::Decision,
        Self::Glossary,
        Self::Warning,
        Self::Constraint,
        Self::Policy,
        Self::Procedure,
        Self::Example,
        Self::AgentInstruction,
    ];

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Claim => "claim",
            Self::Decision => "decision",
            Self::Glossary => "glossary",
            Self::Warning => "warning",
            Self::Constraint => "constraint",
            Self::Policy => "policy",
            Self::Procedure => "procedure",
            Self::Example => "example",
            Self::AgentInstruction => "agent_instruction",
        }
    }

    pub(crate) fn from_fence_word(word: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|kind| kind.as_str() == word)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum KnowledgeObject {
    Claim(Claim),
    Decision(Decision),
    Glossary(Glossary),
    Warning(Warning),
    Constraint(Constraint),
    Policy(Policy),
    Procedure(Procedure),
    Example(Example),
    AgentInstruction(AgentInstruction),
}

impl KnowledgeObject {
    pub(crate) fn kind(&self) -> BlockKind {
        match self {
            Self::Claim(_) => BlockKind::Claim,
            Self::Decision(_) => BlockKind::Decision,
            Self::Glossary(_) => BlockKind::Glossary,
            Self::Warning(_) => BlockKind::Warning,
            Self::Constraint(_) => BlockKind::Constraint,
            Self::Policy(_) => BlockKind::Policy,
            Self::Procedure(_) => BlockKind::Procedure,
            Self::Example(_) => BlockKind::Example,
            Self::AgentInstruction(_) => BlockKind::AgentInstruction,
        }
    }

    pub(crate) fn id(&self) -> &ObjectId {
        match self {
            Self::Claim(claim) => claim.id(),
            Self::Decision(decision) => decision.id(),
            Self::Glossary(glossary) => glossary.id(),
            Self::Warning(warning) => warning.id(),
            Self::Constraint(constraint) => constraint.id(),
            Self::Policy(policy) => policy.id(),
            Self::Procedure(procedure) => procedure.id(),
            Self::Example(example) => example.id(),
            Self::AgentInstruction(ai) => ai.id(),
        }
    }

    pub(crate) fn span(&self) -> &SourceSpan {
        match self {
            Self::Claim(claim) => claim.span(),
            Self::Decision(decision) => decision.span(),
            Self::Glossary(glossary) => glossary.span(),
            Self::Warning(warning) => warning.span(),
            Self::Constraint(constraint) => constraint.span(),
            Self::Policy(policy) => policy.span(),
            Self::Procedure(procedure) => procedure.span(),
            Self::Example(example) => example.span(),
            Self::AgentInstruction(ai) => ai.span(),
        }
    }

    pub(crate) fn body(&self) -> &Body {
        match self {
            Self::Claim(claim) => claim.body(),
            Self::Decision(decision) => decision.body(),
            Self::Glossary(glossary) => glossary.body(),
            Self::Warning(warning) => warning.body(),
            Self::Constraint(constraint) => constraint.body(),
            Self::Policy(policy) => policy.body(),
            Self::Procedure(procedure) => procedure.body(),
            Self::Example(example) => example.body(),
            Self::AgentInstruction(ai) => ai.body(),
        }
    }

    pub(crate) fn body_mut(&mut self) -> &mut Body {
        match self {
            Self::Claim(claim) => claim.body_mut(),
            Self::Decision(decision) => decision.body_mut(),
            Self::Glossary(glossary) => glossary.body_mut(),
            Self::Warning(warning) => warning.body_mut(),
            Self::Constraint(constraint) => constraint.body_mut(),
            Self::Policy(policy) => policy.body_mut(),
            Self::Procedure(procedure) => procedure.body_mut(),
            Self::Example(example) => example.body_mut(),
            Self::AgentInstruction(ai) => ai.body_mut(),
        }
    }

    pub(crate) fn relations(&self) -> &Relations {
        match self {
            Self::Claim(claim) => claim.relations(),
            Self::Decision(decision) => decision.relations(),
            Self::Glossary(glossary) => glossary.relations(),
            Self::Warning(warning) => warning.relations(),
            Self::Constraint(constraint) => constraint.relations(),
            Self::Policy(policy) => policy.relations(),
            Self::Procedure(procedure) => procedure.relations(),
            Self::Example(example) => example.relations(),
            Self::AgentInstruction(ai) => ai.relations(),
        }
    }

    /// V3.3 opt-in `impacts:` list. Empty slice for kinds that do not carry
    /// this field (`glossary`, `warning`, `agent_instruction`) or for objects
    /// without it.
    pub(crate) fn impacts(&self) -> &[RelPath] {
        match self {
            Self::Claim(claim) => claim.impacts().unwrap_or(&[]),
            Self::Decision(decision) => decision.impacts().unwrap_or(&[]),
            Self::Constraint(constraint) => constraint.impacts().unwrap_or(&[]),
            Self::Policy(policy) => policy.impacts().unwrap_or(&[]),
            Self::Procedure(procedure) => procedure.impacts().unwrap_or(&[]),
            Self::Example(example) => example.impacts().unwrap_or(&[]),
            Self::Glossary(_) | Self::Warning(_) | Self::AgentInstruction(_) => &[],
        }
    }

    pub(crate) fn fields(&self) -> &crate::domain::values::OptionalFields {
        match self {
            Self::Claim(claim) => claim.fields(),
            Self::Decision(decision) => decision.fields(),
            Self::Glossary(glossary) => glossary.fields(),
            Self::Warning(warning) => warning.fields(),
            Self::Constraint(constraint) => constraint.fields(),
            Self::Policy(policy) => policy.fields(),
            Self::Procedure(procedure) => procedure.fields(),
            Self::Example(example) => example.fields(),
            Self::AgentInstruction(ai) => ai.fields(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use super::*;
    use crate::domain::diagnostic::{SourcePosition, SourceSpan};
    use crate::domain::inline::InlineSegment;
    use crate::domain::knowledge_object::agent_instruction::AgentInstruction;
    use crate::domain::knowledge_object::claim::Claim;
    use crate::domain::knowledge_object::constraint::Constraint;
    use crate::domain::knowledge_object::decision::{AcceptedVerdict, DecidedBy, Decision};
    use crate::domain::knowledge_object::example::Example;
    use crate::domain::knowledge_object::glossary::Glossary;
    use crate::domain::knowledge_object::policy::Policy;
    use crate::domain::knowledge_object::procedure::Procedure;
    use crate::domain::knowledge_object::warning::Warning;

    fn span(file: &str, line: u32, column: u32) -> SourceSpan {
        SourceSpan {
            file: PathBuf::from(file),
            start: SourcePosition {
                line,
                column,
                offset: 0,
            },
            end: SourcePosition {
                line,
                column: column + 20,
                offset: 20,
            },
        }
    }

    fn claim_object() -> KnowledgeObject {
        KnowledgeObject::Claim(
            Claim::try_new(
                "billing.credits",
                Some("plain"),
                "Claim body.",
                BTreeMap::from([("audience".to_string(), "support".to_string())]),
                None,
                span("claim.adoc", 3, 1),
            )
            .expect("valid claim"),
        )
    }

    fn decision_object() -> KnowledgeObject {
        KnowledgeObject::Decision(
            Decision::try_new(
                "billing.policy",
                Some("accepted"),
                "Decision body.",
                BTreeMap::from([("audience".to_string(), "ops".to_string())]),
                Some(AcceptedVerdict::new(
                    DecidedBy::try_new("architecture").expect("decided_by"),
                )),
                span("decision.adoc", 5, 1),
            )
            .expect("valid decision"),
        )
    }

    fn glossary_object() -> KnowledgeObject {
        KnowledgeObject::Glossary(
            Glossary::try_new(
                "billing.ledger",
                "Glossary body.",
                BTreeMap::from([("owner".to_string(), "team-billing".to_string())]),
                span("glossary.adoc", 7, 1),
            )
            .expect("valid glossary"),
        )
    }

    fn warning_object() -> KnowledgeObject {
        KnowledgeObject::Warning(
            Warning::try_new(
                "auth.session",
                Some("high"),
                "Warning body.",
                BTreeMap::from([("owner".to_string(), "platform".to_string())]),
                span("warning.adoc", 11, 1),
            )
            .expect("valid warning"),
        )
    }

    fn constraint_object() -> KnowledgeObject {
        KnowledgeObject::Constraint(
            Constraint::try_new(
                "auth.session.no-local-storage",
                Some("critical"),
                "Constraint body.",
                BTreeMap::from([("owner".to_string(), "platform-security".to_string())]),
                span("constraint.adoc", 13, 1),
            )
            .expect("valid constraint"),
        )
    }

    fn policy_object() -> KnowledgeObject {
        KnowledgeObject::Policy(
            Policy::try_new(
                "security.data-retention",
                "active",
                "security-lead",
                vec!["security-lead"],
                "2026-04-01",
                None,
                "Customer data is retained for no more than 365 days.",
                BTreeMap::from([("audience".to_string(), "all".to_string())]),
                span("policy.adoc", 19, 1),
            )
            .expect("valid policy"),
        )
    }

    fn procedure_object() -> KnowledgeObject {
        KnowledgeObject::Procedure(
            Procedure::try_new(
                "auth.key.rotate",
                Some("draft"),
                "1. Open the console.",
                BTreeMap::from([("owner".to_string(), "platform".to_string())]),
                None,
                span("procedure.adoc", 15, 1),
            )
            .expect("valid procedure"),
        )
    }

    fn example_object() -> KnowledgeObject {
        KnowledgeObject::Example(
            Example::try_new(
                "auth.credits.example",
                Some("draft"),
                Some("ts"),
                None,
                "const x = 1 + 1;",
                None,
                None,
                BTreeMap::from([("owner".to_string(), "platform".to_string())]),
                span("example.adoc", 17, 1),
            )
            .expect("valid example"),
        )
    }

    fn agent_instruction_object() -> KnowledgeObject {
        KnowledgeObject::AgentInstruction(
            AgentInstruction::try_new(
                "auth.docs-answering-policy",
                "docs/auth/*",
                "team",
                vec!["summarize", "cite"],
                vec!["execute_shell", "access_secrets"],
                "Prefer verified claims over draft notes.",
                BTreeMap::from([("owner".to_string(), "ai-platform".to_string())]),
                span("agent_instruction.adoc", 21, 1),
            )
            .expect("valid agent_instruction"),
        )
    }

    #[test]
    fn block_kind_labels_match_source_fence_words() {
        assert_eq!(BlockKind::Claim.as_str(), "claim");
        assert_eq!(BlockKind::Decision.as_str(), "decision");
        assert_eq!(BlockKind::Glossary.as_str(), "glossary");
        assert_eq!(BlockKind::Warning.as_str(), "warning");
        assert_eq!(BlockKind::Constraint.as_str(), "constraint");
        assert_eq!(BlockKind::Policy.as_str(), "policy");
        assert_eq!(BlockKind::Procedure.as_str(), "procedure");
        assert_eq!(BlockKind::Example.as_str(), "example");
        assert_eq!(BlockKind::AgentInstruction.as_str(), "agent_instruction");
    }

    #[test]
    fn block_kind_resolves_supported_fence_words_only() {
        assert_eq!(BlockKind::from_fence_word("claim"), Some(BlockKind::Claim));
        assert_eq!(
            BlockKind::from_fence_word("decision"),
            Some(BlockKind::Decision)
        );
        assert_eq!(
            BlockKind::from_fence_word("glossary"),
            Some(BlockKind::Glossary)
        );
        assert_eq!(
            BlockKind::from_fence_word("warning"),
            Some(BlockKind::Warning)
        );
        assert_eq!(
            BlockKind::from_fence_word("constraint"),
            Some(BlockKind::Constraint)
        );
        assert_eq!(
            BlockKind::from_fence_word("policy"),
            Some(BlockKind::Policy)
        );
        assert_eq!(
            BlockKind::from_fence_word("procedure"),
            Some(BlockKind::Procedure)
        );
        assert_eq!(
            BlockKind::from_fence_word("example"),
            Some(BlockKind::Example)
        );
        assert_eq!(
            BlockKind::from_fence_word("agent_instruction"),
            Some(BlockKind::AgentInstruction)
        );
        assert_eq!(BlockKind::from_fence_word("fact"), None);
        assert_eq!(BlockKind::from_fence_word("Claim"), None);
    }

    #[test]
    fn block_kind_all_lists_every_supported_kind() {
        assert_eq!(
            BlockKind::ALL,
            &[
                BlockKind::Claim,
                BlockKind::Decision,
                BlockKind::Glossary,
                BlockKind::Warning,
                BlockKind::Constraint,
                BlockKind::Policy,
                BlockKind::Procedure,
                BlockKind::Example,
                BlockKind::AgentInstruction,
            ]
        );
    }

    #[test]
    fn graph_relation_kind_all_lists_every_supported_relation_field_in_source_order() {
        use crate::domain::graph::GraphRelationKind;

        assert_eq!(
            GraphRelationKind::ALL,
            [
                GraphRelationKind::DependsOn,
                GraphRelationKind::Supersedes,
                GraphRelationKind::RelatedTo,
            ]
        );
        assert_eq!(GraphRelationKind::DependsOn.as_str(), "depends_on");
        assert_eq!(GraphRelationKind::Supersedes.as_str(), "supersedes");
        assert_eq!(GraphRelationKind::RelatedTo.as_str(), "related_to");
    }

    #[test]
    fn knowledge_object_common_accessors_report_aggregate_basics_for_each_variant() {
        let cases = [
            (
                claim_object(),
                BlockKind::Claim,
                "billing.credits",
                "Claim body.",
                "claim.adoc",
                3,
                "audience",
            ),
            (
                decision_object(),
                BlockKind::Decision,
                "billing.policy",
                "Decision body.",
                "decision.adoc",
                5,
                "audience",
            ),
            (
                glossary_object(),
                BlockKind::Glossary,
                "billing.ledger",
                "Glossary body.",
                "glossary.adoc",
                7,
                "owner",
            ),
            (
                warning_object(),
                BlockKind::Warning,
                "auth.session",
                "Warning body.",
                "warning.adoc",
                11,
                "owner",
            ),
            (
                constraint_object(),
                BlockKind::Constraint,
                "auth.session.no-local-storage",
                "Constraint body.",
                "constraint.adoc",
                13,
                "owner",
            ),
            (
                policy_object(),
                BlockKind::Policy,
                "security.data-retention",
                "Customer data is retained for no more than 365 days.",
                "policy.adoc",
                19,
                "audience",
            ),
            (
                procedure_object(),
                BlockKind::Procedure,
                "auth.key.rotate",
                "1. Open the console.",
                "procedure.adoc",
                15,
                "owner",
            ),
            (
                example_object(),
                BlockKind::Example,
                "auth.credits.example",
                "const x = 1 + 1;",
                "example.adoc",
                17,
                "owner",
            ),
            (
                agent_instruction_object(),
                BlockKind::AgentInstruction,
                "auth.docs-answering-policy",
                "Prefer verified claims over draft notes.",
                "agent_instruction.adoc",
                21,
                "owner",
            ),
        ];

        for (mut object, kind, id, body, file, line, metadata_key) in cases {
            assert_eq!(object.kind(), kind);
            assert_eq!(object.id().as_str(), id);
            assert_eq!(object.body().to_source(), body);
            assert_eq!(object.span().file, PathBuf::from(file));
            assert_eq!(object.span().start.line, line);
            assert!(object.relations().is_empty());
            assert_eq!(
                object
                    .fields()
                    .iter()
                    .next()
                    .map(|(key, _value)| key.as_str()),
                Some(metadata_key)
            );

            object
                .body_mut()
                .inlines_mut()
                .push(InlineSegment::Text(" Extended.".to_string()));

            assert_eq!(object.body().to_source(), format!("{body} Extended."));
        }
    }
}
