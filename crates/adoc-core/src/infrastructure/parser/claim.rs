//! Line-by-line claim-block handlers for the `::claim` typed-block parser.
//!
//! Functions in this module are called from [`super::mod`] when the parser
//! sees a `::claim` open-fence (`try_open_typed_block`) or when it is already
//! inside a claim block (`consume_claim_line`, `finalize_unclosed_claim`).

use crate::domain::ast::ParsedClaim;
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::source::SourceFile;

use super::state::{ClaimBuildingState, ClaimPhase};

/// Result of attempting to open a typed block on an `Idle` line starting with
/// `::` at column 1.
pub(super) enum TypedBlockOpen {
    /// Not a typed-block opener at all (e.g. line was `::` alone or indented).
    None,
    /// Valid `::claim <id>` opener — transition to `ClaimBlock` state.
    OpenedClaim(ClaimBuildingState),
    /// Recognised typed-block syntax but malformed or unknown kind — emit the
    /// diagnostic; consume the line; do NOT enter any block state.
    Diagnostic(Diagnostic),
}

/// Result of feeding one line to an active claim block.
pub(super) enum ClaimLineOutcome {
    /// Line consumed; still accumulating the claim block.
    Continue,
    /// `::` close fence encountered — emit the completed `ParsedClaim`.
    Closed(ParsedClaim),
}

/// Attempt to open a typed block when `state == Idle` and the raw line starts
/// with `::` at column 1.
///
/// Recognition rules (per issue #28 D6/D11):
/// - `::claim <id>` (single token) → `OpenedClaim`.
/// - `::claim <id> <junk>` → `Diagnostic(ParseMalformedOpenFence)`.
/// - `::claim` alone → `Diagnostic(ParseMalformedOpenFence)`.
/// - `::<word> …` where word ≠ `claim` and word non-empty → `Diagnostic(ParseUnknownBlockType)`.
/// - `::` exactly (optional trailing whitespace) → `None`.
pub(super) fn try_open_typed_block(
    line: &str,
    line_number: u32,
    source: &SourceFile,
) -> TypedBlockOpen {
    // Strip the leading `::`.  The caller already checked `line.starts_with("::")`.
    let after_colons = &line[2..];

    // Trim trailing whitespace for classification, but keep `line` for spans.
    let after_colons_trimmed = after_colons.trim_end();

    // `::` alone (optional trailing ws) → None.
    if after_colons_trimmed.is_empty() {
        return TypedBlockOpen::None;
    }

    // Extract the word immediately after `::` (no leading space allowed).
    // `:: foo` has a leading space → word is empty → None (not a typed-block).
    let word_end = after_colons
        .find(|c: char| c.is_whitespace())
        .unwrap_or(after_colons.len());
    let word = &after_colons[..word_end];

    if word.is_empty() {
        // Space right after `::` — not a typed-block opener.
        return TypedBlockOpen::None;
    }

    let full_line_span = source.span_for_line(line_number, line);

    match word {
        "claim" => {
            // Everything after `::claim`.
            let rest = after_colons[word_end..].trim();

            if rest.is_empty() {
                // `::claim` with no id.
                return TypedBlockOpen::Diagnostic(
                    Diagnostic::error(
                        DiagnosticCode::ParseMalformedOpenFence,
                        "::claim block is missing a required id token",
                    )
                    .with_span(full_line_span),
                );
            }

            // Whitespace-separated tokens in `rest`.
            let mut tokens = rest.split_whitespace();
            let id_text = tokens
                .next()
                .expect("rest is non-empty so at least one token")
                .to_string();

            if tokens.next().is_some() {
                // Trailing junk after the id.
                return TypedBlockOpen::Diagnostic(
                    Diagnostic::error(
                        DiagnosticCode::ParseMalformedOpenFence,
                        format!(
                            "::claim open fence has unexpected tokens after the id `{id_text}`"
                        ),
                    )
                    .with_span(full_line_span),
                );
            }

            let open_fence_span = source.span_for_line(line_number, line);
            TypedBlockOpen::OpenedClaim(ClaimBuildingState {
                id_text,
                open_fence_span,
                phase: ClaimPhase::ReadingFields,
                raw_fields: std::collections::BTreeMap::new(),
                duplicate_keys: Vec::new(),
                body_lines: Vec::new(),
            })
        }
        unknown => TypedBlockOpen::Diagnostic(
            Diagnostic::error(
                DiagnosticCode::ParseUnknownBlockType,
                format!("unknown typed-block kind `{unknown}`; supported in v0.2: claim"),
            )
            .with_span(full_line_span),
        ),
    }
}

