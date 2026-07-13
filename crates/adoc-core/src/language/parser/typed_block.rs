//! Line-by-line typed-block handlers for currently supported typed blocks.
//!
//! Functions in this module are called from [`super::mod`] when the parser
//! sees a typed-block open-fence (`try_open_typed_block`) or when it is already
//! inside a typed block (`consume_typed_block_line`, `finalize_unclosed_typed_block`).

use crate::domain::ast::ParsedTypedBlock;
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, SourceSpan};
use crate::domain::inline::{InlineOrigin, InlineSegment};
use crate::domain::source::SourceFile;

use super::inline::parse_inlines;
use super::state::{TypedBlockBuildingState, TypedBlockPhase};

const FIELD_KEY_GRAMMAR: &str = "[a-z][a-z0-9_]*";
const FENCE_WORD_GRAMMAR: &str = "[a-z][a-z0-9_]*";
const NESTED_TYPED_BLOCK_HELP: &str =
    "nested typed blocks are not supported in V0; declare each typed block at top level";

/// Result of attempting to open a typed block on an `Idle` line starting with
/// `::` at column 1.
pub(super) enum TypedBlockOpen {
    /// Not a typed-block opener at all (e.g. line was `::` alone or indented).
    None,
    /// Valid typed-block opener — transition to `TypedBlock` state. Boxed:
    /// the builder carries span state (V6.4) that would otherwise dominate
    /// the enum's stack footprint.
    Opened(Box<TypedBlockBuildingState>),
    /// Recognised typed-block syntax but malformed or unknown kind — emit the
    /// diagnostic; consume the line; do NOT enter any block state.
    Diagnostic(Diagnostic),
}

/// Result of feeding one line to an active typed block.
pub(super) enum TypedBlockLineOutcome {
    /// Line consumed; still accumulating the typed block.
    Continue,
    /// `::` close fence encountered — emit the completed `ParsedTypedBlock`.
    Closed(Box<ParsedTypedBlock>),
}

/// Attempt to open a typed block when `state == Idle` and the raw line starts
/// with `::` at column 1.
///
/// Recognition rules:
/// - `::<grammar-valid-word> <id>` (single token) → `Opened`.
/// - `::<grammar-valid-word> <id> <junk>` → `Diagnostic(ParseMalformedOpenFence)`.
/// - `::<grammar-valid-word>` alone → `Diagnostic(ParseMalformedOpenFence)`.
/// - `::<grammar-invalid-word> …` → `Diagnostic(ParseMalformedOpenFence)`.
/// - `::` exactly (optional trailing whitespace) → `None`.
pub(super) fn try_open_typed_block(
    line: &str,
    line_number: u32,
    source: &SourceFile,
) -> TypedBlockOpen {
    // Strip the leading `::`.  The caller already checked `line.starts_with("::")`.
    let after_colons = &line[2..];

    // Trim trailing ASCII whitespace for classification, but keep `line` for spans.
    let after_colons_trimmed =
        after_colons.trim_end_matches(|character: char| character.is_ascii_whitespace());

    // `::` alone (optional trailing ws) → None.
    if after_colons_trimmed.is_empty() {
        return TypedBlockOpen::None;
    }

    // Extract the word immediately after `::` (no leading space allowed).
    // `:: foo` has a leading space → word is empty → None (not a typed-block).
    let word_end = after_colons
        .find(|c: char| c.is_ascii_whitespace())
        .unwrap_or(after_colons.len());
    let word = &after_colons[..word_end];

    if word.is_empty() {
        // Space right after `::` — not a typed-block opener.
        return TypedBlockOpen::None;
    }

    let full_line_span = source.span_for_line(line_number, line);

    if !is_fence_word(word) {
        return TypedBlockOpen::Diagnostic(
            Diagnostic::error(
                DiagnosticCode::ParseMalformedOpenFence,
                format!("typed-block kind `{word}` must match {FENCE_WORD_GRAMMAR}"),
            )
            .with_span(full_line_span),
        );
    }

    let rest =
        after_colons[word_end..].trim_matches(|character: char| character.is_ascii_whitespace());

    if rest.is_empty() {
        return TypedBlockOpen::Diagnostic(
            Diagnostic::error(
                DiagnosticCode::ParseMalformedOpenFence,
                format!("::{word} block is missing a required id token"),
            )
            .with_span(full_line_span),
        );
    }

    let mut tokens = rest.split_ascii_whitespace();
    let id_text = tokens
        .next()
        .expect("rest is non-empty so at least one token")
        .to_string();

    if tokens.next().is_some() {
        return TypedBlockOpen::Diagnostic(
            Diagnostic::error(
                DiagnosticCode::ParseMalformedOpenFence,
                format!("::{word} open fence has unexpected tokens after the id `{id_text}`"),
            )
            .with_span(full_line_span),
        );
    }

    let open_fence_span = source.span_for_line(line_number, line);
    // The leading `::` owns columns 1-2; `is_fence_word` keeps `word` ASCII,
    // so character count matches displayed source columns.
    let kind_word_span =
        source.span_for_line_columns(line_number, 3, 3 + word.chars().count() as u32);
    TypedBlockOpen::Opened(Box::new(TypedBlockBuildingState {
        kind_word: word.to_string(),
        kind_word_span,
        id_text,
        open_fence_span,
        phase: TypedBlockPhase::ReadingFields,
        raw_fields: std::collections::BTreeMap::new(),
        raw_field_spans: std::collections::BTreeMap::new(),
        duplicate_keys: Vec::new(),
        body_lines: Vec::new(),
        body_spans: Vec::new(),
        content_spans: Vec::new(),
        body_separator_span: None,
    }))
}

