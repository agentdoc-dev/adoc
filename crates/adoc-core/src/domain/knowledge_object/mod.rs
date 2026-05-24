//! Aggregate family — populated by Slice 1.

use std::collections::BTreeSet;

use crate::domain::ast::ParsedTypedBlock;
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, SourcePosition, SourceSpan};
use crate::domain::graph::GraphRelationKind;
use crate::domain::identity::{OBJECT_ID_GRAMMAR_HELP, ObjectId};
use crate::domain::value_objects::rel_path::RelPath;
use crate::domain::values::{Body, NonEmpty};

pub(super) const IMPACTS_FIELD: &str = "impacts";

pub(crate) mod claim;
pub(crate) mod decision;
pub(crate) mod draft;
pub(crate) mod glossary;
pub(crate) mod projection;
pub(crate) mod warning;

use claim::Claim;
use decision::Decision;
use glossary::Glossary;
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
}

impl BlockKind {
    pub(crate) const ALL: &'static [Self] =
        &[Self::Claim, Self::Decision, Self::Glossary, Self::Warning];

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Claim => "claim",
            Self::Decision => "decision",
            Self::Glossary => "glossary",
            Self::Warning => "warning",
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
}

impl KnowledgeObject {
    pub(crate) fn kind(&self) -> BlockKind {
        match self {
            Self::Claim(_) => BlockKind::Claim,
            Self::Decision(_) => BlockKind::Decision,
            Self::Glossary(_) => BlockKind::Glossary,
            Self::Warning(_) => BlockKind::Warning,
        }
    }

    pub(crate) fn id(&self) -> &ObjectId {
        match self {
            Self::Claim(claim) => claim.id(),
            Self::Decision(decision) => decision.id(),
            Self::Glossary(glossary) => glossary.id(),
            Self::Warning(warning) => warning.id(),
        }
    }

    pub(crate) fn span(&self) -> &SourceSpan {
        match self {
            Self::Claim(claim) => claim.span(),
            Self::Decision(decision) => decision.span(),
            Self::Glossary(glossary) => glossary.span(),
            Self::Warning(warning) => warning.span(),
        }
    }

    pub(crate) fn body(&self) -> &Body {
        match self {
            Self::Claim(claim) => claim.body(),
            Self::Decision(decision) => decision.body(),
            Self::Glossary(glossary) => glossary.body(),
            Self::Warning(warning) => warning.body(),
        }
    }

    pub(crate) fn body_mut(&mut self) -> &mut Body {
        match self {
            Self::Claim(claim) => claim.body_mut(),
            Self::Decision(decision) => decision.body_mut(),
            Self::Glossary(glossary) => glossary.body_mut(),
            Self::Warning(warning) => warning.body_mut(),
        }
    }

    pub(crate) fn relations(&self) -> &Relations {
        match self {
            Self::Claim(claim) => claim.relations(),
            Self::Decision(decision) => decision.relations(),
            Self::Glossary(glossary) => glossary.relations(),
            Self::Warning(warning) => warning.relations(),
        }
    }

    /// V3.3 opt-in `impacts:` list. Empty slice for kinds that do not carry
    /// this field (`glossary`, `warning`) or for `claim`/`decision` instances
    /// without it.
    pub(crate) fn impacts(&self) -> &[RelPath] {
        match self {
            Self::Claim(claim) => claim.impacts().unwrap_or(&[]),
            Self::Decision(decision) => decision.impacts().unwrap_or(&[]),
            Self::Glossary(_) | Self::Warning(_) => &[],
        }
    }

    pub(crate) fn fields(&self) -> &crate::domain::values::OptionalFields {
        match self {
            Self::Claim(claim) => claim.fields(),
            Self::Decision(decision) => decision.fields(),
            Self::Glossary(glossary) => glossary.fields(),
            Self::Warning(warning) => warning.fields(),
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
    use crate::domain::knowledge_object::claim::Claim;
    use crate::domain::knowledge_object::decision::{AcceptedVerdict, DecidedBy, Decision};
    use crate::domain::knowledge_object::glossary::Glossary;
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

    #[test]
    fn block_kind_labels_match_source_fence_words() {
        assert_eq!(BlockKind::Claim.as_str(), "claim");
        assert_eq!(BlockKind::Decision.as_str(), "decision");
        assert_eq!(BlockKind::Glossary.as_str(), "glossary");
        assert_eq!(BlockKind::Warning.as_str(), "warning");
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
                BlockKind::Warning
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
