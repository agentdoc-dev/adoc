//! Aggregate family — populated by Slice 1.

use std::collections::BTreeSet;

use crate::domain::ast::ParsedTypedBlock;
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};

pub(crate) mod claim;
pub(crate) mod decision;
pub(crate) mod glossary;
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
