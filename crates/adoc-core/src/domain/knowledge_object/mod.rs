//! Aggregate family — populated by Slice 1.

use std::collections::BTreeSet;

use crate::domain::ast::ParsedTypedBlock;
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, SourceSpan};
use crate::domain::graph::GraphRelationKind;
use crate::domain::identity::{OBJECT_ID_GRAMMAR_HELP, ObjectId};
use crate::domain::value_objects::approved_by::ApprovedBy;
use crate::domain::value_objects::evidence::Evidence;
use crate::domain::value_objects::rel_path::RelPath;
use crate::domain::values::{Body, NonEmpty};

pub(super) const IMPACTS_FIELD: &str = "impacts";
pub(crate) const APPROVED_BY_FIELD: &str = "approved_by";

pub(crate) mod agent_instruction;
pub(crate) mod api;
pub(crate) mod claim;
pub(crate) mod constraint;
pub(crate) mod contradiction;
pub(crate) mod decision;
pub(crate) mod draft;
pub(crate) mod example;
mod field_decoder;
pub(crate) mod glossary;
pub(crate) mod metadata;
pub(crate) mod observation;
pub(crate) mod policy;
pub(crate) mod procedure;
pub(crate) mod projection;
pub(crate) mod question;
pub(crate) mod source;
pub(crate) mod task;
pub(crate) mod warning;

use agent_instruction::AgentInstruction;
use api::Api;
use claim::Claim;
use constraint::Constraint;
use contradiction::Contradiction;
use decision::Decision;
use example::Example;
use glossary::Glossary;
use observation::Observation;
use policy::Policy;
use procedure::Procedure;
use question::Question;
use source::Source;
use task::Task;
use warning::Warning;

use field_decoder::{DecodedListField, DecodedListSegment};
pub(super) use field_decoder::{take_optional_scalar, take_required_scalar, take_scalar_text};

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

fn take_decoded_list(
    parsed: &mut ParsedTypedBlock,
    field_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<DecodedListField> {
    match field_decoder::take_list_field(parsed, field_name)? {
        Ok(field) => Some(field),
        Err(error) => {
            diagnostics.push(error.into_diagnostic(parsed, field_name));
            None
        }
    }
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
        let Some(field) = take_decoded_list(parsed, key, diagnostics) else {
            continue;
        };
        let targets = parse_relation_targets(parsed, key, field.segments, diagnostics);
        relations.set_targets(relation, targets);
    }

    relations
}

/// V5.8 TB2/TB3: the `evidence_ref:` field name, shared by `claim` and
/// `decision`. Each entry names a `source` Knowledge Object by ID; the
/// workspace validator checks the reference resolves.
pub(crate) const EVIDENCE_REF_FIELD: &str = "evidence_ref";

