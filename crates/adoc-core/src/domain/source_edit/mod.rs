//! Formatting-preserving span splicing for patch apply (V6.4, ADR-0036).
//!
//! Pure byte math: a [`SourceEditPlan`] is a sorted, non-overlapping set of
//! [`SpanEdit`]s over a source string, and [`SourceEditPlan::splice`] copies
//! every byte outside the edited ranges verbatim — formatting preservation
//! holds by construction. All ranges are **byte** offsets (the
//! `SourcePosition.offset` convention); char-based parser columns must never
//! reach this module.

pub(crate) mod planner;

use std::fmt;
use std::ops::Range;

/// One replacement: the bytes in `byte_range` are replaced by `replacement`.
/// An insertion is an empty range (`start == end`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SpanEdit {
    pub(crate) byte_range: Range<usize>,
    pub(crate) replacement: String,
}

/// Why a plan could not be constructed or spliced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SourceEditError {
    /// Two edits overlap. Touching ranges (`a.end == b.start`) are legal.
    Overlap {
        first: Range<usize>,
        second: Range<usize>,
    },
    /// An edit range exceeds the source length.
    OutOfBounds { range: Range<usize>, len: usize },
    /// An edit boundary falls inside a multi-byte character.
    NotCharBoundary { offset: usize },
}

impl fmt::Display for SourceEditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Overlap { first, second } => write!(
                f,
                "edit ranges overlap: {}..{} and {}..{}",
                first.start, first.end, second.start, second.end
            ),
            Self::OutOfBounds { range, len } => write!(
                f,
                "edit range {}..{} exceeds source length {len}",
                range.start, range.end
            ),
            Self::NotCharBoundary { offset } => {
                write!(f, "edit boundary at byte {offset} is not a char boundary")
            }
        }
    }
}

/// A validated, sorted, non-overlapping set of edits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceEditPlan {
    edits: Vec<SpanEdit>,
}

impl SourceEditPlan {
    /// Sorts edits by range start and rejects overlap. Touching ranges
    /// (`a.end == b.start`) are legal; insertions at the same offset are an
    /// overlap (their relative order would be ambiguous).
    pub(crate) fn new(mut edits: Vec<SpanEdit>) -> Result<Self, SourceEditError> {
        edits.sort_by_key(|edit| (edit.byte_range.start, edit.byte_range.end));
        for pair in edits.windows(2) {
            let (first, second) = (&pair[0].byte_range, &pair[1].byte_range);
            let overlaps = if first.start == second.start {
                // Same start: two insertions or an insertion plus a
                // replacement at one offset — ambiguous, reject.
                true
            } else {
                first.end > second.start
            };
            if overlaps {
                return Err(SourceEditError::Overlap {
                    first: first.clone(),
                    second: second.clone(),
                });
            }
        }
        Ok(Self { edits })
    }

    #[cfg(test)]
    pub(crate) fn empty() -> Self {
        Self { edits: Vec::new() }
    }

    /// Apply the plan to `source`. Every byte outside the edited ranges is
    /// copied verbatim, by construction.
    pub(crate) fn splice(&self, source: &str) -> Result<String, SourceEditError> {
        for edit in &self.edits {
            let range = &edit.byte_range;
            if range.end > source.len() || range.start > range.end {
                return Err(SourceEditError::OutOfBounds {
                    range: range.clone(),
                    len: source.len(),
                });
            }
            for offset in [range.start, range.end] {
                if !source.is_char_boundary(offset) {
                    return Err(SourceEditError::NotCharBoundary { offset });
                }
            }
        }

        let mut output = String::with_capacity(source.len());
        let mut cursor = 0;
        for edit in &self.edits {
            output.push_str(&source[cursor..edit.byte_range.start]);
            output.push_str(&edit.replacement);
            cursor = edit.byte_range.end;
        }
        output.push_str(&source[cursor..]);
        Ok(output)
    }
}

/// Line-ending convention of a source file, detected from its first newline.
/// Parser spans never cover a `\r`, so in-range replacements preserve CRLF;
/// only text this module *synthesizes* (joined body lines, inserted field
/// lines) must match the file's convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineEnding {
    Lf,
    CrLf,
}

