use crate::domain::ast::ParsedTypedBlock;
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, SourcePosition, SourceSpan};
use crate::domain::values::trim_ascii_edges;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DecodedListField {
    pub(super) value_span: SourceSpan,
    pub(super) segments: Vec<DecodedListSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DecodedListSegment {
    pub(super) value: Option<String>,
    pub(super) span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MalformedListField {
    span: SourceSpan,
}

impl MalformedListField {
    pub(super) fn into_diagnostic(self, parsed: &ParsedTypedBlock, field_name: &str) -> Diagnostic {
        Diagnostic::error(
            DiagnosticCode::IdInvalid,
            format!(
                "malformed relation array in `{field_name}` for `{}`",
                parsed.id_text
            ),
        )
        .with_span(self.span)
        .with_object_id(&parsed.id_text)
        .with_help(
            "Relation arrays must use `[object.id, other.id]`; each target must also be a valid Object ID.",
        )
    }
}

/// Remove and decode a scalar or bracket-list field. Segment spans are
/// calculated once in bytes and Unicode columns; a trailing empty segment is
/// omitted so the established trailing-comma tolerance remains intact.
pub(super) fn take_list_field(
    parsed: &mut ParsedTypedBlock,
    field_name: &str,
) -> Option<Result<DecodedListField, MalformedListField>> {
    let value = parsed.raw_fields.remove(field_name)?;
    let value_span = parsed
        .raw_field_spans
        .get(field_name)
        .cloned()
        .unwrap_or_else(|| parsed.span.clone());
    let Some((trimmed, trimmed_start, trimmed_end)) = trim_segment(&value) else {
        return Some(Ok(DecodedListField {
            value_span,
            segments: Vec::new(),
        }));
    };

    let (content_start, content_end) = match (trimmed.strip_prefix('['), trimmed.strip_suffix(']'))
    {
        (Some(_), Some(_)) => (trimmed_start + 1, trimmed_end - 1),
        (Some(_), None) | (None, Some(_)) => {
            return Some(Err(MalformedListField {
                span: segment_span(&value_span, &value, trimmed_start, trimmed_end),
            }));
        }
        (None, None) => (0, value.len()),
    };

    let content = &value[content_start..content_end];
    if trim_segment(content).is_none() {
        return Some(Ok(DecodedListField {
            value_span,
            segments: Vec::new(),
        }));
    }

    let mut ranges = Vec::new();
    let mut segment_start = content_start;
    for (relative_comma, _) in content.match_indices(',') {
        let comma = content_start + relative_comma;
        ranges.push((segment_start, comma, false));
        segment_start = comma + 1;
    }
    ranges.push((segment_start, content_end, true));

    let segments = ranges
        .into_iter()
        .filter_map(|(start, end, is_last)| {
            let raw = &value[start..end];
            match trim_segment(raw) {
                Some((trimmed, trimmed_start, trimmed_end)) => Some(DecodedListSegment {
                    value: Some(trimmed.to_string()),
                    span: segment_span(
                        &value_span,
                        &value,
                        start + trimmed_start,
                        start + trimmed_end,
                    ),
                }),
                None if is_last => None,
                None => Some(DecodedListSegment {
                    value: None,
                    span: segment_span(&value_span, &value, start, end),
                }),
            }
        })
        .collect();

    Some(Ok(DecodedListField {
        value_span,
        segments,
    }))
}

/// Remove a scalar field, trim ASCII edges, and treat blank as absent.
pub(crate) fn take_optional_scalar<T, E>(
    parsed: &mut ParsedTypedBlock,
    field_name: &str,
    ctor: impl FnOnce(&str) -> Result<T, E>,
) -> Result<Option<T>, E> {
    let Some(raw) = parsed.raw_fields.remove(field_name) else {
        return Ok(None);
    };
    let trimmed = trim_ascii_edges(&raw);
    if trimmed.is_empty() {
        return Ok(None);
    }
    ctor(trimmed).map(Some)
}

pub(crate) fn take_required_scalar<T, E>(
    parsed: &mut ParsedTypedBlock,
    field_name: &str,
    ctor: impl FnOnce(&str) -> Result<T, E>,
    missing: impl FnOnce() -> E,
) -> Result<T, E> {
    take_optional_scalar(parsed, field_name, ctor)?.ok_or_else(missing)
}

pub(crate) fn take_scalar_text(parsed: &mut ParsedTypedBlock, field_name: &str) -> Option<String> {
    let raw = parsed.raw_fields.remove(field_name)?;
    let trimmed = trim_ascii_edges(&raw);
    (!trimmed.is_empty()).then(|| trimmed.to_string())
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

fn segment_span(value_span: &SourceSpan, value: &str, start: usize, end: usize) -> SourceSpan {
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use super::*;

    fn span() -> SourceSpan {
        SourceSpan {
            file: PathBuf::from("docs/test.adoc"),
            start: SourcePosition {
                line: 3,
                column: 10,
                offset: 20,
            },
            end: SourcePosition {
                line: 3,
                column: 40,
                offset: 50,
            },
        }
    }

    fn parsed(value: &str) -> ParsedTypedBlock {
        ParsedTypedBlock {
            kind_word: "claim".to_string(),
            kind_word_span: span(),
            id_text: "billing.credits".to_string(),
            raw_fields: BTreeMap::from([("items".to_string(), value.to_string())]),
            raw_field_spans: BTreeMap::from([("items".to_string(), span())]),
            duplicate_keys: Vec::new(),
            body_text: "body".to_string(),
            body_inlines: ParsedTypedBlock::test_body_inlines_from_text("body"),
            body_spans: Vec::new(),
            content_spans: Vec::new(),
            span: span(),
            close_fence_span: span(),
            body_separator_span: None,
        }
    }

    #[test]
    fn decodes_scalar_and_bracket_lists_with_trailing_comma_tolerance() {
        let mut scalar = parsed("alpha");
        let scalar = take_list_field(&mut scalar, "items")
            .expect("field")
            .expect("valid");
        assert_eq!(scalar.segments[0].value.as_deref(), Some("alpha"));

        let mut bracket = parsed("[alpha, beta,]");
        let bracket = take_list_field(&mut bracket, "items")
            .expect("field")
            .expect("valid");
        assert_eq!(bracket.segments.len(), 2);
        assert_eq!(bracket.segments[1].value.as_deref(), Some("beta"));
    }

    #[test]
    fn preserves_empty_middle_segments_and_unicode_aware_columns() {
        let mut parsed = parsed("[één,, two]");
        let decoded = take_list_field(&mut parsed, "items")
            .expect("field")
            .expect("valid");

        assert_eq!(decoded.segments.len(), 3);
        assert!(decoded.segments[1].value.is_none());
        assert_eq!(decoded.segments[2].span.start.column, 17);
        assert_eq!(decoded.segments[2].span.start.offset, 29);
    }

    #[test]
    fn returns_typed_malformed_bracket_failure() {
        let mut parsed = parsed("[alpha, beta");
        let error = take_list_field(&mut parsed, "items")
            .expect("field")
            .expect_err("malformed");
        let diagnostic = error.into_diagnostic(&parsed, "items");

        assert_eq!(diagnostic.code, DiagnosticCode::IdInvalid);
        assert!(diagnostic.message.contains("malformed relation array"));
    }
}