/// Parse the `evidence_ref:` field into a deduplicated list of
/// [`Evidence::ObjectRef`] entries. Accepts both scalar (`evidence_ref: id.one`)
/// and bracket-list (`evidence_ref: [id.one, id.two]`) syntax. Invalid IDs emit
/// [`DiagnosticCode::IdInvalid`] and are silently dropped from the result.
/// Returns an empty `Vec` when the field is absent or yields no valid entries.
///
/// This shared implementation is called by both `claim::build_from_parsed` and
/// `decision::build_from_parsed` — do NOT duplicate the logic.
pub(crate) fn parse_evidence_refs(
    parsed: &mut ParsedTypedBlock,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<Evidence> {
    let Some(field) = take_decoded_list(parsed, EVIDENCE_REF_FIELD, diagnostics) else {
        return Vec::new();
    };
    let mut seen = BTreeSet::new();
    field
        .segments
        .into_iter()
        .filter_map(|segment| match segment.value {
            None => {
                diagnostics.push(
                    Diagnostic::error(
                        DiagnosticCode::IdInvalid,
                        format!(
                            "empty `evidence_ref` segment in `{}`; remove the extra comma or fill in the id",
                            parsed.id_text
                        ),
                    )
                    .with_span(segment.span)
                    .with_object_id(&parsed.id_text)
                    .with_help(OBJECT_ID_GRAMMAR_HELP),
                );
                None
            }
            Some(value) => match ObjectId::new(&value) {
                Ok(id) if seen.insert(id.clone()) => Some(Evidence::object_ref(id)),
                Ok(_) => None,
                Err(error) => {
                    diagnostics.push(
                        Diagnostic::error(
                            DiagnosticCode::IdInvalid,
                            format!(
                                "invalid `evidence_ref` id `{value}` for `{}`: {error}",
                                parsed.id_text
                            ),
                        )
                        .with_span(segment.span)
                        .with_object_id(&value)
                        .with_help(OBJECT_ID_GRAMMAR_HELP),
                    );
                    None
                }
            },
        })
        .collect()
}

fn parse_relation_targets(
    parsed: &ParsedTypedBlock,
    key: &str,
    segments: Vec<DecodedListSegment>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<RelationTarget> {
    let mut targets = Vec::new();
    let mut seen = BTreeSet::new();
    for segment in segments {
        let Some(value) = segment.value else {
            diagnostics.push(
                Diagnostic::error(
                    DiagnosticCode::IdInvalid,
                    format!("empty relation segment in `{key}` for `{}`", parsed.id_text),
                )
                .with_span(segment.span)
                .with_object_id(&parsed.id_text)
                .with_help(OBJECT_ID_GRAMMAR_HELP),
            );
            continue;
        };
        match ObjectId::new(&value) {
            Ok(id) => {
                if seen.insert(id.clone()) {
                    targets.push(RelationTarget::new(id, segment.span));
                }
            }
            Err(error) => diagnostics.push(
                Diagnostic::error(
                    DiagnosticCode::IdInvalid,
                    format!(
                        "invalid relation id `{value}` in `{key}` for `{}`: {error}",
                        parsed.id_text
                    ),
                )
                .with_span(segment.span)
                .with_object_id(&value)
                .with_help(OBJECT_ID_GRAMMAR_HELP),
            ),
        }
    }
    targets
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
    let field = take_decoded_list(parsed, IMPACTS_FIELD, diagnostics)?;
    if field.segments.is_empty() {
        diagnostics.push(empty_impacts_diagnostic(parsed, &field.value_span));
        return None;
    }

    let mut paths = BTreeSet::new();
    for segment in field.segments {
        let Some(value) = segment.value else {
            diagnostics.push(
                Diagnostic::error(
                    DiagnosticCode::SchemaImpactsInvalidPath,
                    format!(
                        "empty `impacts` segment in `{}`; remove the extra comma or fill in the path",
                        parsed.id_text
                    ),
                )
                .with_span(segment.span)
                .with_object_id(&parsed.id_text),
            );
            continue;
        };
        match RelPath::try_new(&value) {
            Ok(path) => {
                paths.insert(path);
            }
            Err(error) => diagnostics.push(
                Diagnostic::error(
                    DiagnosticCode::SchemaImpactsInvalidPath,
                    format!(
                        "invalid `impacts` path `{value}` for `{}`: {error}",
                        parsed.id_text
                    ),
                )
                .with_span(segment.span)
                .with_object_id(&parsed.id_text),
            ),
        }
    }
    NonEmpty::from_vec(paths.into_iter().collect())
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
    let items = take_sorted_text_list(parsed, field_name, "action", diagnostics)?;
    let result: Vec<T> = items.into_iter().filter_map(|item| ctor(&item)).collect();
    if result.is_empty() {
        None
    } else {
        Some(result)
    }
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
    let approvers = take_sorted_text_list(parsed, APPROVED_BY_FIELD, "approver", diagnostics)?;
    NonEmpty::from_vec(
        approvers
            .into_iter()
            .filter_map(|approver| ApprovedBy::try_new(&approver))
            .collect(),
    )
}

fn take_sorted_text_list(
    parsed: &mut ParsedTypedBlock,
    field_name: &str,
    item_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Vec<String>> {
    let field = take_decoded_list(parsed, field_name, diagnostics)?;
    let mut items = BTreeSet::new();
    for segment in field.segments {
        if let Some(value) = segment.value {
            items.insert(value);
        } else {
            diagnostics.push(
                Diagnostic::error(
                    DiagnosticCode::SchemaMissingField,
                    format!(
                        "empty `{field_name}` segment in `{}`; remove the extra comma or fill in the {item_name}",
                        parsed.id_text
                    ),
                )
                .with_span(segment.span)
                .with_object_id(&parsed.id_text),
            );
        }
    }
    (!items.is_empty()).then(|| items.into_iter().collect())
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
    Contradiction,
    Source,
    Api,
    Observation,
    Question,
    Task,
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
        Self::Contradiction,
        Self::Source,
        Self::Api,
        Self::Observation,
        Self::Question,
        Self::Task,
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
            Self::Contradiction => "contradiction",
            Self::Source => "source",
            Self::Api => "api",
            Self::Observation => "observation",
            Self::Question => "question",
            Self::Task => "task",
        }
    }

    pub(crate) fn from_fence_word(word: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|kind| kind.as_str() == word)
    }

    /// Field populated by the patch wire's generic `changes.status` value.
    /// Kinds without a lifecycle/severity discriminant reject that wire field.
    pub(crate) const fn patch_discriminant_field(self) -> Option<&'static str> {
        match self {
            Self::Warning | Self::Constraint => Some("severity"),
            Self::Claim
            | Self::Decision
            | Self::Policy
            | Self::Procedure
            | Self::Example
            | Self::Contradiction
            | Self::Api
            | Self::Observation
            | Self::Question
            | Self::Task => Some("status"),
            Self::Glossary | Self::AgentInstruction | Self::Source => None,
        }
    }
}

