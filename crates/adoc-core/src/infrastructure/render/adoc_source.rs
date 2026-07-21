//! V8.1.1 prose serializer: `PageAst` (parsed from Markdown) → canonical
//! strict-mode `.adoc` source text (ADR-0043).
//!
//! A different concern from the ADR-0036 span-splice patch writer in
//! `domain/source_edit/`, which edits existing sources — this module
//! generates a fresh file. Losslessness is graph-semantic: every block must
//! re-compile to a graph node content-equal to the Markdown compile.
//!
//! Blocks the strict grammar cannot carry are *quarantined*: preserved
//! verbatim inside a fenced code block (the only strict-legal verbatim
//! carrier — strict rejects raw HTML and unsafe link schemes as ERRORs),
//! each backed 1:1 by a `migrate.*` WARNING. The quarantine predicate for
//! prose blocks is "the strict validators reject this serialization",
//! checked by re-running the strict parser and source rules on each
//! fragment — never a hand-maintained approximation of strict's rules.

use std::path::PathBuf;

use crate::domain::ast::{BlockAst, HeadingAst, ListAst, ListKind, PageAst, UnknownExtensionKind};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, Severity, SourceSpan};
use crate::domain::inline::to_source;
use crate::domain::source::SourceFile;
use crate::infrastructure::parser::parse_page;
use crate::infrastructure::validate::validate_source_page;

/// The reconciliation anchor: every quarantine diagnostic message contains
/// this phrase, so a report (or the losslessness test) can count kind
/// changes 1:1 against diagnostics. Drop diagnostics never contain it.
pub(crate) const QUARANTINE_PHRASE: &str = "preserved verbatim in a fenced code block";

/// Serialize a Markdown-parsed page to strict `.adoc` source text.
///
/// Returns the canonical text, the number of prose blocks it carries (the
/// serialized fragment count — quarantine fence bodies may contain blank
/// lines, so the text itself cannot be re-split to count blocks), and the
/// `migrate.*` diagnostics the serialization produced (quarantines, drops,
/// and unrepresentable-content errors). An ERROR diagnostic means the output
/// is not safe to write; the orchestrator refuses the run.
pub(crate) fn page_to_adoc_source(
    page: &PageAst,
    source: &SourceFile,
) -> (String, usize, Vec<Diagnostic>) {
    let mut serializer = Serializer {
        source,
        fragments: Vec::new(),
        diagnostics: Vec::new(),
    };
    for block in &page.blocks {
        serializer.push_block(block);
    }
    let prose_blocks = serializer.fragments.len();
    let mut text = serializer.fragments.join("\n\n");
    text.push('\n');
    (text, prose_blocks, serializer.diagnostics)
}

struct Serializer<'a> {
    source: &'a SourceFile,
    fragments: Vec<String>,
    diagnostics: Vec<Diagnostic>,
}