impl LineEnding {
    pub(crate) fn detect(text: &str) -> Self {
        match text.find('\n') {
            Some(index) if index > 0 && text.as_bytes()[index - 1] == b'\r' => Self::CrLf,
            _ => Self::Lf,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(range: Range<usize>, replacement: &str) -> SpanEdit {
        SpanEdit {
            byte_range: range,
            replacement: replacement.to_string(),
        }
    }

    const FIXTURES: &[&str] = &[
        "",
        "plain text, no newline",
        "lf line one\nlf line two\n",
        "crlf line one\r\ncrlf line two\r\n",
        "no trailing newline\nlast line",
        "multibyte: café 🦀 née\nsecond line\n",
    ];

    #[test]
    fn empty_plan_is_byte_identical_for_every_fixture() {
        for fixture in FIXTURES {
            let spliced = SourceEditPlan::empty().splice(fixture).expect("splices");
            assert_eq!(&spliced, fixture, "empty plan must be identity");
        }
    }

    #[test]
    fn new_rejects_overlapping_ranges() {
        let error = SourceEditPlan::new(vec![edit(0..5, "a"), edit(3..8, "b")])
            .expect_err("overlap must be rejected");
        assert_eq!(
            error,
            SourceEditError::Overlap {
                first: 0..5,
                second: 3..8,
            }
        );
    }

    #[test]
    fn new_rejects_same_offset_insertions() {
        let error = SourceEditPlan::new(vec![edit(4..4, "a"), edit(4..4, "b")])
            .expect_err("ambiguous same-offset insertions must be rejected");
        assert!(matches!(error, SourceEditError::Overlap { .. }));
    }

    #[test]
    fn new_accepts_touching_ranges_and_sorts() {
        let plan =
            SourceEditPlan::new(vec![edit(5..8, "B"), edit(0..5, "A")]).expect("touching is legal");
        let spliced = plan.splice("0123456789").expect("splices");
        assert_eq!(spliced, "AB89");
    }

    #[test]
    fn splice_rejects_out_of_bounds_range() {
        let plan = SourceEditPlan::new(vec![edit(0..99, "x")]).expect("plan builds");
        let error = plan.splice("short").expect_err("out of bounds");
        assert_eq!(
            error,
            SourceEditError::OutOfBounds {
                range: 0..99,
                len: 5,
            }
        );
    }

    #[test]
    fn splice_rejects_non_char_boundary() {
        // "é" is two bytes; offset 1 is inside it.
        let plan = SourceEditPlan::new(vec![edit(1..2, "x")]).expect("plan builds");
        let error = plan.splice("é").expect_err("boundary violation");
        assert_eq!(error, SourceEditError::NotCharBoundary { offset: 1 });
    }

    #[test]
    fn splice_preserves_every_byte_outside_edited_ranges_exhaustively() {
        // Hand-rolled property test: enumerate every valid (range, range)
        // pair over a small fixture and assert all bytes outside the ranges
        // are unchanged.
        let source = "abcdefgh";
        let len = source.len();
        for first_start in 0..=len {
            for first_end in first_start..=len {
                for second_start in first_end..=len {
                    for second_end in second_start..=len {
                        let edits = vec![
                            edit(first_start..first_end, "XX"),
                            edit(second_start..second_end, "Y"),
                        ];
                        let Ok(plan) = SourceEditPlan::new(edits) else {
                            // Same-offset insertion pairs are rejected; skip.
                            continue;
                        };
                        let spliced = plan.splice(source).expect("in-bounds splice");
                        let expected = format!(
                            "{}XX{}Y{}",
                            &source[..first_start],
                            &source[first_end..second_start],
                            &source[second_end..],
                        );
                        assert_eq!(spliced, expected);
                    }
                }
            }
        }
    }

    #[test]
    fn insertion_at_offset_inserts_without_consuming() {
        let plan = SourceEditPlan::new(vec![edit(3..3, "-INSERT-")]).expect("plan builds");
        assert_eq!(plan.splice("abcdef").expect("splices"), "abc-INSERT-def");
    }

    #[test]
    fn line_ending_detection() {
        assert_eq!(LineEnding::detect("a\nb"), LineEnding::Lf);
        assert_eq!(LineEnding::detect("a\r\nb"), LineEnding::CrLf);
        assert_eq!(LineEnding::detect("no newline"), LineEnding::Lf);
        assert_eq!(LineEnding::detect(""), LineEnding::Lf);
        assert_eq!(LineEnding::detect("\n"), LineEnding::Lf);
    }
}