/// Feed one line to an active `ClaimBlock` state, updating it in place.
///
/// Called for every line while `ParseState::ClaimBlock(_)` is active.
pub(super) fn consume_claim_line(
    state: &mut ClaimBuildingState,
    line: &str,
    line_number: u32,
    source: &SourceFile,
    diagnostics: &mut Vec<Diagnostic>,
) -> ClaimLineOutcome {
    match state.phase {
        ClaimPhase::ReadingFields => {
            if line == "::" {
                // Close fence in fields region — no body.
                return ClaimLineOutcome::Closed(build_parsed_claim(state));
            }

            if line == "--" {
                // Separator: transition to reading the body.
                state.phase = ClaimPhase::ReadingBody;
                return ClaimLineOutcome::Continue;
            }

            if line.trim().is_empty() {
                // Blank lines between fields are silently skipped.
                return ClaimLineOutcome::Continue;
            }

            // Try to parse as `key: value` where key starts with lowercase
            // and consists of [a-z][a-z0-9_]*.
            if let Some((key, value)) = try_parse_field(line.trim_end()) {
                if state.raw_fields.contains_key(&key) {
                    state.duplicate_keys.push(key.clone());
                }
                state.raw_fields.insert(key, value);
                return ClaimLineOutcome::Continue;
            }

            // Anything else is a malformed field line.
            diagnostics.push(
                Diagnostic::error(
                    DiagnosticCode::ParseMalformedField,
                    format!("malformed claim field line: {line:?}"),
                )
                .with_span(source.span_for_line(line_number, line)),
            );
            ClaimLineOutcome::Continue
        }

        ClaimPhase::ReadingBody => {
            if line == "::" {
                // Close fence.
                return ClaimLineOutcome::Closed(build_parsed_claim(state));
            }
            // Append raw line (preserve all internal whitespace).
            state.body_lines.push(line.to_string());
            ClaimLineOutcome::Continue
        }
    }
}