impl Serializer<'_> {
    fn push_block(&mut self, block: &BlockAst) {
        match block {
            BlockAst::Heading(heading) => self.push_heading(heading),
            BlockAst::Paragraph(paragraph) => {
                let text = to_source(&paragraph.inlines);
                if text.trim().is_empty() {
                    // Parser artifact (e.g. the trailing line of a math
                    // fence): zero content, and blank source cannot produce a
                    // graph node in strict mode. Dropped, said out loud.
                    self.diagnostics.push(
                        Diagnostic::warning(
                            DiagnosticCode::MigrateUnrecognizedExtension,
                            format!(
                                "empty prose block dropped from {}; it carries no content \
                                 and has no strict .adoc form",
                                self.source.physical_path.display()
                            ),
                        )
                        .with_span(paragraph.span.clone()),
                    );
                } else if text.contains('\n') {
                    self.quarantine(
                        &text,
                        "markdown",
                        "paragraph with a hard line break",
                        DiagnosticCode::MigrateUnrecognizedExtension,
                        &paragraph.span,
                    );
                } else {
                    self.push_prose(text, &paragraph.span, "paragraph");
                }
            }
            BlockAst::List(list) => self.push_list(list),
            BlockAst::CodeBlock(code_block) => {
                let language = code_block.language.as_deref().unwrap_or("");
                // The parser stores code with a trailing newline per line;
                // strip exactly one so the closing fence does not add a blank
                // line (a deliberate trailing blank line is preserved).
                let body = code_block
                    .code
                    .strip_suffix('\n')
                    .unwrap_or(&code_block.code);
                self.push_fence(language, body, &code_block.span);
            }
            BlockAst::QuarantinedHtml(html) => self.quarantine(
                &html.source_text,
                "html",
                "raw HTML block",
                DiagnosticCode::MigrateRawHtmlQuarantined,
                &html.span,
            ),
            BlockAst::Table(table) => self.quarantine(
                &table.source_text,
                "markdown",
                "GFM table",
                DiagnosticCode::MigrateUnrecognizedExtension,
                &table.span,
            ),
            BlockAst::FootnoteDefinition(footnote) => self.quarantine(
                &footnote.source_text,
                "markdown",
                "footnote definition",
                DiagnosticCode::MigrateUnrecognizedExtension,
                &footnote.span,
            ),
            BlockAst::UnknownExtension(extension) => self.quarantine(
                &extension.source_text,
                "markdown",
                extension_construct_name(extension.kind),
                DiagnosticCode::MigrateUnrecognizedExtension,
                &extension.span,
            ),
            BlockAst::ThematicBreak(thematic_break) => self.push_prose(
                thematic_break.source_text.trim_end().to_string(),
                &thematic_break.span,
                "thematic break",
            ),
            BlockAst::KnowledgeObject(_) | BlockAst::KnowledgeObjectPending(_) => {
                // Markdown parsing never produces typed blocks (ADR-0023); if
                // one appears the invariant is broken upstream. Surface loudly
                // instead of panicking — the ERROR refuses the run.
                self.diagnostics.push(Diagnostic::error(
                    DiagnosticCode::MigrateUnrecognizedExtension,
                    format!(
                        "internal: typed Knowledge Object block in Markdown source {}; \
                         Compatibility Mode parsing must never produce one (ADR-0023)",
                        self.source.physical_path.display()
                    ),
                ));
            }
        }
    }

    fn push_heading(&mut self, heading: &HeadingAst) {
        let text = to_source(&heading.inlines);
        let fragment = format!("{} {}", "#".repeat(usize::from(heading.level)), text);
        // The quarantine payload is the heading *text*, not the marked-up
        // fragment: the graph projects heading content without its marker, and
        // the payload must stay content-equal to the Markdown graph node.
        match self.strict_rejection(&fragment, &heading.span) {
            None => self.fragments.push(fragment),
            Some(code) => {
                self.quarantine_for_strict_rejection(&text, "heading", code, &heading.span)
            }
        }
    }

    fn push_list(&mut self, list: &ListAst) {
        if list.items.iter().any(|item| !item.content.is_empty()) {
            // Loose or nested list: the strict grammar has flat, tight lists
            // only. Preserve the original source slice verbatim.
            let payload = self.slice_for(&list.span);
            self.quarantine(
                &payload,
                "markdown",
                "loose or nested list",
                DiagnosticCode::MigrateUnrecognizedExtension,
                &list.span,
            );
            return;
        }

        let mut lines = Vec::with_capacity(list.items.len());
        for (index, item) in list.items.iter().enumerate() {
            if item.task_state.is_some() {
                self.diagnostics.push(
                    Diagnostic::warning(
                        DiagnosticCode::MigrateUnrecognizedExtension,
                        format!(
                            "task-list checkbox marker dropped from a list item in {}; \
                             the item text is preserved",
                            self.source.physical_path.display()
                        ),
                    )
                    .with_span(item.span.clone()),
                );
            }
            let marker = match list.kind {
                ListKind::Unordered => "- ".to_string(),
                ListKind::Ordered => format!("{}. ", index + 1),
            };
            lines.push(format!("{marker}{}", to_source(&item.inlines)));
        }
        let fragment = lines.join("\n");
        match self.strict_rejection(&fragment, &list.span) {
            None => self.fragments.push(fragment),
            Some(code) => self.quarantine_for_strict_rejection(&fragment, "list", code, &list.span),
        }
    }

    fn push_prose(&mut self, text: String, span: &SourceSpan, construct: &str) {
        match self.strict_rejection(&text, span) {
            None => self.fragments.push(text),
            Some(code) => self.quarantine_for_strict_rejection(&text, construct, code, span),
        }
    }

    /// The post-check rule (ADR-0043 §2): re-validate the serialized fragment
    /// with the strict parser and source rules themselves, so the quarantine
    /// predicate can never drift from what `adoc build` actually rejects.
    /// Returns the first strict ERROR code, or `None` when the fragment is
    /// legal strict prose.
    ///
    /// Non-error (WARNING/INFO) strict diagnostics are forwarded into the
    /// serializer's diagnostics — and so into the `adoc.migrate.report.v0`
    /// envelope — re-attributed from the synthetic `migrate/recheck.adoc`
    /// path to the real source block span.
    ///
    /// Single-code by design: when a fragment triggers several strict ERROR
    /// rules, the first code wins (parse diagnostics drain before source-rule
    /// diagnostics) and the quarantine is attributed to that one `migrate.*`
    /// bucket. The report's reconciliation invariant is unaffected — each
    /// count equals the tally of its emitted code — but a mixed-cause
    /// fragment reports as a single-cause quarantine; this is intent, not a
    /// counting bug.
    ///
    /// ponytail: no strict rule emits a non-error severity today, so the
    /// forwarding path is exercised only at its unit-test seam until one
    /// does. And one throwaway parse per prose block is O(blocks); switch to
    /// a single whole-page validation with span mapping only if migrating
    /// large trees measures slow.
    fn strict_rejection(&mut self, fragment: &str, span: &SourceSpan) -> Option<DiagnosticCode> {
        let mut text = fragment.to_string();
        text.push('\n');
        let fragment_source = SourceFile::new_with_identity_path(
            PathBuf::from("migrate/recheck.adoc"),
            text,
            PathBuf::from("migrate/recheck.adoc"),
        );
        let (page, parse_diagnostics) = parse_page(&fragment_source);
        let mut rejection = None;
        for diagnostic in parse_diagnostics
            .into_iter()
            .chain(validate_source_page(&page, &fragment_source))
        {
            if diagnostic.severity == Severity::Error {
                rejection.get_or_insert(diagnostic.code);
            } else {
                self.diagnostics
                    .push(reattribute_to_source(diagnostic, span));
            }
        }
        rejection
    }

    fn quarantine_for_strict_rejection(
        &mut self,
        payload: &str,
        construct: &str,
        strict_code: DiagnosticCode,
        span: &SourceSpan,
    ) {
        let migrate_code = match strict_code {
            DiagnosticCode::ParseRawHtml => DiagnosticCode::MigrateRawHtmlQuarantined,
            DiagnosticCode::ParseUnsafeLink => DiagnosticCode::MigrateBrokenLink,
            _ => DiagnosticCode::MigrateUnrecognizedExtension,
        };
        self.quarantine(
            payload,
            "markdown",
            &format!("{construct} strict mode rejects ({})", strict_code.as_str()),
            migrate_code,
            span,
        );
    }

    fn quarantine(
        &mut self,
        payload: &str,
        language: &str,
        construct: &str,
        code: DiagnosticCode,
        span: &SourceSpan,
    ) {
        let body = payload.trim_end_matches('\n');
        if !self.push_fence(language, body, span) {
            return;
        }
        self.diagnostics.push(
            Diagnostic::warning(
                code,
                format!(
                    "{construct} {QUARANTINE_PHRASE} in {}",
                    self.source.physical_path.display()
                ),
            )
            .with_span(span.clone()),
        );
    }

    /// Emit a fenced code block. The strict parser closes a fence only on a
    /// line that trims to exactly ```` ``` ```` and supports no longer fence
    /// markers, so a payload containing such a line is unrepresentable —
    /// surfaced as an ERROR that refuses the run rather than writing a file
    /// that would re-parse differently.
    fn push_fence(&mut self, language: &str, body: &str, span: &SourceSpan) -> bool {
        if body.lines().any(|line| line.trim() == "```") {
            self.diagnostics.push(
                Diagnostic::error(
                    DiagnosticCode::MigrateUnrecognizedExtension,
                    format!(
                        "cannot represent content containing a bare ``` fence line in strict \
                         .adoc ({}); resolve the nested fence by hand before migrating",
                        self.source.physical_path.display()
                    ),
                )
                .with_span(span.clone()),
            );
            return false;
        }
        let mut fragment = format!("```{language}\n");
        if !body.is_empty() {
            fragment.push_str(body);
            fragment.push('\n');
        }
        fragment.push_str("```");
        self.fragments.push(fragment);
        true
    }

    fn slice_for(&self, span: &SourceSpan) -> String {
        let start = span.start.offset as usize;
        let end = (span.end.offset as usize).min(self.source.text.len());
        if start >= end {
            return String::new();
        }
        self.source.text[start..end].to_string()
    }
}

