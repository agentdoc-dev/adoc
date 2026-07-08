//! V8.1.4 export serializer: `PageAst` (parsed from strict prose-mode
//! `.adoc`) → Markdown source text (ADR-0043 §5).
//!
//! The reverse of `adoc_source`: where import quarantines what strict cannot
//! carry, export *unwraps* those carriers — a fenced code block whose info
//! string is exactly `markdown` or `html` becomes its verbatim content, each
//! unwrap backed 1:1 by a WARNING under the same code the import quarantine
//! used for that carrier, so counts reconcile across a round trip. A genuine
//! hand-written ` ```markdown `/` ```html ` fence is indistinguishable from a
//! carrier and unwraps too — a member of the ADR-0043 §5 closed set, not a
//! bug.
//!
//! Markdown is the permissive target, so there is no re-check/quarantine
//! pass and no escaping: the strict grammar cannot produce a paragraph that
//! re-parses as a different Markdown block (fences close at the bare ```` ``` ````
//! line, list markers and headings are canonical, setext underlines cannot
//! merge across the `\n\n` join). Hand-authored strict prose that happens to
//! start a line with `+ ` mirrors `adoc_source`'s no-escaping stance.

use crate::domain::ast::{BlockAst, ListKind, PageAst};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::inline::to_source;
use crate::domain::source::SourceFile;

/// The reconciliation anchor for export: every unwrap diagnostic message
/// contains this phrase, mirroring `adoc_source::QUARANTINE_PHRASE`.
pub(crate) const UNWRAP_PHRASE: &str = "unwrapped to its verbatim content";

/// Serialize a strict prose-mode page to Markdown source text.
///
/// Returns the text, the serialized fragment count (the report's
/// `prose_blocks` — headings, paragraphs, lists, fences, and unwrapped
/// payloads each count one), and the `migrate.*` diagnostics the
/// serialization produced (fence unwraps; a defensive ERROR on typed
/// blocks, which the orchestrator refuses before calling).
pub(crate) fn page_to_markdown(
    page: &PageAst,
    source: &SourceFile,
) -> (String, usize, Vec<Diagnostic>) {
    let mut exporter = Exporter {
        source,
        fragments: Vec::new(),
        diagnostics: Vec::new(),
    };
    for block in &page.blocks {
        exporter.push_block(block);
    }
    let prose_blocks = exporter.fragments.len();
    let mut text = exporter.fragments.join("\n\n");
    text.push('\n');
    (text, prose_blocks, exporter.diagnostics)
}

struct Exporter<'a> {
    source: &'a SourceFile,
    fragments: Vec<String>,
    diagnostics: Vec<Diagnostic>,
}

