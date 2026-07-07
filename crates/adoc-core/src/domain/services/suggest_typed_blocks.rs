//! V8.1.3 suggested typed-block candidates (PRD §28.4, ADR-0043).
//!
//! Scans a migrated page's prose blocks and emits report records naming the
//! typed block a human could write — **never auto-typed**: suggestions live
//! only in the `adoc.migrate.report.v0` envelope and never touch the rendered
//! `.adoc` text (ADR-0023, evidence-first). Rules, not weights (the V1
//! parameter-free rule): a suggestion either matches a named rule or does not
//! exist; there are no confidence scores. Rules run first-match-wins per
//! top-level block in a fixed order, so each block yields at most one
//! suggestion and "must not" prose lands as `warning`, never `claim`.

use serde::Serialize;

use crate::domain::ast::{BlockAst, ListKind, PageAst, ParagraphAst};
use crate::domain::diagnostic::SourceSpan;
use crate::domain::inline::plain_text;
use crate::domain::knowledge_object::BlockKind;

/// One §28.4 typed-block candidate: a report record, never applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SuggestedTypedBlock {
    pub span: SourceSpan,
    /// A `BlockKind` wire name (`task`, `procedure`, `warning`, `glossary`,
    /// `claim`) — the block the human would type.
    pub suggested_kind: &'static str,
    pub matched_rule: &'static str,
    /// First line of the matched plain text, truncated to
    /// `EXCERPT_MAX_CHARS` (120) characters.
    pub excerpt: String,
}

const EXCERPT_MAX_CHARS: usize = 120;

/// A rule's matcher: the reported span plus the text the excerpt derives from.
type Matcher = fn(&BlockAst) -> Option<(SourceSpan, String)>;

struct SuggestionRule {
    name: &'static str,
    kind: BlockKind,
    matches: Matcher,
}

/// Fixed evaluation order — `warning_phrase` before `assertive_modal` is
/// load-bearing; `todo_line` before `numbered_step_list` so an ordered list
/// carrying a TODO item suggests the task, not the procedure.
const RULES: &[SuggestionRule] = &[
    SuggestionRule {
        name: "todo_line",
        kind: BlockKind::Task,
        matches: match_todo_line,
    },
    SuggestionRule {
        name: "numbered_step_list",
        kind: BlockKind::Procedure,
        matches: match_numbered_step_list,
    },
    SuggestionRule {
        name: "warning_phrase",
        kind: BlockKind::Warning,
        matches: match_warning_phrase,
    },
    SuggestionRule {
        name: "definitional_paragraph",
        kind: BlockKind::Glossary,
        matches: match_definitional_paragraph,
    },
    SuggestionRule {
        name: "assertive_modal",
        kind: BlockKind::Claim,
        matches: match_assertive_modal,
    },
];

/// Scan a page's top-level blocks in document order. Quarantined constructs
/// (raw HTML, tables, unknown extensions) are skipped by variant: only
/// paragraphs and lists are prose a human would retype as a block.
// ponytail: top-level blocks only; recurse into loose ListItem.content if
// partner friction logs ask for it.
pub(crate) fn suggest_typed_blocks(page: &PageAst) -> Vec<SuggestedTypedBlock> {
    page.blocks.iter().filter_map(suggest_block).collect()
}

fn suggest_block(block: &BlockAst) -> Option<SuggestedTypedBlock> {
    RULES.iter().find_map(|rule| {
        (rule.matches)(block).map(|(span, text)| SuggestedTypedBlock {
            span,
            suggested_kind: rule.kind.as_str(),
            matched_rule: rule.name,
            excerpt: excerpt(&text),
        })
    })
}

fn excerpt(text: &str) -> String {
    let first_line = text.lines().next().unwrap_or_default();
    match first_line.char_indices().nth(EXCERPT_MAX_CHARS) {
        // `char_indices` byte offsets are always char boundaries.
        Some((byte, _)) => first_line[..byte].to_string(),
        None => first_line.to_string(),
    }
}

/// A paragraph line or list-item text opening with `TODO` (uppercase-only,
/// followed by end/`:`/space so `TODOS` and prose "todo" never fire). The
/// excerpt is the TODO line itself, not the block's first line. Lines here
/// are hard-break lines: the parser folds soft breaks to spaces, so a TODO
/// continuing a soft-wrapped paragraph is mid-sentence text and stays
/// unsuggested — precision over recall.
fn match_todo_line(block: &BlockAst) -> Option<(SourceSpan, String)> {
    let todo_line = |text: String, span: &SourceSpan| {
        text.lines()
            .map(str::trim_start)
            .find(|line| is_todo_line(line))
            .map(|line| (span.clone(), line.to_string()))
    };
    match block {
        BlockAst::Paragraph(paragraph) => {
            todo_line(plain_text(&paragraph.inlines), &paragraph.span)
        }
        BlockAst::List(list) => list
            .items
            .iter()
            .find_map(|item| todo_line(plain_text(&item.inlines), &item.span)),
        _ => None,
    }
}

fn is_todo_line(line: &str) -> bool {
    line.strip_prefix("TODO")
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(':') || rest.starts_with(' '))
}

fn match_numbered_step_list(block: &BlockAst) -> Option<(SourceSpan, String)> {
    let BlockAst::List(list) = block else {
        return None;
    };
    if list.kind != ListKind::Ordered {
        return None;
    }
    let first_item = list.items.first()?;
    Some((list.span.clone(), plain_text(&first_item.inlines)))
}

