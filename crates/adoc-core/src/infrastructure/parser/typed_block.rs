//! Line-by-line typed-block handlers for currently supported typed blocks.
//!
//! Functions in this module are called from [`super::mod`] when the parser
//! sees a typed-block open-fence (`try_open_typed_block`) or when it is already
//! inside a typed block (`consume_typed_block_line`, `finalize_unclosed_typed_block`).

use crate::domain::ast::ParsedTypedBlock;
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::knowledge_object::BlockKind;
use crate::domain::source::SourceFile;

use super::state::{TypedBlockBuildingState, TypedBlockPhase};

const FIELD_KEY_GRAMMAR: &str = "[a-z][a-z0-9_]*";

#[derive(Debug, Clone, Copy)]
struct SupportedKind {
    word: &'static str,
    kind: BlockKind,
}

const SUPPORTED_KINDS: &[SupportedKind] = &[
    SupportedKind {
        word: BlockKind::Claim.as_str(),
        kind: BlockKind::Claim,
    },
    SupportedKind {
        word: BlockKind::Decision.as_str(),
        kind: BlockKind::Decision,
    },
    SupportedKind {
        word: BlockKind::Warning.as_str(),
        kind: BlockKind::Warning,
    },
];

/// Result of attempting to open a typed block on an `Idle` line starting with
/// `::` at column 1.
pub(super) enum TypedBlockOpen {
    /// Not a typed-block opener at all (e.g. line was `::` alone or indented).
    None,
    /// Valid typed-block opener — transition to `TypedBlock` state.
    Opened(TypedBlockBuildingState),
    /// Recognised typed-block syntax but malformed or unknown kind — emit the
    /// diagnostic; consume the line; do NOT enter any block state.
    Diagnostic(Diagnostic),
}

/// Result of feeding one line to an active typed block.
pub(super) enum TypedBlockLineOutcome {
    /// Line consumed; still accumulating the typed block.
    Continue,
    /// `::` close fence encountered — emit the completed `ParsedTypedBlock`.
    Closed(ParsedTypedBlock),
}

/// Attempt to open a typed block when `state == Idle` and the raw line starts
/// with `::` at column 1.
///
/// Recognition rules (per issue #28 D6/D11):
/// - `::<supported-kind> <id>` (single token) → `Opened`.
/// - `::<supported-kind> <id> <junk>` → `Diagnostic(ParseMalformedOpenFence)`.
/// - `::<supported-kind>` alone → `Diagnostic(ParseMalformedOpenFence)`.
/// - `::<word> …` where word is unsupported and non-empty → `Diagnostic(ParseUnknownBlockType)`.
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

    if supported_kind_non_ascii_separator(after_colons).is_some() {
        return TypedBlockOpen::Diagnostic(
            Diagnostic::error(
                DiagnosticCode::ParseMalformedOpenFence,
                format!(
                    "::{} open fence must use ASCII whitespace between `{}` and the id",
                    word, word
                ),
            )
            .with_span(full_line_span),
        );
    }

    let Some(kind) = kind_for_fence_word(word) else {
        return TypedBlockOpen::Diagnostic(
            Diagnostic::error(
                DiagnosticCode::ParseUnknownBlockType,
                format!(
                    "unknown typed-block kind `{word}`; supported kinds: {}",
                    supported_kind_list()
                ),
            )
            .with_span(full_line_span),
        );
    };

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
    TypedBlockOpen::Opened(TypedBlockBuildingState {
        kind,
        id_text,
        open_fence_span,
        phase: TypedBlockPhase::ReadingFields,
        raw_fields: std::collections::BTreeMap::new(),
        duplicate_keys: Vec::new(),
        body_lines: Vec::new(),
        content_spans: Vec::new(),
    })
}

fn kind_for_fence_word(word: &str) -> Option<BlockKind> {
    SUPPORTED_KINDS
        .iter()
        .find(|supported| supported.word == word)
        .map(|supported| supported.kind)
}

fn fence_word_for_kind(kind: BlockKind) -> &'static str {
    SUPPORTED_KINDS
        .iter()
        .find(|supported| supported.kind == kind)
        .map(|supported| supported.word)
        .expect("every BlockKind must have parser metadata")
}

fn supported_kind_non_ascii_separator(value: &str) -> Option<&'static str> {
    SUPPORTED_KINDS.iter().find_map(|supported| {
        value
            .strip_prefix(supported.word)
            .and_then(|rest| rest.chars().next())
            .is_some_and(|character| character.is_whitespace() && !character.is_ascii_whitespace())
            .then_some(supported.word)
    })
}

