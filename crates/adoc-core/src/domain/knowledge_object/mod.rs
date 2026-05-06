//! Aggregate family — populated by Slice 1.

use std::collections::BTreeSet;

use crate::domain::ast::ParsedTypedBlock;
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, SourcePosition, SourceSpan};
use crate::domain::identity::{OBJECT_ID_GRAMMAR_HELP, ObjectId};
use crate::domain::inline::{InlineOrigin, InlineSegment, parse_inlines};
use crate::domain::values::Body;

pub(crate) mod claim;
pub(crate) mod decision;
pub(crate) mod glossary;
pub(crate) mod warning;

use claim::Claim;
use decision::Decision;
use glossary::Glossary;
use warning::Warning;

pub(crate) const DEPENDS_ON_FIELD: &str = "depends_on";
pub(crate) const SUPERSEDES_FIELD: &str = "supersedes";
pub(crate) const RELATED_TO_FIELD: &str = "related_to";

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
    if parsed.body_spans.is_empty() {
        return Body::from_plain_text(&parsed.body_text);
    }

    let mut inlines = Vec::new();
    for (index, line) in parsed.body_text.split('\n').enumerate() {
        if index > 0 {
            inlines.push(InlineSegment::Text("\n".to_string()));
        }
        let span = &parsed.body_spans[index];
        let (line_inlines, diagnostics) = parse_inlines(line, InlineOrigin::from_span(span));
        debug_assert!(
            diagnostics.is_empty(),
            "body inline parsing should not emit diagnostics in V0.5"
        );
        inlines.extend(line_inlines);
    }

    Body::try_new(inlines)
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

    pub(crate) fn depends_on(&self) -> &[RelationTarget] {
        &self.depends_on
    }

    pub(crate) fn supersedes(&self) -> &[RelationTarget] {
        &self.supersedes
    }

    pub(crate) fn related_to(&self) -> &[RelationTarget] {
        &self.related_to
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

    for key in [DEPENDS_ON_FIELD, SUPERSEDES_FIELD, RELATED_TO_FIELD] {
        let Some(value) = parsed.raw_fields.remove(key) else {
            continue;
        };
        let value_span = parsed
            .raw_field_spans
            .get(key)
            .cloned()
            .unwrap_or_else(|| parsed.span.clone());
        let targets = parse_relation_targets(parsed, key, &value, &value_span, diagnostics);
        match key {
            DEPENDS_ON_FIELD => relations.depends_on = targets,
            SUPERSEDES_FIELD => relations.supersedes = targets,
            RELATED_TO_FIELD => relations.related_to = targets,
            _ => unreachable!("relation key list is exhaustive"),
        }
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

    let mut targets = Vec::new();
    let mut seen = BTreeSet::new();
    let mut segment_start = 0;
    for (comma_index, _) in value.match_indices(',') {
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
        value.len(),
        value_span,
        &mut seen,
        &mut targets,
        diagnostics,
        true,
    );

    targets
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