/// Kind name strings for every supported typed block, in `BlockKind::ALL`
/// order. Public so the published-docs guard (ADR-0041) can assert doc kind
/// lists against the shipped vocabulary without widening `BlockKind` itself.
pub fn block_kind_names() -> Vec<&'static str> {
    BlockKind::ALL
        .iter()
        .copied()
        .map(BlockKind::as_str)
        .collect()
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
    Contradiction(Contradiction),
    Source(Source),
    Api(Api),
    Observation(Observation),
    Question(Question),
    Task(Task),
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
            Self::Contradiction(_) => BlockKind::Contradiction,
            Self::Source(_) => BlockKind::Source,
            Self::Api(_) => BlockKind::Api,
            Self::Observation(_) => BlockKind::Observation,
            Self::Question(_) => BlockKind::Question,
            Self::Task(_) => BlockKind::Task,
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
            Self::Contradiction(contradiction) => contradiction.id(),
            Self::Source(source) => source.id(),
            Self::Api(api) => api.id(),
            Self::Observation(observation) => observation.id(),
            Self::Question(question) => question.id(),
            Self::Task(task) => task.id(),
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
            Self::Contradiction(contradiction) => contradiction.span(),
            Self::Source(source) => source.span(),
            Self::Api(api) => api.span(),
            Self::Observation(observation) => observation.span(),
            Self::Question(question) => question.span(),
            Self::Task(task) => task.span(),
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
            Self::Contradiction(contradiction) => contradiction.body(),
            Self::Source(source) => source.body(),
            Self::Api(api) => api.body(),
            Self::Observation(observation) => observation.body(),
            Self::Question(question) => question.body(),
            Self::Task(task) => task.body(),
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
            Self::Contradiction(contradiction) => contradiction.body_mut(),
            Self::Source(source) => source.body_mut(),
            Self::Api(api) => api.body_mut(),
            Self::Observation(observation) => observation.body_mut(),
            Self::Question(question) => question.body_mut(),
            Self::Task(task) => task.body_mut(),
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
            Self::Contradiction(contradiction) => contradiction.relations(),
            Self::Source(source) => source.relations(),
            Self::Api(api) => api.relations(),
            Self::Observation(observation) => observation.relations(),
            Self::Question(question) => question.relations(),
            Self::Task(task) => task.relations(),
        }
    }

    /// V3.3 opt-in `impacts:` list. Empty slice for kinds that do not carry
    /// this field (`glossary`, `warning`, `agent_instruction`, `contradiction`,
    /// `observation`) or for objects without it.
    pub(crate) fn impacts(&self) -> &[RelPath] {
        match self {
            Self::Claim(claim) => claim.impacts().unwrap_or(&[]),
            Self::Api(api) => api.impacts().unwrap_or(&[]),
            Self::Decision(decision) => decision.impacts().unwrap_or(&[]),
            Self::Constraint(constraint) => constraint.impacts().unwrap_or(&[]),
            Self::Policy(policy) => policy.impacts().unwrap_or(&[]),
            Self::Procedure(procedure) => procedure.impacts().unwrap_or(&[]),
            Self::Example(example) => example.impacts().unwrap_or(&[]),
            Self::Glossary(_)
            | Self::Warning(_)
            | Self::AgentInstruction(_)
            | Self::Contradiction(_)
            | Self::Source(_)
            | Self::Observation(_) => &[],
            Self::Question(_) => &[],
            Self::Task(_) => &[],
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
            Self::Contradiction(contradiction) => contradiction.fields(),
            Self::Source(source) => source.fields(),
            Self::Api(api) => api.fields(),
            Self::Observation(observation) => observation.fields(),
            Self::Question(question) => question.fields(),
            Self::Task(task) => task.fields(),
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
    use crate::domain::knowledge_object::contradiction::Contradiction;
    use crate::domain::knowledge_object::decision::{AcceptedVerdict, DecidedBy, Decision};
    use crate::domain::knowledge_object::example::Example;
    use crate::domain::knowledge_object::glossary::Glossary;
    use crate::domain::knowledge_object::observation::Observation;
    use crate::domain::knowledge_object::policy::Policy;
    use crate::domain::knowledge_object::procedure::Procedure;
    use crate::domain::knowledge_object::source::Source;
    use crate::domain::knowledge_object::task::Task;
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

    fn parsed_block_with_fields(fields: &[(&str, &str)]) -> ParsedTypedBlock {
        let block_span = span("test.adoc", 1, 1);
        ParsedTypedBlock {
            kind_word: "claim".to_string(),
            kind_word_span: block_span.clone(),
            id_text: "billing.credits".to_string(),
            raw_fields: fields
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect(),
            raw_field_spans: BTreeMap::new(),
            duplicate_keys: Vec::new(),
            body_text: "x".to_string(),
            body_inlines: ParsedTypedBlock::test_body_inlines_from_text("x"),
            body_spans: Vec::new(),
            content_spans: Vec::new(),
            span: block_span.clone(),
            close_fence_span: block_span,
            body_separator_span: None,
        }
    }

    #[test]
    fn take_optional_scalar_treats_blank_as_absent_and_trims() {
        let mut parsed =
            parsed_block_with_fields(&[("status", "  "), ("lang", "  rust  "), ("keep", "x")]);

        let blank: Result<Option<String>, ()> =
            take_optional_scalar(&mut parsed, "status", |s| Ok(s.to_string()));
        assert_eq!(blank, Ok(None));

        let absent: Result<Option<String>, ()> =
            take_optional_scalar(&mut parsed, "missing", |s| Ok(s.to_string()));
        assert_eq!(absent, Ok(None));

        let trimmed: Result<Option<String>, ()> =
            take_optional_scalar(&mut parsed, "lang", |s| Ok(s.to_string()));
        assert_eq!(trimmed, Ok(Some("rust".to_string())));

        // The field is consumed either way; unrelated fields stay.
        assert!(!parsed.raw_fields.contains_key("status"));
        assert!(!parsed.raw_fields.contains_key("lang"));
        assert!(parsed.raw_fields.contains_key("keep"));
    }

    #[test]
    fn take_optional_scalar_propagates_ctor_error() {
        let mut parsed = parsed_block_with_fields(&[("status", "bogus")]);
        let result: Result<Option<String>, String> =
            take_optional_scalar(&mut parsed, "status", |s| Err(s.to_string()));
        assert_eq!(result, Err("bogus".to_string()));
    }

    #[test]
    fn take_required_scalar_maps_blank_and_absent_to_missing() {
        let mut parsed = parsed_block_with_fields(&[("status", "  ")]);
        let blank: Result<String, &str> =
            take_required_scalar(&mut parsed, "status", |s| Ok(s.to_string()), || "missing");
        assert_eq!(blank, Err("missing"));
        let absent: Result<String, &str> =
            take_required_scalar(&mut parsed, "status", |s| Ok(s.to_string()), || "missing");
        assert_eq!(absent, Err("missing"));
    }

    #[test]
    fn take_scalar_text_trims_and_treats_blank_as_absent() {
        let mut parsed = parsed_block_with_fields(&[("format", "  json  "), ("checks", " ")]);
        assert_eq!(
            take_scalar_text(&mut parsed, "format"),
            Some("json".to_string())
        );
        assert_eq!(take_scalar_text(&mut parsed, "checks"), None);
        assert_eq!(take_scalar_text(&mut parsed, "absent"), None);
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
                    Vec::new(),
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

    fn contradiction_object() -> KnowledgeObject {
        KnowledgeObject::Contradiction(
            Contradiction::try_new(
                "auth.conflict",
                "high",
                "unresolved",
                vec!["auth.a", "auth.b"],
                "Claim auth.a conflicts with auth.b.",
                BTreeMap::from([("owner".to_string(), "auth-team".to_string())]),
                span("contradiction.adoc", 23, 1),
            )
            .expect("valid contradiction"),
        )
    }

    fn task_object() -> KnowledgeObject {
        KnowledgeObject::Task(
            Task::try_new(
                "billing.update-support-runbook",
                "open",
                "support-ops",
                Some("2026-05-20"),
                "Update the support runbook.",
                BTreeMap::from([("audience".to_string(), "support".to_string())]),
                span("task.adoc", 27, 1),
            )
            .expect("valid task"),
        )
    }

    fn source_object() -> KnowledgeObject {
        KnowledgeObject::Source(
            Source::try_new(
                "billing.consume-use-case",
                "source_code",
                Some("src/features/credits/consume.ts"),
                None,
                "Source implementation for credit consumption.",
                BTreeMap::from([("owner".to_string(), "backend-platform".to_string())]),
                span("source.adoc", 25, 1),
            )
            .expect("valid source"),
        )
    }

    fn observation_object() -> KnowledgeObject {
        KnowledgeObject::Observation(
            Observation::try_new(
                "onboarding.credit-confusion",
                "observed",
                Some("37"),
                Some("2026-04-30"),
                "Users misunderstand credit usage.",
                BTreeMap::from([("owner".to_string(), "product-growth".to_string())]),
                span("observation.adoc", 27, 1),
            )
            .expect("valid observation"),
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
        assert_eq!(BlockKind::Contradiction.as_str(), "contradiction");
        assert_eq!(BlockKind::Source.as_str(), "source");
        assert_eq!(BlockKind::Api.as_str(), "api");
        assert_eq!(BlockKind::Observation.as_str(), "observation");
        assert_eq!(BlockKind::Question.as_str(), "question");
        assert_eq!(BlockKind::Task.as_str(), "task");
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
        assert_eq!(
            BlockKind::from_fence_word("contradiction"),
            Some(BlockKind::Contradiction)
        );
        assert_eq!(
            BlockKind::from_fence_word("source"),
            Some(BlockKind::Source)
        );
        assert_eq!(BlockKind::from_fence_word("api"), Some(BlockKind::Api));
        assert_eq!(
            BlockKind::from_fence_word("observation"),
            Some(BlockKind::Observation)
        );
        assert_eq!(BlockKind::from_fence_word("task"), Some(BlockKind::Task));
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
                BlockKind::Contradiction,
                BlockKind::Source,
                BlockKind::Api,
                BlockKind::Observation,
                BlockKind::Question,
                BlockKind::Task,
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
            (
                contradiction_object(),
                BlockKind::Contradiction,
                "auth.conflict",
                "Claim auth.a conflicts with auth.b.",
                "contradiction.adoc",
                23,
                "owner",
            ),
            (
                source_object(),
                BlockKind::Source,
                "billing.consume-use-case",
                "Source implementation for credit consumption.",
                "source.adoc",
                25,
                "owner",
            ),
            (
                observation_object(),
                BlockKind::Observation,
                "onboarding.credit-confusion",
                "Users misunderstand credit usage.",
                "observation.adoc",
                27,
                "owner",
            ),
            (
                task_object(),
                BlockKind::Task,
                "billing.update-support-runbook",
                "Update the support runbook.",
                "task.adoc",
                27,
                "audience",
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