fn supported_kind_list() -> String {
    SUPPORTED_KINDS
        .iter()
        .map(|supported| supported.word)
        .collect::<Vec<_>>()
        .join(", ")
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
            if line == "::" {
                // Close fence in fields region — no body.
                return TypedBlockLineOutcome::Closed(build_parsed_typed_block(state));
            }

            if line == "--" {
                // Separator: transition to reading the body.
                state.phase = TypedBlockPhase::ReadingBody;
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
            if let Some((key, value)) = try_parse_field(line.trim_end()) {
                if state.raw_fields.contains_key(&key) {
                    state.duplicate_keys.push(key.clone());
                }
                state.raw_fields.insert(key, value);
                return TypedBlockLineOutcome::Continue;
            }

            // Anything else is a malformed field line.
            let kind_word = fence_word_for_kind(state.kind);
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
            if line == "::" {
                // Close fence.
                return TypedBlockLineOutcome::Closed(build_parsed_typed_block(state));
            }
            // Append raw line (preserve all internal whitespace).
            state.body_lines.push(line.to_string());
            state
                .content_spans
                .push(source.span_for_line(line_number, line));
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
    let kind_word = fence_word_for_kind(state.kind);
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

/// Attempt to parse a field line of the form `key: value` or `key:value`
/// where `key` matches `[a-z][a-z0-9_]*`.
fn try_parse_field(trimmed: &str) -> Option<(String, String)> {
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
    let value = after_colon
        .strip_prefix(' ')
        .or_else(|| after_colon.strip_prefix('\t'))
        .unwrap_or(after_colon)
        .to_string();

    Some((key.to_string(), value))
}

/// Assemble a `ParsedTypedBlock` from the current builder state.
/// Leading and trailing fully-blank lines are trimmed from `body_lines`.
fn build_parsed_typed_block(state: &TypedBlockBuildingState) -> ParsedTypedBlock {
    let trimmed_body_lines = trim_blank_edges(&state.body_lines);
    let body_text = trimmed_body_lines.join("\n");

    ParsedTypedBlock {
        kind: state.kind,
        id_text: state.id_text.clone(),
        raw_fields: state.raw_fields.clone(),
        duplicate_keys: state.duplicate_keys.clone(),
        body_text,
        content_spans: state.content_spans.clone(),
        span: state.open_fence_span.clone(),
    }
}

/// Strip leading and trailing elements from `lines` that are blank (trim to
/// empty). Internal blank lines are preserved.
fn trim_blank_edges(lines: &[String]) -> &[String] {
    let start = lines
        .iter()
        .position(|l| !l.trim().is_empty())
        .unwrap_or(lines.len());
    let end = lines
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .map(|i| i + 1)
        .unwrap_or(0);
    if start >= end {
        &lines[0..0]
    } else {
        &lines[start..end]
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
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
            kind: BlockKind::Claim,
            id_text: id.to_string(),
            open_fence_span: dummy_span(),
            phase: TypedBlockPhase::ReadingFields,
            raw_fields: std::collections::BTreeMap::new(),
            duplicate_keys: Vec::new(),
            body_lines: Vec::new(),
            content_spans: Vec::new(),
        }
    }

    #[test]
    fn supported_kind_metadata_round_trips_every_block_kind() {
        for &kind in BlockKind::ALL {
            let word = fence_word_for_kind(kind);
            assert_eq!(kind_for_fence_word(word), Some(kind));
        }
    }

    #[test]
    fn supported_kind_metadata_has_no_duplicate_words_or_kinds() {
        let mut words = BTreeSet::new();
        let mut kinds = Vec::new();

        for supported in SUPPORTED_KINDS {
            assert!(
                words.insert(supported.word),
                "duplicate supported typed-block word `{}`",
                supported.word
            );
            assert!(
                !kinds.contains(&supported.kind),
                "duplicate parser metadata for {:?}",
                supported.kind
            );
            kinds.push(supported.kind);
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
                assert_eq!(state.kind, BlockKind::Claim);
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
                    d.message.contains("ASCII whitespace"),
                    "message should explain ASCII whitespace requirement: {}",
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
                assert_eq!(state.kind, BlockKind::Decision);
                assert_eq!(state.id_text, "foo.bar");
            }
            _ => panic!("expected Opened"),
        }
    }

    #[test]
    fn try_open_typed_block_rejects_unknown_block_kind() {
        let line = "::fact foo.bar";
        let source = source_for_line(line);
        let result = try_open_typed_block(line, 1, &source);
        match result {
            TypedBlockOpen::Diagnostic(d) => {
                assert_eq!(d.code, DiagnosticCode::ParseUnknownBlockType);
                assert!(
                    d.message.contains("fact"),
                    "message should quote the unknown word"
                );
                assert!(
                    d.message
                        .contains("supported kinds: claim, decision, warning"),
                    "message should list supported kinds from parser metadata: {}",
                    d.message
                );
                assert!(
                    !d.message.contains("v0."),
                    "message should not carry stale milestone text: {}",
                    d.message
                );
            }
            _ => panic!("expected Diagnostic(ParseUnknownBlockType)"),
        }
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
