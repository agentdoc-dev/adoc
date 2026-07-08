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