/// Re-attribute a strict re-check diagnostic from the synthetic
/// `migrate/recheck.adoc` fragment to the real source block: the span is
/// replaced wholesale — fragment line numbers are meaningless against the
/// real file (the same move `quarantine` makes).
fn reattribute_to_source(diagnostic: Diagnostic, span: &SourceSpan) -> Diagnostic {
    diagnostic.with_span(span.clone())
}

fn extension_construct_name(kind: UnknownExtensionKind) -> &'static str {
    match kind {
        UnknownExtensionKind::MdxComponent => "MDX component",
        UnknownExtensionKind::PandocDirective => "Pandoc directive",
        UnknownExtensionKind::AttributeBlock => "attribute block",
        UnknownExtensionKind::MathFence => "math fence",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::diagnostic::Severity;
    use crate::infrastructure::parser::parse_markdown_page;

    fn markdown_source(text: &str) -> SourceFile {
        SourceFile::new_with_identity_path(
            PathBuf::from("guides/page.md"),
            text.to_string(),
            PathBuf::from("guides/page.md"),
        )
    }

    fn span_in(file: &str) -> SourceSpan {
        use crate::domain::diagnostic::SourcePosition;
        SourceSpan {
            file: PathBuf::from(file),
            start: SourcePosition {
                line: 3,
                column: 1,
                offset: 10,
            },
            end: SourcePosition {
                line: 3,
                column: 12,
                offset: 21,
            },
        }
    }

    #[test]
    fn reattributes_recheck_diagnostics_to_the_real_source_span() {
        let advisory = Diagnostic::warning(
            DiagnosticCode::MigrateUnrecognizedExtension,
            "synthetic strict advisory",
        )
        .with_span(span_in("migrate/recheck.adoc"));
        let block_span = span_in("guides/page.md");

        let reattributed = reattribute_to_source(advisory, &block_span);

        assert_eq!(reattributed.span, Some(block_span));
        assert_eq!(reattributed.severity, Severity::Warning);
        assert_eq!(
            reattributed.code,
            DiagnosticCode::MigrateUnrecognizedExtension
        );
        assert_eq!(reattributed.message, "synthetic strict advisory");
    }

    #[test]
    fn strict_recheck_forwards_non_error_diagnostics() {
        // No strict rule emits a WARNING/INFO today, so the only end-to-end
        // assertion possible is that a clean fragment forwards nothing; the
        // re-attribution seam itself is covered by the test above.
        let source = markdown_source("# Title\n\nProse.\n");
        let mut serializer = Serializer {
            source: &source,
            fragments: Vec::new(),
            diagnostics: Vec::new(),
        };

        let rejection = serializer.strict_rejection("Plain prose.", &span_in("guides/page.md"));

        assert_eq!(rejection, None);
        assert!(
            serializer.diagnostics.is_empty(),
            "{:?}",
            serializer.diagnostics
        );
    }

    fn serialize(text: &str) -> (String, Vec<Diagnostic>) {
        let source = markdown_source(text);
        let (page, parse_diagnostics) = parse_markdown_page(&source);
        assert!(
            parse_diagnostics
                .iter()
                .all(|diagnostic| diagnostic.severity != Severity::Error),
            "test markdown must parse cleanly: {parse_diagnostics:?}"
        );
        let (adoc_text, _prose_blocks, diagnostics) = page_to_adoc_source(&page, &source);
        (adoc_text, diagnostics)
    }

    #[test]
    fn headings_and_paragraphs_round_trip_as_prose() {
        let (text, diagnostics) = serialize("# Title\n\nSome *emphasized* prose.\n\n## Section\n");

        assert_eq!(text, "# Title\n\nSome *emphasized* prose.\n\n## Section\n");
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn tight_lists_serialize_with_canonical_markers() {
        let (text, diagnostics) = serialize("- alpha\n- beta\n\n1. first\n2. second\n");

        assert_eq!(text, "- alpha\n- beta\n\n1. first\n2. second\n");
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn code_blocks_round_trip_with_language() {
        let (text, diagnostics) = serialize("```rust\nfn main() {}\n```\n");

        assert_eq!(text, "```rust\nfn main() {}\n```\n");
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn task_checkboxes_are_dropped_with_a_diagnostic() {
        let (text, diagnostics) = serialize("- [x] done thing\n- [ ] open thing\n");

        assert_eq!(text, "- done thing\n- open thing\n");
        let drops: Vec<_> = diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.code == DiagnosticCode::MigrateUnrecognizedExtension
                    && diagnostic.message.contains("checkbox")
            })
            .collect();
        assert_eq!(drops.len(), 2, "{diagnostics:?}");
        assert!(
            drops
                .iter()
                .all(|diagnostic| !diagnostic.message.contains(QUARANTINE_PHRASE)),
            "checkbox drops are not quarantines"
        );
    }

    #[test]
    fn loose_list_is_quarantined_verbatim() {
        let markdown = "- first item\n\n  continuation paragraph\n- second item\n";
        let (text, diagnostics) = serialize(markdown);

        assert!(
            text.starts_with("```markdown\n") && text.contains("continuation paragraph"),
            "loose list must be fenced verbatim:\n{text}"
        );
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.code == DiagnosticCode::MigrateUnrecognizedExtension
                    && diagnostic.message.contains("loose or nested list")
                    && diagnostic.message.contains(QUARANTINE_PHRASE)
            }),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn raw_html_block_is_quarantined_in_an_html_fence() {
        let (text, diagnostics) =
            serialize("<div class=\"banner\">\n  <strong>Hi</strong>\n</div>\n");

        assert!(text.starts_with("```html\n"), "{text}");
        assert!(text.contains("<div class=\"banner\">"), "{text}");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.code == DiagnosticCode::MigrateRawHtmlQuarantined
                    && diagnostic.message.contains(QUARANTINE_PHRASE)
            }),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn table_and_footnote_are_quarantined_as_markdown_fences() {
        let markdown =
            "| a | b |\n| --- | --- |\n| 1 | 2 |\n\nNote.[^n]\n\n[^n]: the footnote body\n";
        let (text, diagnostics) = serialize(markdown);

        assert!(text.contains("```markdown\n| a | b |"), "{text}");
        assert!(
            text.contains("```markdown\n[^n]: the footnote body"),
            "{text}"
        );
        let quarantines = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.message.contains(QUARANTINE_PHRASE))
            .count();
        assert_eq!(quarantines, 2, "{diagnostics:?}");
    }

    #[test]
    fn strict_rejected_inline_html_paragraph_is_quarantined_as_raw_html() {
        // `<token>` sits inside inline code, but the strict raw-HTML scan has
        // no backtick awareness — the post-check quarantines what strict
        // would reject, by construction.
        let (text, diagnostics) =
            serialize("Send the `Authorization: Bearer <token>` header with every call.\n");

        assert!(text.starts_with("```markdown\n"), "{text}");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.code == DiagnosticCode::MigrateRawHtmlQuarantined
                    && diagnostic.message.contains(QUARANTINE_PHRASE)
            }),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn unsafe_link_paragraph_is_quarantined_as_broken_link() {
        let (text, diagnostics) = serialize("Click [here](javascript:doThing()) to trigger it.\n");

        assert!(text.starts_with("```markdown\n"), "{text}");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.code == DiagnosticCode::MigrateBrokenLink
                    && diagnostic.message.contains(QUARANTINE_PHRASE)
            }),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn nested_fence_content_is_an_error_not_a_mangled_file() {
        let markdown = "````\ninner\n```\nstill inner\n````\n";
        let (_, diagnostics) = serialize(markdown);

        let error = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.severity == Severity::Error)
            .expect("nested fence must refuse loudly");
        assert_eq!(error.code, DiagnosticCode::MigrateUnrecognizedExtension);
        assert!(error.message.contains("``` fence line"), "{error:?}");
    }

    #[test]
    fn thematic_break_round_trips_verbatim() {
        let (text, diagnostics) = serialize("before\n\n---\n\nafter\n");

        assert_eq!(text, "before\n\n---\n\nafter\n");
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn math_fence_is_quarantined_as_unrecognized_extension() {
        let (text, diagnostics) = serialize("$$\nE = mc^2\n$$\n");

        assert!(text.starts_with("```markdown\n"), "{text}");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.code == DiagnosticCode::MigrateUnrecognizedExtension
                    && diagnostic.message.contains("math fence")
            }),
            "{diagnostics:?}"
        );
    }
}
