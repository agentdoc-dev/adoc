//! Op-specific edit planners over a [`TypedBlockLayout`] (V6.4, ADR-0036).
//!
//! Pure byte math: layouts are built from fresh parser spans by
//! `infrastructure::parser::layout`; planners turn a patch intent into a
//! [`SourceEditPlan`] without touching the filesystem or the parser. Refusals
//! come back as fix-oriented diagnostics, never panics.

use std::collections::BTreeMap;
use std::ops::Range;

use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};

use super::{LineEnding, SourceEditPlan, SpanEdit};

/// Byte-range view of one typed block inside its source file, built from a
/// fresh parse. Ranges never include line endings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TypedBlockLayout {
    /// The `::kind id` open-fence line.
    pub(crate) open_fence: Range<usize>,
    /// Field **value** ranges keyed by field key (zero-width for `key:`).
    pub(crate) field_values: BTreeMap<String, Range<usize>>,
    /// Keys the parser saw more than once; edits targeting them are refused.
    pub(crate) duplicate_keys: Vec<String>,
    /// Blank-edge-trimmed body lines, one range per line.
    pub(crate) body_lines: Vec<Range<usize>>,
    /// The `--` separator line, when the block has one.
    pub(crate) body_separator: Option<Range<usize>>,
    /// The closing `::` fence line.
    pub(crate) close_fence: Range<usize>,
}

/// Plan a `replace_body`: replace only the region between `--` and the
/// closing `::`, inserting the separator when the block has none.
pub(crate) fn plan_replace_body(
    source: &str,
    layout: &TypedBlockLayout,
    new_body: &str,
) -> Result<SourceEditPlan, Vec<Diagnostic>> {
    let diagnostics = guard_body_lines(new_body);
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let eol = LineEnding::detect(source);
    let joined = join_lines(new_body, eol);

    let edit = match (layout.body_lines.first(), layout.body_lines.last()) {
        (Some(first), Some(last)) => SpanEdit {
            byte_range: first.start..last.end,
            replacement: joined,
        },
        _ => match &layout.body_separator {
            // Separator present, body empty: insert on the line after `--`.
            Some(separator) => SpanEdit {
                byte_range: insertion_offset_after_line(source, separator.end),
                replacement: format!("{joined}{}", eol.as_str()),
            },
            // No separator at all: insert `--` + body before the close fence.
            None => SpanEdit {
                byte_range: layout.close_fence.start..layout.close_fence.start,
                replacement: format!("--{eol}{joined}{eol}", eol = eol.as_str()),
            },
        },
    };

    SourceEditPlan::new(vec![edit]).map_err(|error| vec![internal_plan_error(error)])
}

/// Plan an `update_fields`: rewrite only the targeted field-value spans; new
/// keys insert `key: value` lines after the last existing field line (after
/// the open fence when the block has none).
pub(crate) fn plan_update_fields(
    source: &str,
    layout: &TypedBlockLayout,
    fields: &BTreeMap<String, String>,
) -> Result<SourceEditPlan, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut edits = Vec::new();
    let mut new_lines: Vec<String> = Vec::new();
    let eol = LineEnding::detect(source);

    for (key, value) in fields {
        if value.contains('\n') || value.contains('\r') {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::PatchValidationFailed,
                format!("field `{key}` value contains a line break and cannot be applied"),
            ));
            continue;
        }
        if layout.duplicate_keys.iter().any(|duplicate| duplicate == key) {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::PatchValidationFailed,
                format!(
                    "field `{key}` appears more than once in the source block; \
                     fix the duplicate before applying"
                ),
            ));
            continue;
        }
        match layout.field_values.get(key) {
            Some(range) => {
                // `key:` with no value has a zero-width span at end of line;
                // restore the separating space the parser stripped.
                let needs_space = range.start == range.end
                    && source.as_bytes().get(range.start.wrapping_sub(1)) == Some(&b':');
                let replacement = if needs_space {
                    format!(" {value}")
                } else {
                    value.clone()
                };
                edits.push(SpanEdit {
                    byte_range: range.clone(),
                    replacement,
                });
            }
            None => {
                if !is_field_key(key) {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticCode::PatchValidationFailed,
                        format!(
                            "new field key `{key}` does not match [a-z][a-z0-9_]* \
                             and would not reparse as a field line"
                        ),
                    ));
                    continue;
                }
                new_lines.push(format!("{key}: {value}"));
            }
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    if !new_lines.is_empty() {
        // One insertion for all new keys (BTreeMap order keeps it sorted and
        // deterministic), at the end of the last field line — or the open
        // fence when the block has no fields.
        let anchor_end = layout
            .field_values
            .values()
            .map(|range| range.end)
            .max()
            .unwrap_or(layout.open_fence.end);
        let offset = insertion_offset_at_end_of_line(source, anchor_end);
        let mut replacement = String::new();
        for line in &new_lines {
            replacement.push_str(eol.as_str());
            replacement.push_str(line);
        }
        edits.push(SpanEdit {
            byte_range: offset..offset,
            replacement,
        });
    }

    SourceEditPlan::new(edits).map_err(|error| vec![internal_plan_error(error)])
}