fn is_fence_word(word: &str) -> bool {
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_lowercase()
        && chars.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

/// Silent nested-opener detector for active typed blocks.
///
/// This intentionally mirrors the opener's prefix/word extraction without
/// emitting diagnostics. Grammar-invalid shapes like `::Fact-Cap` remain body
/// text instead of becoming nested-block errors.
fn looks_like_open_fence(line: &str) -> bool {
    let Some(after_colons) = line.strip_prefix("::") else {
        return false;
    };
    if after_colons
        .trim_end_matches(|character: char| character.is_ascii_whitespace())
        .is_empty()
    {
        return false;
    }

    let word_end = after_colons
        .find(|character: char| character.is_ascii_whitespace())
        .unwrap_or(after_colons.len());
    let word = &after_colons[..word_end];
    !word.is_empty() && is_fence_word(word)
}

fn nested_typed_block_diagnostic(source: &SourceFile, line_number: u32, line: &str) -> Diagnostic {
    Diagnostic::error(
        DiagnosticCode::ParseNestedTypedBlock,
        "nested typed block opener is not allowed inside a typed block",
    )
    .with_span(source.span_for_line(line_number, line))
    .with_help(NESTED_TYPED_BLOCK_HELP)
}

/// Feed one line to an active `TypedBlock` state, updating it in place.
///
/// Called for every line while `ParseState::TypedBlock(_)` is active.
pub(super) fn consume_typed_block_line(
    state: &mut TypedBlockBuildingState,
    line: &str,
    line_number: u32,
    source: &SourceFile,
    diagnostics: &mut Vec<Diagnostic>,
) -> TypedBlockLineOutcome {
    match state.phase {
        TypedBlockPhase::ReadingFields => {
            if looks_like_open_fence(line) {
                diagnostics.push(nested_typed_block_diagnostic(source, line_number, line));
                return TypedBlockLineOutcome::Continue;
            }

            if line == "::" {
                // Close fence in fields region — no body.
                return TypedBlockLineOutcome::Closed(Box::new(build_parsed_typed_block(
                    state,
                    source.span_for_line(line_number, line),
                    diagnostics,
                )));
            }

            if line == "--" {
                // Separator: transition to reading the body.
                state.phase = TypedBlockPhase::ReadingBody;
                state.body_separator_span = Some(source.span_for_line(line_number, line));
                return TypedBlockLineOutcome::Continue;
            }

            if line.trim().is_empty() {
                // Blank lines between fields are silently skipped.
                return TypedBlockLineOutcome::Continue;
            }

            // Try to parse as `key: value` where key starts with lowercase
            // and consists of [a-z][a-z0-9_]*.
            state
                .content_spans
                .push(source.span_for_line(line_number, line));
            if let Some(field) = try_parse_field(line.trim_end()) {
                let value_span = source.span_for_line_columns(
                    line_number,
                    field.value_start_column,
                    field.value_start_column + field.value.chars().count() as u32,
                );
                let key = field.key;
                if state.raw_fields.contains_key(&key) {
                    state.duplicate_keys.push(key.clone());
                }
                state.raw_fields.insert(key.clone(), field.value);
                state.raw_field_spans.insert(key, value_span);
                return TypedBlockLineOutcome::Continue;
            }

            // Anything else is a malformed field line.
            let kind_word = state.kind_word.as_str();
            diagnostics.push(
                Diagnostic::error(
                    DiagnosticCode::ParseMalformedField,
                    format!(
                        "malformed {kind_word} field line: {line:?}; {kind_word} field keys must match {FIELD_KEY_GRAMMAR} followed by ':'"
                    ),
                )
                .with_span(source.span_for_line(line_number, line)),
            );
            TypedBlockLineOutcome::Continue
        }

        TypedBlockPhase::ReadingBody => {
            if looks_like_open_fence(line) {
                diagnostics.push(nested_typed_block_diagnostic(source, line_number, line));
                // Preserve the line so close-fence recovery is unchanged; the
                // error blocks artifact writes before this body text can emit.
                state.body_lines.push(line.to_string());
                let span = source.span_for_line(line_number, line);
                state.body_spans.push(span.clone());
                state.content_spans.push(span);
                return TypedBlockLineOutcome::Continue;
            }

            if line == "::" {
                // Close fence.
                return TypedBlockLineOutcome::Closed(Box::new(build_parsed_typed_block(
                    state,
                    source.span_for_line(line_number, line),
                    diagnostics,
                )));
            }
            // Append raw line (preserve all internal whitespace).
            state.body_lines.push(line.to_string());
            let span = source.span_for_line(line_number, line);
            state.body_spans.push(span.clone());
            state.content_spans.push(span);
            TypedBlockLineOutcome::Continue
        }
    }
}

/// Called at EOF when a typed block was never closed.
///
/// Emits one `parse.unclosed_fence` diagnostic and discards the partial state.
/// This intentionally short-circuits schema validation for the unfinished
/// block, so users see the structural fence error before any missing-field
/// follow-on diagnostics.
pub(super) fn finalize_unclosed_typed_block(
    state: TypedBlockBuildingState,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let kind_word = state.kind_word.as_str();
    diagnostics.push(
        Diagnostic::error(
            DiagnosticCode::ParseUnclosedFence,
            format!("::{kind_word} block is missing a closing :: fence"),
        )
        .with_span(state.open_fence_span),
    );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

struct ParsedFieldLine {
    key: String,
    value: String,
    value_start_column: u32,
}

/// Attempt to parse a field line of the form `key: value` or `key:value`
/// where `key` matches `[a-z][a-z0-9_]*`.
fn try_parse_field(trimmed: &str) -> Option<ParsedFieldLine> {
    let colon_pos = trimmed.find(':')?;
    let key = &trimmed[..colon_pos];

    // Key must start with lowercase and contain only [a-z0-9_].
    if key.is_empty() {
        return None;
    }
    let mut key_chars = key.chars();
    let first = key_chars
        .next()
        .expect("key is non-empty per explicit check above");
    if !first.is_ascii_lowercase() {
        return None;
    }
    if !key_chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
        return None;
    }

    // Value is what comes after the colon; strip at most one leading space/tab.
    let after_colon = &trimmed[colon_pos + 1..];
    let (leading_stripped_chars, value) = if let Some(value) = after_colon.strip_prefix(' ') {
        (1, value)
    } else if let Some(value) = after_colon.strip_prefix('\t') {
        (1, value)
    } else {
        (0, after_colon)
    };
    let value_start_column = colon_pos as u32 + 2 + leading_stripped_chars;

    Some(ParsedFieldLine {
        key: key.to_string(),
        value: value.to_string(),
        value_start_column,
    })
}

/// Assemble a `ParsedTypedBlock` from the current builder state.
/// Leading and trailing fully-blank lines are trimmed from `body_lines`.
fn build_parsed_typed_block(
    state: &TypedBlockBuildingState,
    close_fence_span: SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> ParsedTypedBlock {
    let body_range = trim_blank_edges_range(&state.body_lines);
    let body_text = state.body_lines[body_range.clone()].join("\n");
    let body_spans = state.body_spans[body_range].to_vec();
    let body_inlines = parse_body_inlines(&body_text, &body_spans, diagnostics);

    ParsedTypedBlock {
        kind_word: state.kind_word.clone(),
        kind_word_span: state.kind_word_span.clone(),
        id_text: state.id_text.clone(),
        raw_fields: state.raw_fields.clone(),
        raw_field_spans: state.raw_field_spans.clone(),
        duplicate_keys: state.duplicate_keys.clone(),
        body_text,
        body_inlines,
        body_spans,
        content_spans: state.content_spans.clone(),
        span: state.open_fence_span.clone(),
        close_fence_span,
        body_separator_span: state.body_separator_span.clone(),
    }
}

fn parse_body_inlines(
    body_text: &str,
    body_spans: &[SourceSpan],
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<InlineSegment> {
    if body_spans.is_empty() {
        return Vec::new();
    }

    let mut inlines = Vec::new();
    for (index, line) in body_text.split('\n').enumerate() {
        if index > 0 {
            inlines.push(InlineSegment::Text("\n".to_string()));
        }
        let Some(span) = body_spans.get(index) else {
            break;
        };
        let (line_inlines, line_diagnostics) = parse_inlines(line, InlineOrigin::from_span(span));
        diagnostics.extend(line_diagnostics);
        inlines.extend(line_inlines);
    }
    inlines
}

/// Strip leading and trailing elements from `lines` that are blank (trim to
/// empty). Internal blank lines are preserved.
fn trim_blank_edges_range(lines: &[String]) -> std::ops::Range<usize> {
    let start = lines
        .iter()
        .position(|l| !l.trim().is_empty())
        .unwrap_or(lines.len());
    let end = lines
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .map(|i| i + 1)
        .unwrap_or(0);
    if start >= end { 0..0 } else { start..end }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::domain::diagnostic::{DiagnosticCode, SourcePosition, SourceSpan};

    fn make_source(text: &str) -> SourceFile {
        SourceFile::new_with_identity_path(
            PathBuf::from("test.adoc"),
            text.to_string(),
            PathBuf::from("test.adoc"),
        )
    }

    fn source_for_line(line: &str) -> SourceFile {
        make_source(&format!("{line}\n"))
    }

    fn dummy_span() -> SourceSpan {
        SourceSpan {
            file: PathBuf::from("test.adoc"),
            start: SourcePosition {
                line: 1,
                column: 1,
                offset: 0,
            },
            end: SourcePosition {
                line: 1,
                column: 1,
                offset: 0,
            },
        }
    }

    fn fresh_state(id: &str) -> TypedBlockBuildingState {
        TypedBlockBuildingState {
            kind_word: "claim".to_string(),
            kind_word_span: dummy_span(),
            id_text: id.to_string(),
            open_fence_span: dummy_span(),
            phase: TypedBlockPhase::ReadingFields,
            raw_fields: std::collections::BTreeMap::new(),
            raw_field_spans: std::collections::BTreeMap::new(),
            duplicate_keys: Vec::new(),
            body_lines: Vec::new(),
            body_spans: Vec::new(),
            content_spans: Vec::new(),
            body_separator_span: None,
        }
    }

    #[test]
    fn try_open_typed_block_returns_none_for_just_double_colon() {
        let source = source_for_line("::");
        let result = try_open_typed_block("::", 1, &source);
        assert!(
            matches!(result, TypedBlockOpen::None),
            "bare `::` must return None"
        );
    }

    #[test]
    fn try_open_typed_block_returns_none_for_double_colon_with_trailing_spaces() {
        let source = source_for_line("::   ");
        let result = try_open_typed_block("::   ", 1, &source);
        assert!(
            matches!(result, TypedBlockOpen::None),
            "`::   ` must return None"
        );
    }

    #[test]
    fn try_open_typed_block_returns_none_for_double_colon_space_word() {
        let source = source_for_line(":: foo");
        let result = try_open_typed_block(":: foo", 1, &source);
        assert!(
            matches!(result, TypedBlockOpen::None),
            "`:: foo` (space between :: and word) must return None"
        );
    }

    #[test]
    fn try_open_typed_block_recognizes_minimal_claim() {
        let line = "::claim billing.credits";
        let source = source_for_line(line);
        let result = try_open_typed_block(line, 1, &source);
        match result {
            TypedBlockOpen::Opened(state) => {
                assert_eq!(state.kind_word, "claim");
                assert_eq!(state.kind_word_span.start.column, 3);
                assert_eq!(state.kind_word_span.end.column, 8);
                assert_eq!(state.id_text, "billing.credits");
                assert!(matches!(state.phase, TypedBlockPhase::ReadingFields));
                assert!(state.raw_fields.is_empty());
                assert!(state.duplicate_keys.is_empty());
                assert!(state.body_lines.is_empty());
            }
            other => panic!(
                "expected Opened, got {}",
                match other {
                    TypedBlockOpen::None => "None",
                    TypedBlockOpen::Diagnostic(_) => "Diagnostic",
                    TypedBlockOpen::Opened(_) => "Opened",
                }
            ),
        }
    }

    #[test]
    fn try_open_typed_block_tolerates_tab_between_claim_and_id() {
        let line = "::claim\tbilling.credits";
        let source = source_for_line(line);
        let result = try_open_typed_block(line, 1, &source);
        assert!(
            matches!(result, TypedBlockOpen::Opened(_)),
            "tab separator should be tolerated"
        );
    }

    #[test]
    fn try_open_typed_block_rejects_non_ascii_separator_between_claim_and_id() {
        let line = "::claim\u{00a0}billing.credits";
        let source = source_for_line(line);
        let result = try_open_typed_block(line, 1, &source);
        match result {
            TypedBlockOpen::Diagnostic(d) => {
                assert_eq!(d.code, DiagnosticCode::ParseMalformedOpenFence);
                assert!(
                    d.message.contains(FENCE_WORD_GRAMMAR),
                    "message should explain fence-word grammar: {}",
                    d.message
                );
            }
            _ => panic!("expected non-ASCII separator to reject the claim opener"),
        }
    }

    #[test]
    fn try_open_typed_block_tolerates_trailing_spaces_after_id() {
        let line = "::claim billing.credits   ";
        let source = source_for_line(line);
        let result = try_open_typed_block(line, 1, &source);
        match result {
            TypedBlockOpen::Opened(state) => {
                assert_eq!(state.id_text, "billing.credits");
            }
            _ => panic!("expected Opened with trimmed id"),
        }
    }

    #[test]
    fn try_open_typed_block_recognizes_minimal_decision() {
        let line = "::decision foo.bar";
        let source = source_for_line(line);
        let result = try_open_typed_block(line, 1, &source);
        match result {
            TypedBlockOpen::Opened(state) => {
                assert_eq!(state.kind_word, "decision");
                assert_eq!(state.id_text, "foo.bar");
            }
            _ => panic!("expected Opened"),
        }
    }

    #[test]
    fn try_open_typed_block_opens_unknown_grammar_valid_block_kind() {
        let line = "::fact foo.bar";
        let source = source_for_line(line);
        let result = try_open_typed_block(line, 1, &source);
        match result {
            TypedBlockOpen::Opened(state) => {
                assert_eq!(state.kind_word, "fact");
                assert_eq!(state.id_text, "foo.bar");
                assert_eq!(state.kind_word_span.start.column, 3);
                assert_eq!(state.kind_word_span.end.column, 7);
            }
            _ => panic!("expected unknown grammar-valid kind to open"),
        }
    }

    #[test]
    fn try_open_typed_block_rejects_grammar_invalid_block_kind() {
        let line = "::Fact-Cap foo.bar";
        let source = source_for_line(line);
        let result = try_open_typed_block(line, 1, &source);
        match result {
            TypedBlockOpen::Diagnostic(d) => {
                assert_eq!(d.code, DiagnosticCode::ParseMalformedOpenFence);
                assert!(
                    d.message.contains(FENCE_WORD_GRAMMAR),
                    "message should explain fence-word grammar: {}",
                    d.message
                );
            }
            _ => panic!("expected Diagnostic(ParseMalformedOpenFence)"),
        }
    }

    #[test]
    fn looks_like_open_fence_recognizes_only_column_one_grammar_valid_openers() {
        assert!(looks_like_open_fence("::warning auth.session"));
        assert!(looks_like_open_fence("::fact billing.policy"));
        assert!(looks_like_open_fence("::fact"));
        assert!(!looks_like_open_fence("::"));
        assert!(!looks_like_open_fence("::   "));
        assert!(!looks_like_open_fence(" ::warning auth.session"));
        assert!(!looks_like_open_fence("::~ascii"));
        assert!(!looks_like_open_fence("::Fact-Cap foo.bar"));
    }

    #[test]
    fn try_open_typed_block_rejects_trailing_junk() {
        let line = "::claim foo.bar trailing";
        let source = source_for_line(line);
        let result = try_open_typed_block(line, 1, &source);
        match result {
            TypedBlockOpen::Diagnostic(d) => {
                assert_eq!(d.code, DiagnosticCode::ParseMalformedOpenFence);
            }
            _ => panic!("expected Diagnostic(ParseMalformedOpenFence)"),
        }
    }

    #[test]
    fn try_open_typed_block_rejects_missing_id() {
        let line = "::claim";
        let source = source_for_line(line);
        let result = try_open_typed_block(line, 1, &source);
        match result {
            TypedBlockOpen::Diagnostic(d) => {
                assert_eq!(d.code, DiagnosticCode::ParseMalformedOpenFence);
            }
            _ => panic!("expected Diagnostic(ParseMalformedOpenFence) for missing id"),
        }
    }

    #[test]
    fn try_open_typed_block_rejects_claim_with_only_whitespace_after() {
        let line = "::claim   ";
        let source = source_for_line(line);
        let result = try_open_typed_block(line, 1, &source);
        match result {
            TypedBlockOpen::Diagnostic(d) => {
                assert_eq!(d.code, DiagnosticCode::ParseMalformedOpenFence);
            }
            _ => panic!("expected Diagnostic for ::claim with whitespace only after"),
        }
    }

    #[test]
    fn consume_typed_block_line_records_field() {
        let source = make_source("status: verified\n");
        let mut state = fresh_state("billing.credits");
        let mut diagnostics = Vec::new();

        let outcome =
            consume_typed_block_line(&mut state, "status: verified", 1, &source, &mut diagnostics);

        assert!(matches!(outcome, TypedBlockLineOutcome::Continue));
        assert_eq!(
            state.raw_fields.get("status").map(String::as_str),
            Some("verified")
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn consume_typed_block_line_tracks_duplicate_field() {
        let source = make_source("status: first\nstatus: second\n");
        let mut state = fresh_state("billing.credits");
        let mut diagnostics = Vec::new();

        consume_typed_block_line(&mut state, "status: first", 1, &source, &mut diagnostics);
        consume_typed_block_line(&mut state, "status: second", 2, &source, &mut diagnostics);

        assert!(
            state.duplicate_keys.contains(&"status".to_string()),
            "duplicate_keys should contain 'status'"
        );
        assert_eq!(
            state.raw_fields.get("status").map(String::as_str),
            Some("second"),
            "last value wins"
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn consume_typed_block_line_transitions_to_body_on_separator() {
        let source = make_source("--\n");
        let mut state = fresh_state("billing.credits");
        let mut diagnostics = Vec::new();

        let outcome = consume_typed_block_line(&mut state, "--", 1, &source, &mut diagnostics);

        assert!(matches!(outcome, TypedBlockLineOutcome::Continue));
        assert!(matches!(state.phase, TypedBlockPhase::ReadingBody));
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn consume_typed_block_line_requires_separator_at_exact_column_one() {
        let source = make_source("  --\n");
        let mut state = fresh_state("billing.credits");
        let mut diagnostics = Vec::new();

        let outcome = consume_typed_block_line(&mut state, "  --", 1, &source, &mut diagnostics);

        assert!(matches!(outcome, TypedBlockLineOutcome::Continue));
        assert!(matches!(state.phase, TypedBlockPhase::ReadingFields));
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::ParseMalformedField);
    }

    #[test]
    fn consume_typed_block_line_requires_separator_without_trailing_whitespace() {
        let source = make_source("-- \n");
        let mut state = fresh_state("billing.credits");
        let mut diagnostics = Vec::new();

        let outcome = consume_typed_block_line(&mut state, "-- ", 1, &source, &mut diagnostics);

        assert!(matches!(outcome, TypedBlockLineOutcome::Continue));
        assert!(matches!(state.phase, TypedBlockPhase::ReadingFields));
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::ParseMalformedField);
    }

    #[test]
    fn consume_typed_block_line_requires_field_key_at_column_one() {
        let source = make_source("  status: verified\n");
        let mut state = fresh_state("billing.credits");
        let mut diagnostics = Vec::new();

        let outcome = consume_typed_block_line(
            &mut state,
            "  status: verified",
            1,
            &source,
            &mut diagnostics,
        );

        assert!(matches!(outcome, TypedBlockLineOutcome::Continue));
        assert!(state.raw_fields.is_empty());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::ParseMalformedField);
    }

    #[test]
    fn consume_typed_block_line_emits_malformed_field_for_capitalized_key() {
        let source = make_source("Status: x\n");
        let mut state = fresh_state("billing.credits");
        let mut diagnostics = Vec::new();

        let outcome =
            consume_typed_block_line(&mut state, "Status: x", 1, &source, &mut diagnostics);

        assert!(matches!(outcome, TypedBlockLineOutcome::Continue));
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::ParseMalformedField);
        assert!(
            diagnostics[0].message.contains(FIELD_KEY_GRAMMAR),
            "message should explain field key grammar: {}",
            diagnostics[0].message
        );
        assert!(
            state.raw_fields.is_empty(),
            "malformed line must not be stored"
        );
    }

    #[test]
    fn consume_typed_block_line_rejects_nested_open_fence_in_fields_phase() {
        let source = make_source("::warning auth.session\n");
        let mut state = fresh_state("billing.credits");
        let mut diagnostics = Vec::new();

        let outcome = consume_typed_block_line(
            &mut state,
            "::warning auth.session",
            1,
            &source,
            &mut diagnostics,
        );

        assert!(matches!(outcome, TypedBlockLineOutcome::Continue));
        assert!(matches!(state.phase, TypedBlockPhase::ReadingFields));
        assert!(state.raw_fields.is_empty(), "nested opener is not a field");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::ParseNestedTypedBlock);
        assert_eq!(
            diagnostics[0]
                .span
                .as_ref()
                .map(|span| (span.start.line, span.start.column)),
            Some((1, 1))
        );
    }

    #[test]
    fn consume_typed_block_line_explains_field_key_grammar_for_hyphenated_key() {
        let source = make_source("reviewed-by: team-a\n");
        let mut state = fresh_state("billing.credits");
        let mut diagnostics = Vec::new();

        let outcome = consume_typed_block_line(
            &mut state,
            "reviewed-by: team-a",
            1,
            &source,
            &mut diagnostics,
        );

        assert!(matches!(outcome, TypedBlockLineOutcome::Continue));
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::ParseMalformedField);
        assert!(
            diagnostics[0].message.contains(FIELD_KEY_GRAMMAR),
            "message should explain field key grammar: {}",
            diagnostics[0].message
        );
        assert!(
            state.raw_fields.is_empty(),
            "malformed line must not be stored"
        );
    }

    #[test]
    fn consume_typed_block_line_appends_body_lines_in_body_phase() {
        let source = make_source("--\nfirst body line\nsecond body line\n");
        let mut state = fresh_state("billing.credits");
        let mut diagnostics = Vec::new();

        // transition to body
        consume_typed_block_line(&mut state, "--", 1, &source, &mut diagnostics);
        consume_typed_block_line(&mut state, "first body line", 2, &source, &mut diagnostics);
        consume_typed_block_line(&mut state, "second body line", 3, &source, &mut diagnostics);

        assert_eq!(
            state.body_lines,
            vec!["first body line", "second body line"]
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn consume_typed_block_line_rejects_nested_open_fence_in_body_phase_and_preserves_line() {
        let source = make_source("::fact billing.policy\n");
        let mut state = fresh_state("billing.credits");
        state.phase = TypedBlockPhase::ReadingBody;
        let mut diagnostics = Vec::new();

        let outcome = consume_typed_block_line(
            &mut state,
            "::fact billing.policy",
            1,
            &source,
            &mut diagnostics,
        );

        assert!(matches!(outcome, TypedBlockLineOutcome::Continue));
        assert_eq!(state.body_lines, vec!["::fact billing.policy"]);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::ParseNestedTypedBlock);
        assert_eq!(
            diagnostics[0]
                .span
                .as_ref()
                .map(|span| (span.start.line, span.start.column)),
            Some((1, 1))
        );
    }

    #[test]
    fn consume_typed_block_line_keeps_grammar_invalid_open_fence_shape_in_body() {
        let source = make_source("::Fact-Cap billing.policy\n");
        let mut state = fresh_state("billing.credits");
        state.phase = TypedBlockPhase::ReadingBody;
        let mut diagnostics = Vec::new();

        let outcome = consume_typed_block_line(
            &mut state,
            "::Fact-Cap billing.policy",
            1,
            &source,
            &mut diagnostics,
        );

        assert!(matches!(outcome, TypedBlockLineOutcome::Continue));
        assert_eq!(state.body_lines, vec!["::Fact-Cap billing.policy"]);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn consume_typed_block_line_closes_on_double_colon() {
        let source = make_source("status: verified\n--\nSome body text.\n::\n");
        let mut state = fresh_state("billing.credits");
        let mut diagnostics = Vec::new();

        consume_typed_block_line(&mut state, "status: verified", 1, &source, &mut diagnostics);
        consume_typed_block_line(&mut state, "--", 2, &source, &mut diagnostics);
        consume_typed_block_line(&mut state, "Some body text.", 3, &source, &mut diagnostics);
        let outcome = consume_typed_block_line(&mut state, "::", 4, &source, &mut diagnostics);

        match outcome {
            TypedBlockLineOutcome::Closed(parsed) => {
                assert_eq!(parsed.id_text, "billing.credits");
                assert_eq!(parsed.body_text, "Some body text.");
                assert_eq!(
                    parsed.body_inlines,
                    vec![InlineSegment::Text("Some body text.".to_string())]
                );
                assert_eq!(
                    parsed.raw_fields.get("status").map(String::as_str),
                    Some("verified")
                );
                let content_lines: Vec<u32> = parsed
                    .content_spans
                    .iter()
                    .map(|span| span.start.line)
                    .collect();
                assert_eq!(content_lines, vec![1, 3]);
            }
            TypedBlockLineOutcome::Continue => panic!("expected Closed"),
        }
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn consume_typed_block_line_keeps_indented_double_colon_in_body() {
        let source = make_source("  ::\n");
        let mut state = fresh_state("billing.credits");
        state.phase = TypedBlockPhase::ReadingBody;
        let mut diagnostics = Vec::new();

        let outcome = consume_typed_block_line(&mut state, "  ::", 1, &source, &mut diagnostics);

        assert!(matches!(outcome, TypedBlockLineOutcome::Continue));
        assert_eq!(state.body_lines, vec!["  ::"]);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn consume_typed_block_line_keeps_trailing_space_double_colon_in_body() {
        let source = make_source(":: \n");
        let mut state = fresh_state("billing.credits");
        state.phase = TypedBlockPhase::ReadingBody;
        let mut diagnostics = Vec::new();

        let outcome = consume_typed_block_line(&mut state, ":: ", 1, &source, &mut diagnostics);

        assert!(matches!(outcome, TypedBlockLineOutcome::Continue));
        assert_eq!(state.body_lines, vec![":: "]);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn consume_typed_block_line_trims_leading_and_trailing_blank_body_lines() {
        let source = make_source("--\n\nline one\n\nline two\n\n::\n");
        let mut state = fresh_state("billing.credits");
        let mut diagnostics = Vec::new();

        consume_typed_block_line(&mut state, "--", 1, &source, &mut diagnostics);
        consume_typed_block_line(&mut state, "", 2, &source, &mut diagnostics);
        consume_typed_block_line(&mut state, "line one", 3, &source, &mut diagnostics);
        consume_typed_block_line(&mut state, "", 4, &source, &mut diagnostics);
        consume_typed_block_line(&mut state, "line two", 5, &source, &mut diagnostics);
        consume_typed_block_line(&mut state, "", 6, &source, &mut diagnostics);
        let outcome = consume_typed_block_line(&mut state, "::", 7, &source, &mut diagnostics);

        match outcome {
            TypedBlockLineOutcome::Closed(parsed) => {
                // Leading/trailing blank lines stripped; internal blank preserved.
                assert_eq!(parsed.body_text, "line one\n\nline two");
                assert_eq!(
                    parsed.body_inlines,
                    vec![
                        InlineSegment::Text("line one".to_string()),
                        InlineSegment::Text("\n".to_string()),
                        InlineSegment::Text("\n".to_string()),
                        InlineSegment::Text("line two".to_string()),
                    ]
                );
            }
            TypedBlockLineOutcome::Continue => panic!("expected Closed"),
        }
    }

    // Test: close fence in fields region (no body)
    #[test]
    fn consume_typed_block_line_closes_on_double_colon_in_fields_region() {
        let source = make_source("status: verified\n::\n");
        let mut state = fresh_state("billing.credits");
        let mut diagnostics = Vec::new();

        consume_typed_block_line(&mut state, "status: verified", 1, &source, &mut diagnostics);
        let outcome = consume_typed_block_line(&mut state, "::", 2, &source, &mut diagnostics);

        match outcome {
            TypedBlockLineOutcome::Closed(parsed) => {
                assert_eq!(parsed.body_text, "");
            }
            TypedBlockLineOutcome::Continue => panic!("expected Closed"),
        }
    }

    // Test: blank lines between fields are silently skipped
    #[test]
    fn consume_typed_block_line_skips_blank_lines_in_fields_region() {
        let source = make_source("status: verified\n\nowner: team-a\n");
        let mut state = fresh_state("billing.credits");
        let mut diagnostics = Vec::new();

        consume_typed_block_line(&mut state, "status: verified", 1, &source, &mut diagnostics);
        consume_typed_block_line(&mut state, "", 2, &source, &mut diagnostics);
        consume_typed_block_line(&mut state, "owner: team-a", 3, &source, &mut diagnostics);

        assert!(diagnostics.is_empty());
        assert_eq!(state.raw_fields.len(), 2);
    }

    #[test]
    fn close_fence_span_is_recorded_when_closing_from_body_phase() {
        let source = make_source("status: verified\n--\nBody.\n::\n");
        let mut state = fresh_state("billing.credits");
        let mut diagnostics = Vec::new();

        consume_typed_block_line(&mut state, "status: verified", 1, &source, &mut diagnostics);
        consume_typed_block_line(&mut state, "--", 2, &source, &mut diagnostics);
        consume_typed_block_line(&mut state, "Body.", 3, &source, &mut diagnostics);
        let outcome = consume_typed_block_line(&mut state, "::", 4, &source, &mut diagnostics);

        match outcome {
            TypedBlockLineOutcome::Closed(parsed) => {
                assert_eq!(parsed.close_fence_span.start.line, 4);
                assert_eq!(parsed.close_fence_span.start.column, 1);
                assert_eq!(parsed.close_fence_span.end.column, 3);
                // "status: verified\n--\nBody.\n" = 17 + 3 + 6 = 26 bytes before `::`.
                assert_eq!(parsed.close_fence_span.start.offset, 26);
                assert_eq!(parsed.close_fence_span.end.offset, 28);
                let separator = parsed
                    .body_separator_span
                    .as_ref()
                    .expect("separator span recorded");
                assert_eq!(separator.start.line, 2);
                assert_eq!(separator.start.offset, 17);
                assert_eq!(separator.end.offset, 19);
            }
            TypedBlockLineOutcome::Continue => panic!("expected Closed"),
        }
    }

    #[test]
    fn close_fence_span_is_recorded_when_closing_from_fields_phase_without_separator() {
        let source = make_source("status: verified\n::\n");
        let mut state = fresh_state("billing.credits");
        let mut diagnostics = Vec::new();

        consume_typed_block_line(&mut state, "status: verified", 1, &source, &mut diagnostics);
        let outcome = consume_typed_block_line(&mut state, "::", 2, &source, &mut diagnostics);

        match outcome {
            TypedBlockLineOutcome::Closed(parsed) => {
                assert_eq!(parsed.close_fence_span.start.line, 2);
                assert_eq!(parsed.close_fence_span.start.offset, 17);
                assert!(
                    parsed.body_separator_span.is_none(),
                    "no separator line was parsed"
                );
            }
            TypedBlockLineOutcome::Continue => panic!("expected Closed"),
        }
    }

    // Test: finalize_unclosed_typed_block emits the right diagnostic
    #[test]
    fn finalize_unclosed_typed_block_emits_unclosed_fence_diagnostic() {
        let mut diagnostics = Vec::new();
        let state = fresh_state("billing.credits");
        finalize_unclosed_typed_block(state, &mut diagnostics);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::ParseUnclosedFence);
        assert!(diagnostics[0].message.contains("::claim"));
    }
}