fn match_warning_phrase(block: &BlockAst) -> Option<(SourceSpan, String)> {
    paragraph_matching(block, is_warning_text)
}

fn match_definitional_paragraph(block: &BlockAst) -> Option<(SourceSpan, String)> {
    paragraph_matching(block, is_definitional)
}

fn match_assertive_modal(block: &BlockAst) -> Option<(SourceSpan, String)> {
    paragraph_matching(block, has_assertive_modal)
}

fn paragraph_matching(
    block: &BlockAst,
    predicate: fn(&str) -> bool,
) -> Option<(SourceSpan, String)> {
    let BlockAst::Paragraph(ParagraphAst { inlines, span }) = block else {
        return None;
    };
    let text = plain_text(inlines);
    predicate(&text).then(|| (span.clone(), text))
}

const WARNING_MARKERS: &[&str] = &["warning:", "caution:", "danger:", "important:"];

/// An explicit marker prefix, or a prohibition phrase anywhere. Word-based so
/// soft-wrapped "must\nnot" matches and "nevertheless" / "does not" never do.
fn is_warning_text(text: &str) -> bool {
    let lowered = text.to_lowercase();
    if WARNING_MARKERS
        .iter()
        .any(|marker| lowered.starts_with(marker))
    {
        return true;
    }
    let words = words(text);
    words.iter().any(|word| word == "never")
        || words
            .windows(2)
            .any(|pair| (pair[0] == "do" || pair[0] == "must") && pair[1] == "not")
}

/// Opening sentence `<Term> is a/an/the …`: "is" at word index 1 (one-word
/// term) or 2 (article + term), capitalized first word, demonstrative openers
/// excluded — high precision over recall.
fn is_definitional(text: &str) -> bool {
    let sentence = text.split('.').next().unwrap_or_default();
    let words: Vec<&str> = sentence.split_whitespace().collect();
    let Some(first) = words.first() else {
        return false;
    };
    if !first.chars().next().is_some_and(char::is_uppercase) {
        return false;
    }
    if matches!(
        first.to_lowercase().as_str(),
        "this" | "that" | "it" | "there" | "these" | "those"
    ) {
        return false;
    }
    [1usize, 2].iter().any(|&index| {
        words.get(index) == Some(&"is")
            && matches!(
                words.get(index + 1),
                Some(&"a") | Some(&"an") | Some(&"the")
            )
    })
}

fn has_assertive_modal(text: &str) -> bool {
    words(text)
        .iter()
        .any(|word| matches!(word.as_str(), "always" | "guarantees" | "must"))
}

/// Lowercased words with surrounding punctuation stripped ("never." →
/// "never"; "nevertheless" stays intact so it can never equal "never").
fn words(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|word| {
            word.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        })
        .filter(|word| !word.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todo_lines_require_uppercase_and_a_word_boundary() {
        assert!(is_todo_line("TODO"));
        assert!(is_todo_line("TODO: rotate the key"));
        assert!(is_todo_line("TODO rotate the key"));
        assert!(!is_todo_line("todo: lowercase prose"));
        assert!(!is_todo_line("TODOS pile up"));
        assert!(!is_todo_line("Add a TODO here"));
        assert!(!is_todo_line(""));
    }

    #[test]
    fn warning_text_matches_markers_and_prohibitions_on_word_boundaries() {
        assert!(is_warning_text("Warning: the cache is shared."));
        assert!(is_warning_text("IMPORTANT: read this first."));
        assert!(is_warning_text("You must not run this twice."));
        assert!(is_warning_text("Do not re-enable the flag."));
        assert!(is_warning_text("Never delete the ledger."));
        assert!(is_warning_text("Rotation must\nnot skip a step."));
        assert!(!is_warning_text("Nevertheless, the run does not stop."));
        assert!(!is_warning_text("Important note about limits."));
        assert!(!is_warning_text(""));
    }

    #[test]
    fn definitional_text_requires_a_capitalized_term_and_an_early_is_article() {
        assert!(is_definitional("Settlement is the point of no return."));
        assert!(is_definitional("A workspace is the root of a project."));
        assert!(!is_definitional("this pilot is a hand-curated corpus."));
        assert!(!is_definitional("This pilot is a hand-curated corpus."));
        assert!(!is_definitional(
            "The standby health dashboard is the source."
        ));
        assert!(!is_definitional("lambda is the arrival rate."));
        assert!(!is_definitional("Settlement is final."));
        assert!(!is_definitional(""));
    }

    #[test]
    fn assertive_modal_matches_whole_words_only() {
        assert!(has_assertive_modal("Every request must include a token."));
        assert!(has_assertive_modal("The queue always drains in order."));
        assert!(has_assertive_modal("The ledger guarantees idempotency."));
        assert!(!has_assertive_modal("Mustard is a condiment."));
        assert!(!has_assertive_modal("Plain prose."));
    }

    #[test]
    fn excerpt_takes_the_first_line_truncated_on_char_boundaries() {
        assert_eq!(excerpt("first line\nsecond line"), "first line");
        assert_eq!(excerpt(""), "");
        let long = "é".repeat(EXCERPT_MAX_CHARS + 30);
        let truncated = excerpt(&long);
        assert_eq!(truncated.chars().count(), EXCERPT_MAX_CHARS);
        assert_eq!(truncated, "é".repeat(EXCERPT_MAX_CHARS));
    }
}