/// Reject body lines that would re-fence the block on reparse: a bare `::`
/// closes the fence early, and a grammar-valid open-fence shape becomes a
/// nested-block error.
fn guard_body_lines(body: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for line in body.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line == "::" {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::PatchValidationFailed,
                "body contains a bare `::` line, which would close the typed-block fence early",
            ));
        } else if looks_like_open_fence(line) {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::PatchValidationFailed,
                format!("body line {line:?} would parse as a nested typed-block opener"),
            ));
        }
    }
    diagnostics
}

/// Pure mirror of the parser's open-fence detector
/// (`infrastructure::parser::typed_block::looks_like_open_fence`): a spliced
/// file must reparse to exactly the intended block, so the planner refuses
/// body text the parser would treat as an opener.
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
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_lowercase()
        && chars.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

fn is_field_key(key: &str) -> bool {
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_lowercase()
        && chars.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

/// Join patch body text (LF-normalised JSON string) into the file's
/// line-ending convention.
fn join_lines(body: &str, eol: LineEnding) -> String {
    body.split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect::<Vec<_>>()
        .join(eol.as_str())
}

/// Insertion point at the start of the line following the line that ends at
/// `line_content_end` (a span end, which never covers the EOL bytes).
fn insertion_offset_after_line(source: &str, line_content_end: usize) -> Range<usize> {
    let rest = &source[line_content_end..];
    let offset = match rest.find('\n') {
        Some(newline) => line_content_end + newline + 1,
        None => source.len(),
    };
    offset..offset
}

/// Insertion point at the end of the line containing `span_end` — before the
/// line's EOL bytes (handles CRLF), or end-of-file when the line is last and
/// unterminated.
fn insertion_offset_at_end_of_line(source: &str, span_end: usize) -> usize {
    let rest = &source[span_end..];
    match rest.find('\n') {
        Some(newline) => {
            let absolute = span_end + newline;
            if absolute > 0 && source.as_bytes()[absolute - 1] == b'\r' {
                absolute - 1
            } else {
                absolute
            }
        }
        None => source.len(),
    }
}

fn internal_plan_error(error: super::SourceEditError) -> Diagnostic {
    Diagnostic::error(
        DiagnosticCode::PatchValidationFailed,
        format!("internal edit-plan construction failed: {error}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fixture: a claim with two fields and a two-line body.
    const SOURCE: &str = "\
# Page

::claim billing.credits
status: verified
owner: team-billing
--
Body line one.
Body line two.
::

After text.
";

    fn range_of(haystack: &str, needle: &str) -> Range<usize> {
        let start = haystack.find(needle).expect("needle present");
        start..start + needle.len()
    }

    fn layout_for_fixture(source: &str) -> TypedBlockLayout {
        let status_value = range_of(source, "verified");
        let owner_value = range_of(source, "team-billing");
        TypedBlockLayout {
            open_fence: range_of(source, "::claim billing.credits"),
            field_values: BTreeMap::from([
                ("status".to_string(), status_value),
                ("owner".to_string(), owner_value),
            ]),
            duplicate_keys: Vec::new(),
            body_lines: vec![
                range_of(source, "Body line one."),
                range_of(source, "Body line two."),
            ],
            body_separator: Some(range_of(source, "--")),
            close_fence: {
                let start = source.rfind("\n::\n").expect("close fence") + 1;
                start..start + 2
            },
        }
    }

    #[test]
    fn replace_body_rewrites_exactly_the_body_region() {
        let layout = layout_for_fixture(SOURCE);
        let plan =
            plan_replace_body(SOURCE, &layout, "New body.\nSecond new line.").expect("plans");
        let spliced = plan.splice(SOURCE).expect("splices");
        assert_eq!(
            spliced,
            SOURCE.replace(
                "Body line one.\nBody line two.",
                "New body.\nSecond new line."
            )
        );
    }

    #[test]
    fn replace_body_with_separator_but_empty_body_inserts_after_separator() {
        let source = "::claim a.b\nstatus: draft\n--\n::\n";
        let layout = TypedBlockLayout {
            open_fence: range_of(source, "::claim a.b"),
            field_values: BTreeMap::from([("status".to_string(), range_of(source, "draft"))]),
            duplicate_keys: Vec::new(),
            body_lines: Vec::new(),
            body_separator: Some(range_of(source, "--")),
            close_fence: {
                let start = source.rfind("::").expect("close fence");
                start..start + 2
            },
        };
        let plan = plan_replace_body(source, &layout, "Inserted body.").expect("plans");
        let spliced = plan.splice(source).expect("splices");
        assert_eq!(spliced, "::claim a.b\nstatus: draft\n--\nInserted body.\n::\n");
    }

    #[test]
    fn replace_body_without_separator_inserts_separator_and_body_before_close_fence() {
        let source = "::claim a.b\nstatus: draft\n::\n";
        let layout = TypedBlockLayout {
            open_fence: range_of(source, "::claim a.b"),
            field_values: BTreeMap::from([("status".to_string(), range_of(source, "draft"))]),
            duplicate_keys: Vec::new(),
            body_lines: Vec::new(),
            body_separator: None,
            close_fence: {
                let start = source.rfind("::").expect("close fence");
                start..start + 2
            },
        };
        let plan = plan_replace_body(source, &layout, "Added body.").expect("plans");
        let spliced = plan.splice(source).expect("splices");
        assert_eq!(spliced, "::claim a.b\nstatus: draft\n--\nAdded body.\n::\n");
    }

    #[test]
    fn replace_body_preserves_crlf_line_endings_in_synthesized_text() {
        let source = "::claim a.b\r\nstatus: draft\r\n--\r\nOld body.\r\n::\r\n";
        let layout = TypedBlockLayout {
            open_fence: range_of(source, "::claim a.b"),
            field_values: BTreeMap::from([("status".to_string(), range_of(source, "draft"))]),
            duplicate_keys: Vec::new(),
            body_lines: vec![range_of(source, "Old body.")],
            body_separator: Some(range_of(source, "--")),
            close_fence: {
                let start = source.rfind("::").expect("close fence");
                start..start + 2
            },
        };
        let plan = plan_replace_body(source, &layout, "New one.\nNew two.").expect("plans");
        let spliced = plan.splice(source).expect("splices");
        assert_eq!(
            spliced,
            "::claim a.b\r\nstatus: draft\r\n--\r\nNew one.\r\nNew two.\r\n::\r\n"
        );
    }

    #[test]
    fn replace_body_rejects_bare_close_fence_line_in_body() {
        let layout = layout_for_fixture(SOURCE);
        let diagnostics =
            plan_replace_body(SOURCE, &layout, "ok\n::\nmore").expect_err("must refuse");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::PatchValidationFailed);
        assert!(diagnostics[0].message.contains("close the typed-block fence"));
    }

    #[test]
    fn replace_body_rejects_nested_opener_shape_in_body() {
        let layout = layout_for_fixture(SOURCE);
        let diagnostics =
            plan_replace_body(SOURCE, &layout, "::warning auth.x").expect_err("must refuse");
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("nested typed-block opener"));
    }

    #[test]
    fn replace_body_allows_double_dash_and_grammar_invalid_fence_shapes() {
        let layout = layout_for_fixture(SOURCE);
        // `--` is plain body text in body phase; `::Fact-Cap` is grammar-invalid.
        let plan = plan_replace_body(SOURCE, &layout, "--\n::Fact-Cap x\n  ::").expect("plans");
        let spliced = plan.splice(SOURCE).expect("splices");
        assert!(spliced.contains("--\n::Fact-Cap x\n  ::\n::"));
    }

    #[test]
    fn update_fields_rewrites_only_the_targeted_value_span() {
        let layout = layout_for_fixture(SOURCE);
        let fields = BTreeMap::from([("status".to_string(), "deprecated".to_string())]);
        let plan = plan_update_fields(SOURCE, &layout, &fields).expect("plans");
        let spliced = plan.splice(SOURCE).expect("splices");
        assert_eq!(spliced, SOURCE.replace("verified", "deprecated"));
    }

    #[test]
    fn update_fields_restores_separating_space_for_empty_authored_value() {
        let source = "::claim a.b\nowner:\nstatus: draft\n::\n";
        let owner_end = source.find("owner:").expect("owner line") + "owner:".len();
        let layout = TypedBlockLayout {
            open_fence: range_of(source, "::claim a.b"),
            field_values: BTreeMap::from([
                ("owner".to_string(), owner_end..owner_end),
                ("status".to_string(), range_of(source, "draft")),
            ]),
            duplicate_keys: Vec::new(),
            body_lines: Vec::new(),
            body_separator: None,
            close_fence: {
                let start = source.rfind("::").expect("close fence");
                start..start + 2
            },
        };
        let fields = BTreeMap::from([("owner".to_string(), "team-a".to_string())]);
        let plan = plan_update_fields(source, &layout, &fields).expect("plans");
        let spliced = plan.splice(source).expect("splices");
        assert_eq!(spliced, "::claim a.b\nowner: team-a\nstatus: draft\n::\n");
    }

    #[test]
    fn update_fields_inserts_new_keys_after_last_field_line_in_one_edit() {
        let layout = layout_for_fixture(SOURCE);
        let fields = BTreeMap::from([
            ("expires_at".to_string(), "2027-01-01".to_string()),
            ("verified_at".to_string(), "2026-06-12".to_string()),
        ]);
        let plan = plan_update_fields(SOURCE, &layout, &fields).expect("plans");
        let spliced = plan.splice(SOURCE).expect("splices");
        assert_eq!(
            spliced,
            SOURCE.replace(
                "owner: team-billing\n",
                "owner: team-billing\nexpires_at: 2027-01-01\nverified_at: 2026-06-12\n"
            )
        );
    }

    #[test]
    fn update_fields_inserts_after_open_fence_when_block_has_no_fields() {
        let source = "::claim a.b\n--\nBody.\n::\n";
        let layout = TypedBlockLayout {
            open_fence: range_of(source, "::claim a.b"),
            field_values: BTreeMap::new(),
            duplicate_keys: Vec::new(),
            body_lines: vec![range_of(source, "Body.")],
            body_separator: Some(range_of(source, "--")),
            close_fence: {
                let start = source.rfind("::").expect("close fence");
                start..start + 2
            },
        };
        let fields = BTreeMap::from([("status".to_string(), "draft".to_string())]);
        let plan = plan_update_fields(source, &layout, &fields).expect("plans");
        let spliced = plan.splice(source).expect("splices");
        assert_eq!(spliced, "::claim a.b\nstatus: draft\n--\nBody.\n::\n");
    }

    #[test]
    fn update_fields_mixes_existing_rewrite_and_new_key_insert() {
        let layout = layout_for_fixture(SOURCE);
        let fields = BTreeMap::from([
            ("status".to_string(), "deprecated".to_string()),
            ("expires_at".to_string(), "2027-01-01".to_string()),
        ]);
        let plan = plan_update_fields(SOURCE, &layout, &fields).expect("plans");
        let spliced = plan.splice(SOURCE).expect("splices");
        assert_eq!(
            spliced,
            SOURCE
                .replace("verified", "deprecated")
                .replace(
                    "owner: team-billing\n",
                    "owner: team-billing\nexpires_at: 2027-01-01\n"
                )
        );
    }

    #[test]
    fn update_fields_rejects_value_with_line_break() {
        let layout = layout_for_fixture(SOURCE);
        let fields = BTreeMap::from([("status".to_string(), "a\nb".to_string())]);
        let diagnostics = plan_update_fields(SOURCE, &layout, &fields).expect_err("must refuse");
        assert!(diagnostics[0].message.contains("line break"));
    }

    #[test]
    fn update_fields_rejects_duplicate_source_key() {
        let mut layout = layout_for_fixture(SOURCE);
        layout.duplicate_keys.push("status".to_string());
        let fields = BTreeMap::from([("status".to_string(), "draft".to_string())]);
        let diagnostics = plan_update_fields(SOURCE, &layout, &fields).expect_err("must refuse");
        assert!(diagnostics[0].message.contains("more than once"));
    }

    #[test]
    fn update_fields_rejects_grammar_invalid_new_key() {
        let layout = layout_for_fixture(SOURCE);
        let fields = BTreeMap::from([("Bad-Key".to_string(), "x".to_string())]);
        let diagnostics = plan_update_fields(SOURCE, &layout, &fields).expect_err("must refuse");
        assert!(diagnostics[0].message.contains("would not reparse"));
    }

    #[test]
    fn update_fields_multibyte_value_replacement_stays_on_char_boundaries() {
        let source = "::claim a.b\nowner: caf\u{e9}-team\nstatus: draft\n::\n";
        let layout = TypedBlockLayout {
            open_fence: range_of(source, "::claim a.b"),
            field_values: BTreeMap::from([
                ("owner".to_string(), range_of(source, "caf\u{e9}-team")),
                ("status".to_string(), range_of(source, "draft")),
            ]),
            duplicate_keys: Vec::new(),
            body_lines: Vec::new(),
            body_separator: None,
            close_fence: {
                let start = source.rfind("::").expect("close fence");
                start..start + 2
            },
        };
        let fields = BTreeMap::from([("owner".to_string(), "\u{1f980}-crew".to_string())]);
        let plan = plan_update_fields(source, &layout, &fields).expect("plans");
        let spliced = plan.splice(source).expect("splices");
        assert_eq!(spliced, "::claim a.b\nowner: \u{1f980}-crew\nstatus: draft\n::\n");
    }
}