/// Called at EOF when a `::claim` block was never closed.
///
/// Emits one `parse.unclosed_fence` diagnostic and discards the partial state.
pub(super) fn finalize_unclosed_claim(
    state: ClaimBuildingState,
    diagnostics: &mut Vec<Diagnostic>,
) {
    diagnostics.push(
        Diagnostic::error(
            DiagnosticCode::ParseUnclosedFence,
            "::claim block is missing a closing :: fence",
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
    let first = key_chars.next()?;
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

/// Assemble a `ParsedClaim` from the current builder state.
/// Leading and trailing fully-blank lines are trimmed from `body_lines`.
fn build_parsed_claim(state: &ClaimBuildingState) -> ParsedClaim {
    let trimmed_body_lines = trim_blank_edges(&state.body_lines);
    let body_text = trimmed_body_lines.join("\n");

    ParsedClaim {
        id_text: state.id_text.clone(),
        raw_fields: state.raw_fields.clone(),
        duplicate_keys: state.duplicate_keys.clone(),
        body_text,
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

    fn fresh_state(id: &str) -> ClaimBuildingState {
        ClaimBuildingState {
            id_text: id.to_string(),
            open_fence_span: dummy_span(),
            phase: ClaimPhase::ReadingFields,
            raw_fields: std::collections::BTreeMap::new(),
            duplicate_keys: Vec::new(),
            body_lines: Vec::new(),
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
            TypedBlockOpen::OpenedClaim(state) => {
                assert_eq!(state.id_text, "billing.credits");
                assert!(matches!(state.phase, ClaimPhase::ReadingFields));
                assert!(state.raw_fields.is_empty());
                assert!(state.duplicate_keys.is_empty());
                assert!(state.body_lines.is_empty());
            }
            other => panic!(
                "expected OpenedClaim, got {}",
                match other {
                    TypedBlockOpen::None => "None",
                    TypedBlockOpen::Diagnostic(_) => "Diagnostic",
                    TypedBlockOpen::OpenedClaim(_) => "OpenedClaim",
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
            matches!(result, TypedBlockOpen::OpenedClaim(_)),
            "tab separator should be tolerated"
        );
    }

    #[test]
    fn try_open_typed_block_tolerates_trailing_spaces_after_id() {
        let line = "::claim billing.credits   ";
        let source = source_for_line(line);
        let result = try_open_typed_block(line, 1, &source);
        match result {
            TypedBlockOpen::OpenedClaim(state) => {
                assert_eq!(state.id_text, "billing.credits");
            }
            _ => panic!("expected OpenedClaim with trimmed id"),
        }
    }

    #[test]
    fn try_open_typed_block_rejects_unknown_block_kind() {
        let line = "::decision foo.bar";
        let source = source_for_line(line);
        let result = try_open_typed_block(line, 1, &source);
        match result {
            TypedBlockOpen::Diagnostic(d) => {
                assert_eq!(d.code, DiagnosticCode::ParseUnknownBlockType);
                assert!(
                    d.message.contains("decision"),
                    "message should quote the unknown word"
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
    fn consume_claim_line_records_field() {
        let source = make_source("status: verified\n");
        let mut state = fresh_state("billing.credits");
        let mut diagnostics = Vec::new();

        let outcome =
            consume_claim_line(&mut state, "status: verified", 1, &source, &mut diagnostics);

        assert!(matches!(outcome, ClaimLineOutcome::Continue));
        assert_eq!(
            state.raw_fields.get("status").map(String::as_str),
            Some("verified")
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn consume_claim_line_tracks_duplicate_field() {
        let source = make_source("status: first\nstatus: second\n");
        let mut state = fresh_state("billing.credits");
        let mut diagnostics = Vec::new();

        consume_claim_line(&mut state, "status: first", 1, &source, &mut diagnostics);
        consume_claim_line(&mut state, "status: second", 2, &source, &mut diagnostics);

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
    fn consume_claim_line_transitions_to_body_on_separator() {
        let source = make_source("--\n");
        let mut state = fresh_state("billing.credits");
        let mut diagnostics = Vec::new();

        let outcome = consume_claim_line(&mut state, "--", 1, &source, &mut diagnostics);

        assert!(matches!(outcome, ClaimLineOutcome::Continue));
        assert!(matches!(state.phase, ClaimPhase::ReadingBody));
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn consume_claim_line_requires_separator_at_exact_column_one() {
        let source = make_source("  --\n");
        let mut state = fresh_state("billing.credits");
        let mut diagnostics = Vec::new();

        let outcome = consume_claim_line(&mut state, "  --", 1, &source, &mut diagnostics);

        assert!(matches!(outcome, ClaimLineOutcome::Continue));
        assert!(matches!(state.phase, ClaimPhase::ReadingFields));
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::ParseMalformedField);
    }

    #[test]
    fn consume_claim_line_requires_separator_without_trailing_whitespace() {
        let source = make_source("-- \n");
        let mut state = fresh_state("billing.credits");
        let mut diagnostics = Vec::new();

        let outcome = consume_claim_line(&mut state, "-- ", 1, &source, &mut diagnostics);

        assert!(matches!(outcome, ClaimLineOutcome::Continue));
        assert!(matches!(state.phase, ClaimPhase::ReadingFields));
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::ParseMalformedField);
    }

    #[test]
    fn consume_claim_line_requires_field_key_at_column_one() {
        let source = make_source("  status: verified\n");
        let mut state = fresh_state("billing.credits");
        let mut diagnostics = Vec::new();

        let outcome =
            consume_claim_line(&mut state, "  status: verified", 1, &source, &mut diagnostics);

        assert!(matches!(outcome, ClaimLineOutcome::Continue));
        assert!(state.raw_fields.is_empty());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::ParseMalformedField);
    }

    #[test]
    fn consume_claim_line_emits_malformed_field_for_capitalized_key() {
        let source = make_source("Status: x\n");
        let mut state = fresh_state("billing.credits");
        let mut diagnostics = Vec::new();

        let outcome = consume_claim_line(&mut state, "Status: x", 1, &source, &mut diagnostics);

        assert!(matches!(outcome, ClaimLineOutcome::Continue));
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::ParseMalformedField);
        assert!(
            state.raw_fields.is_empty(),
            "malformed line must not be stored"
        );
    }

    #[test]
    fn consume_claim_line_appends_body_lines_in_body_phase() {
        let source = make_source("--\nfirst body line\nsecond body line\n");
        let mut state = fresh_state("billing.credits");
        let mut diagnostics = Vec::new();

        // transition to body
        consume_claim_line(&mut state, "--", 1, &source, &mut diagnostics);
        consume_claim_line(&mut state, "first body line", 2, &source, &mut diagnostics);
        consume_claim_line(&mut state, "second body line", 3, &source, &mut diagnostics);

        assert_eq!(
            state.body_lines,
            vec!["first body line", "second body line"]
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn consume_claim_line_closes_on_double_colon() {
        let source = make_source("status: verified\n--\nSome body text.\n::\n");
        let mut state = fresh_state("billing.credits");
        let mut diagnostics = Vec::new();

        consume_claim_line(&mut state, "status: verified", 1, &source, &mut diagnostics);
        consume_claim_line(&mut state, "--", 2, &source, &mut diagnostics);
        consume_claim_line(&mut state, "Some body text.", 3, &source, &mut diagnostics);
        let outcome = consume_claim_line(&mut state, "::", 4, &source, &mut diagnostics);

        match outcome {
            ClaimLineOutcome::Closed(parsed) => {
                assert_eq!(parsed.id_text, "billing.credits");
                assert_eq!(parsed.body_text, "Some body text.");
                assert_eq!(
                    parsed.raw_fields.get("status").map(String::as_str),
                    Some("verified")
                );
            }
            ClaimLineOutcome::Continue => panic!("expected Closed"),
        }
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn consume_claim_line_keeps_indented_double_colon_in_body() {
        let source = make_source("  ::\n");
        let mut state = fresh_state("billing.credits");
        state.phase = ClaimPhase::ReadingBody;
        let mut diagnostics = Vec::new();

        let outcome = consume_claim_line(&mut state, "  ::", 1, &source, &mut diagnostics);

        assert!(matches!(outcome, ClaimLineOutcome::Continue));
        assert_eq!(state.body_lines, vec!["  ::"]);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn consume_claim_line_keeps_trailing_space_double_colon_in_body() {
        let source = make_source(":: \n");
        let mut state = fresh_state("billing.credits");
        state.phase = ClaimPhase::ReadingBody;
        let mut diagnostics = Vec::new();

        let outcome = consume_claim_line(&mut state, ":: ", 1, &source, &mut diagnostics);

        assert!(matches!(outcome, ClaimLineOutcome::Continue));
        assert_eq!(state.body_lines, vec![":: "]);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn consume_claim_line_trims_leading_and_trailing_blank_body_lines() {
        let source = make_source("--\n\nline one\n\nline two\n\n::\n");
        let mut state = fresh_state("billing.credits");
        let mut diagnostics = Vec::new();

        consume_claim_line(&mut state, "--", 1, &source, &mut diagnostics);
        consume_claim_line(&mut state, "", 2, &source, &mut diagnostics);
        consume_claim_line(&mut state, "line one", 3, &source, &mut diagnostics);
        consume_claim_line(&mut state, "", 4, &source, &mut diagnostics);
        consume_claim_line(&mut state, "line two", 5, &source, &mut diagnostics);
        consume_claim_line(&mut state, "", 6, &source, &mut diagnostics);
        let outcome = consume_claim_line(&mut state, "::", 7, &source, &mut diagnostics);

        match outcome {
            ClaimLineOutcome::Closed(parsed) => {
                // Leading/trailing blank lines stripped; internal blank preserved.
                assert_eq!(parsed.body_text, "line one\n\nline two");
            }
            ClaimLineOutcome::Continue => panic!("expected Closed"),
        }
    }

    // Test: close fence in fields region (no body)
    #[test]
    fn consume_claim_line_closes_on_double_colon_in_fields_region() {
        let source = make_source("status: verified\n::\n");
        let mut state = fresh_state("billing.credits");
        let mut diagnostics = Vec::new();

        consume_claim_line(&mut state, "status: verified", 1, &source, &mut diagnostics);
        let outcome = consume_claim_line(&mut state, "::", 2, &source, &mut diagnostics);

        match outcome {
            ClaimLineOutcome::Closed(parsed) => {
                assert_eq!(parsed.body_text, "");
            }
            ClaimLineOutcome::Continue => panic!("expected Closed"),
        }
    }

    // Test: blank lines between fields are silently skipped
    #[test]
    fn consume_claim_line_skips_blank_lines_in_fields_region() {
        let source = make_source("status: verified\n\nowner: team-a\n");
        let mut state = fresh_state("billing.credits");
        let mut diagnostics = Vec::new();

        consume_claim_line(&mut state, "status: verified", 1, &source, &mut diagnostics);
        consume_claim_line(&mut state, "", 2, &source, &mut diagnostics);
        consume_claim_line(&mut state, "owner: team-a", 3, &source, &mut diagnostics);

        assert!(diagnostics.is_empty());
        assert_eq!(state.raw_fields.len(), 2);
    }

    // Test: finalize_unclosed_claim emits the right diagnostic
    #[test]
    fn finalize_unclosed_claim_emits_unclosed_fence_diagnostic() {
        let mut diagnostics = Vec::new();
        let state = fresh_state("billing.credits");
        finalize_unclosed_claim(state, &mut diagnostics);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::ParseUnclosedFence);
        assert!(diagnostics[0].message.contains("::claim"));
    }
}