impl Exporter<'_> {
    fn push_block(&mut self, block: &BlockAst) {
        match block {
            BlockAst::Heading(heading) => self.fragments.push(format!(
                "{} {}",
                "#".repeat(usize::from(heading.level)),
                to_source(&heading.inlines)
            )),
            BlockAst::Paragraph(paragraph) => {
                // Single line by strict-parser construction: contiguous prose
                // lines fold to one with single spaces at parse time, which
                // is the §5 soft-break-rejoining member already applied.
                self.fragments.push(to_source(&paragraph.inlines));
            }
            BlockAst::List(list) => {
                let mut lines = Vec::with_capacity(list.items.len());
                for (index, item) in list.items.iter().enumerate() {
                    let marker = match list.kind {
                        ListKind::Unordered => "- ".to_string(),
                        ListKind::Ordered => format!("{}. ", index + 1),
                    };
                    lines.push(format!("{marker}{}", to_source(&item.inlines)));
                }
                self.fragments.push(lines.join("\n"));
            }
            BlockAst::CodeBlock(code_block) => {
                let language = code_block.language.as_deref().unwrap_or("");
                // The parser stores code with a trailing newline per line;
                // strip exactly one, mirroring `adoc_source::push_block`.
                let body = code_block
                    .code
                    .strip_suffix('\n')
                    .unwrap_or(&code_block.code);
                if matches!(language, "markdown" | "html") {
                    // The quarantine ceiling (ADR-0043 §5): the fence info
                    // string is the only signal a carrier leaves, so exactly
                    // these two info strings unwrap — under the same code the
                    // import quarantine emitted for that carrier.
                    let code = if language == "html" {
                        DiagnosticCode::MigrateRawHtmlQuarantined
                    } else {
                        DiagnosticCode::MigrateUnrecognizedExtension
                    };
                    self.fragments.push(body.to_string());
                    self.diagnostics.push(
                        Diagnostic::warning(
                            code,
                            format!(
                                "```{language} fence {UNWRAP_PHRASE} in {}",
                                self.source.path.display()
                            ),
                        )
                        .with_span(code_block.span.clone()),
                    );
                } else {
                    let mut fragment = format!("```{language}\n");
                    if !body.is_empty() {
                        fragment.push_str(body);
                        fragment.push('\n');
                    }
                    fragment.push_str("```");
                    self.fragments.push(fragment);
                }
            }
            BlockAst::KnowledgeObject(_) | BlockAst::KnowledgeObjectPending(_) => {
                // The orchestrator refuses typed pages before serializing;
                // this arm keeps the serializer total (the adoc_source move).
                self.diagnostics.push(Diagnostic::error(
                    DiagnosticCode::MigrateExportTypedBlocksPresent,
                    format!(
                        "{} contains a typed Knowledge Object block; exporting typed \
                         knowledge to Markdown is lossy by definition and the run is refused",
                        self.source.path.display()
                    ),
                ));
            }
            BlockAst::QuarantinedHtml(_)
            | BlockAst::ThematicBreak(_)
            | BlockAst::Table(_)
            | BlockAst::FootnoteDefinition(_)
            | BlockAst::UnknownExtension(_) => {
                // Compatibility-Mode-only variants; the strict `.adoc` parser
                // never produces them (their doc comments say so). Surface
                // loudly instead of panicking.
                self.diagnostics.push(Diagnostic::error(
                    DiagnosticCode::MigrateUnrecognizedExtension,
                    format!(
                        "internal: Compatibility Mode block variant in strict source {}; \
                         the .adoc parser must never produce one",
                        self.source.path.display()
                    ),
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::domain::diagnostic::Severity;
    use crate::infrastructure::parser::{parse_markdown_page, parse_page};
    use crate::infrastructure::render::page_to_adoc_source;

    /// The full chain a pilot file travels: `.md` → import serializer →
    /// strict parse → export serializer. One fixture per ADR-0043 §5
    /// closed-set member, each asserting exact output bytes.
    fn round_trip(markdown: &str) -> (String, Vec<Diagnostic>) {
        let md_source = SourceFile::new_with_identity_path(
            PathBuf::from("guides/page.md"),
            markdown.to_string(),
            PathBuf::from("guides/page.md"),
        );
        let (md_page, _) = parse_markdown_page(&md_source);
        let (adoc_text, _, import_diagnostics) = page_to_adoc_source(&md_page, &md_source);
        assert!(
            import_diagnostics
                .iter()
                .all(|diagnostic| diagnostic.severity != Severity::Error),
            "import must not error: {import_diagnostics:?}"
        );
        let adoc_source = SourceFile::new_with_identity_path(
            PathBuf::from("guides/page.adoc"),
            adoc_text,
            PathBuf::from("guides/page.adoc"),
        );
        let (adoc_page, parse_diagnostics) = parse_page(&adoc_source);
        assert!(
            parse_diagnostics
                .iter()
                .all(|diagnostic| diagnostic.severity != Severity::Error),
            "migrated output must parse strict-clean: {parse_diagnostics:?}"
        );
        let (text, _, export_diagnostics) = page_to_markdown(&adoc_page, &adoc_source);
        (text, export_diagnostics)
    }

    #[test]
    fn star_and_plus_list_markers_normalize_to_dash() {
        assert_eq!(round_trip("* alpha\n* beta\n").0, "- alpha\n- beta\n");
        assert_eq!(round_trip("+ alpha\n+ beta\n").0, "- alpha\n- beta\n");
    }

    #[test]
    fn ordered_list_markers_renumber_sequentially() {
        assert_eq!(
            round_trip("1. first\n1. second\n1. third\n").0,
            "1. first\n2. second\n3. third\n"
        );
    }

    #[test]
    fn soft_wrapped_paragraph_rejoins_to_one_line() {
        // Soft breaks only — a two-trailing-space hard break quarantines at
        // import and round-trips verbatim inside its fence carrier instead.
        assert_eq!(
            round_trip("A sentence wrapped\nacross two lines.\n").0,
            "A sentence wrapped across two lines.\n"
        );
    }

    #[test]
    fn trailing_whitespace_is_stripped() {
        assert_eq!(round_trip("# Title \n\nProse. \n").0, "# Title\n\nProse.\n");
    }

    #[test]
    fn underscore_emphasis_canonicalizes_to_star() {
        assert_eq!(
            round_trip("_emphasis_ and __strong__ words.\n").0,
            "*emphasis* and **strong** words.\n"
        );
    }

    #[test]
    fn tilde_fence_normalizes_to_backticks_with_info_string_preserved() {
        assert_eq!(
            round_trip("~~~rust\nlet x = 1;\n~~~\n").0,
            "```rust\nlet x = 1;\n```\n"
        );
    }

    #[test]
    fn hand_written_markdown_fence_unwraps_with_a_warning() {
        // The quarantine ceiling (ADR-0043 §5): the info string is the only
        // signal, so a genuine ```markdown fence does not survive export.
        let (text, diagnostics) = round_trip("```markdown\n# Example\n```\n");

        assert_eq!(text, "# Example\n");
        let unwrap = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.message.contains(UNWRAP_PHRASE))
            .expect("unwrap must be diagnosed");
        assert_eq!(unwrap.severity, Severity::Warning);
        assert_eq!(unwrap.code, DiagnosticCode::MigrateUnrecognizedExtension);
    }

    #[test]
    fn html_quarantine_round_trips_byte_identically_with_a_warning() {
        let original = "# Alerts\n\n<div class=\"alert\">Do not restart.</div>\n";

        let (text, diagnostics) = round_trip(original);

        assert_eq!(text, original);
        let unwrap = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.message.contains(UNWRAP_PHRASE))
            .expect("unwrap must be diagnosed");
        assert_eq!(unwrap.code, DiagnosticCode::MigrateRawHtmlQuarantined);
    }

    #[test]
    fn table_quarantine_round_trips_byte_identically() {
        let original = "# Limits\n\n| Tier | Limit |\n| --- | --- |\n| Free | 100 |\n";

        let (text, diagnostics) = round_trip(original);

        assert_eq!(text, original);
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.code == DiagnosticCode::MigrateUnrecognizedExtension
                    && diagnostic.message.contains(UNWRAP_PHRASE)
            }),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn dropped_constructs_do_not_round_trip() {
        // §5 member 7: front matter and checkbox markers are diagnosed drops
        // at import; export cannot resurrect them.
        assert_eq!(
            round_trip("---\ntitle: Setup\n---\n\n# Setup\n\n- [x] done thing\n").0,
            "# Setup\n\n- done thing\n"
        );
    }
}
