//! V8.1.1 migration orchestration: lossless `.md` → prose-mode `.adoc`
//! import (PRD §28.1–§28.2, ADR-0043).
//!
//! Mirrors `application/compile.rs`: sources arrive through a
//! [`SourceProvider`], Compatibility Mode files are parsed with the existing
//! pulldown-cmark read path, and the `adoc_source` serializer renders each
//! page. This module performs no writes — the outcome carries the rendered
//! text and target paths; the adapter executes `--write` (all-or-nothing,
//! ADR-0043 §3). The `adoc.migrate.report.v0` envelope lands in V8.1.2.

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use crate::application::compile::load_error_diagnostic;
use crate::domain::ast::{BlockAst, PageAst};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, Severity};
use crate::domain::inline::InlineSegment;
use crate::domain::ports::source_provider::SourceProvider;
use crate::domain::source::{SourceFile, SourceMode};
use crate::infrastructure::git::is_committed_and_clean;
use crate::infrastructure::parser::{parse_markdown_page, skip_front_matter};
use crate::infrastructure::render::page_to_adoc_source;

/// How a migration run treats the filesystem (ADR-0043 §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrateMode {
    /// Report only — nothing is written, git is never probed.
    DryRun,
    /// Write `<name>.adoc` and remove the source `.md`. Unless `force`, every
    /// source must be committed-and-clean in git or the run is refused with
    /// `migrate.source_not_committed`.
    Write { force: bool },
}

/// One migrated source: where it came from, where it goes, and the rendered
/// canonical `.adoc` text.
#[derive(Debug, Clone)]
pub struct MigratedFile {
    pub source_path: PathBuf,
    pub target_path: PathBuf,
    pub adoc_text: String,
}

#[derive(Debug, Clone)]
pub struct MigrateResult {
    /// Every Compatibility Mode source under the root, in the provider's
    /// deterministic (lexicographic) order.
    pub files: Vec<MigratedFile>,
    pub diagnostics: Vec<Diagnostic>,
}

impl MigrateResult {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error)
    }
}

pub(crate) fn migrate_with_provider<P: SourceProvider>(
    provider: &P,
    mode: MigrateMode,
) -> MigrateResult {
    let mut diagnostics = Vec::new();
    let mut compat_sources = Vec::new();
    let mut strict_paths = BTreeSet::new();
    for result in provider.load_sources() {
        match result {
            Ok(source) => match source.mode() {
                SourceMode::Compat => compat_sources.push(source),
                SourceMode::Strict => {
                    strict_paths.insert(normalize_path(&source.path));
                }
            },
            Err(load_error) => diagnostics.push(load_error_diagnostic(load_error)),
        }
    }

    let migration_set: BTreeSet<PathBuf> = compat_sources
        .iter()
        .map(|source| normalize_path(&source.path))
        .collect();

    let mut files = Vec::with_capacity(compat_sources.len());
    for source in &compat_sources {
        if skip_front_matter(&source.text) > 0 {
            let first_line = source.text.lines().next().unwrap_or_default();
            diagnostics.push(
                Diagnostic::warning(
                    DiagnosticCode::MigrateUnrecognizedExtension,
                    format!(
                        "front matter dropped from {}; strict .adoc has no front-matter \
                         concept — git history and `adoc migrate --export` cover recovery",
                        source.path.display()
                    ),
                )
                .with_span(source.span_for_line(1, first_line)),
            );
        }
        let (page, parse_diagnostics) = parse_markdown_page(source);
        diagnostics.extend(parse_diagnostics);
        let (adoc_text, serialize_diagnostics) = page_to_adoc_source(&page, source);
        diagnostics.extend(serialize_diagnostics);
        diagnostics.extend(broken_link_diagnostics(&page, source, &migration_set));

        let target_path = source.path.with_extension("adoc");
        if strict_paths.contains(&normalize_path(&target_path)) {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::MigrateTargetExists,
                format!(
                    "migration target {} already exists; refusing the run — leaving \
                     {} beside it would compile duplicate page IDs",
                    target_path.display(),
                    source.path.display()
                ),
            ));
        }
        files.push(MigratedFile {
            source_path: source.path.clone(),
            target_path,
            adoc_text,
        });
    }

    if let MigrateMode::Write { force: false } = mode {
        for file in &files {
            if !is_committed_and_clean(&file.source_path) {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::MigrateSourceNotCommitted,
                    format!(
                        "{} is not committed-and-clean (uncommitted edits, untracked, or \
                         outside a git repository); `--write` removes the source, and a \
                         committed source is what makes that reversible",
                        file.source_path.display()
                    ),
                ));
            }
        }
    }

    MigrateResult { files, diagnostics }
}

/// Warn on relative links whose target is missing from the loaded source set
/// or is a `.md` this run migrates away (ADR-0043). Only links to source
/// extensions are judged — the provider does not see other assets, and a
/// guess would be a silent false positive.
fn broken_link_diagnostics(
    page: &PageAst,
    source: &SourceFile,
    migration_set: &BTreeSet<PathBuf>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let source_dir = source.path.parent().unwrap_or(Path::new(""));
    for block in &page.blocks {
        walk_block_links(block, &mut |url: &str, span| {
            if has_url_scheme(url) {
                return;
            }
            let target = url.split(['#', '?']).next().unwrap_or_default();
            if target.is_empty() {
                return;
            }
            let extension = Path::new(target).extension().and_then(|ext| ext.to_str());
            if !matches!(extension, Some("md") | Some("adoc")) {
                return;
            }
            let resolved = normalize_path(&source_dir.join(target));
            if migration_set.contains(&resolved) {
                diagnostics.push(
                    Diagnostic::warning(
                        DiagnosticCode::MigrateBrokenLink,
                        format!(
                            "link target {target} is migrated to .adoc by this run; \
                             update the link in {}",
                            source.path.display()
                        ),
                    )
                    .with_span(span.clone()),
                );
            } else if !resolved.exists() {
                diagnostics.push(
                    Diagnostic::warning(
                        DiagnosticCode::MigrateBrokenLink,
                        format!(
                            "link target {target} does not exist (referenced from {})",
                            source.path.display()
                        ),
                    )
                    .with_span(span.clone()),
                );
            }
        });
    }
    diagnostics
}

fn walk_block_links(
    block: &BlockAst,
    visit: &mut impl FnMut(&str, &crate::domain::diagnostic::SourceSpan),
) {
    match block {
        BlockAst::Heading(heading) => walk_inline_links(&heading.inlines, visit),
        BlockAst::Paragraph(paragraph) => walk_inline_links(&paragraph.inlines, visit),
        BlockAst::List(list) => {
            for item in &list.items {
                walk_inline_links(&item.inlines, visit);
                for child in &item.content {
                    walk_block_links(child, visit);
                }
            }
        }
        // Quarantined constructs keep their source text verbatim inside a
        // fence; their links are not rewritten, so they are not judged.
        _ => {}
    }
}

fn walk_inline_links(
    segments: &[InlineSegment],
    visit: &mut impl FnMut(&str, &crate::domain::diagnostic::SourceSpan),
) {
    for segment in segments {
        match segment {
            InlineSegment::Link { text, url, span } => {
                visit(url, span);
                walk_inline_links(text, visit);
            }
            InlineSegment::Image { alt, url, span } => {
                visit(url, span);
                walk_inline_links(alt, visit);
            }
            InlineSegment::Emphasis(inner)
            | InlineSegment::Strong(inner)
            | InlineSegment::Strikethrough(inner) => walk_inline_links(inner, visit),
            _ => {}
        }
    }
}

/// RFC 3986 §3.1 scheme detection: `ALPHA *( ALPHA / DIGIT / "+" / "-" /
/// "." ) ":"` marks an absolute URL (`https:`, `mailto:`, `data:`);
/// everything else — including digit-leading segments like
/// `2026:Q3-plan.md` — is treated as a relative path and judged.
fn has_url_scheme(url: &str) -> bool {
    let mut bytes = url.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    if !first.is_ascii_alphabetic() {
        return false;
    }
    for byte in bytes {
        match byte {
            b':' => return true,
            byte if byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'.' | b'-') => {}
            _ => return false,
        }
    }
    false
}

/// Lexically fold `.` and `..` components so provider paths and link-resolved
/// paths compare equal without touching the filesystem.
fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push("..");
                }
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::source::in_memory::InMemorySourceProvider;

    fn markdown_source(identity: &str, text: &str) -> SourceFile {
        SourceFile::new_with_identity_path(
            PathBuf::from(format!("/work/{identity}")),
            text.to_string(),
            PathBuf::from(identity),
        )
    }

    #[test]
    fn dry_run_migrates_only_markdown_sources() {
        let provider = InMemorySourceProvider::new()
            .with_source(markdown_source("guides/setup.md", "# Setup\n\nProse.\n"))
            .with_source(markdown_source(
                "knowledge/claims.adoc",
                "# Claims @doc(knowledge.claims)\n\nNative.\n",
            ));

        let result = migrate_with_provider(&provider, MigrateMode::DryRun);

        assert!(!result.has_errors(), "{:?}", result.diagnostics);
        assert_eq!(result.files.len(), 1);
        assert_eq!(
            result.files[0].target_path,
            PathBuf::from("/work/guides/setup.adoc")
        );
        assert_eq!(result.files[0].adoc_text, "# Setup\n\nProse.\n");
    }

    #[test]
    fn link_to_a_migrated_md_file_warns_broken_link() {
        let provider = InMemorySourceProvider::new()
            .with_source(markdown_source(
                "guides/setup.md",
                "# Setup\n\nSee [the glossary](../reference/glossary.md).\n",
            ))
            .with_source(markdown_source(
                "reference/glossary.md",
                "# Glossary\n\nTerms.\n",
            ));

        let result = migrate_with_provider(&provider, MigrateMode::DryRun);

        assert!(
            result.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == DiagnosticCode::MigrateBrokenLink
                    && diagnostic.message.contains("migrated to .adoc")
            }),
            "{:?}",
            result.diagnostics
        );
    }

    #[test]
    fn link_to_a_missing_source_warns_broken_link() {
        let provider = InMemorySourceProvider::new().with_source(markdown_source(
            "guides/setup.md",
            "# Setup\n\nSee [gone](./missing.md).\n",
        ));

        let result = migrate_with_provider(&provider, MigrateMode::DryRun);

        assert!(
            result.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == DiagnosticCode::MigrateBrokenLink
                    && diagnostic.message.contains("does not exist")
            }),
            "{:?}",
            result.diagnostics
        );
    }

    #[test]
    fn absolute_and_anchor_links_are_not_judged() {
        let provider = InMemorySourceProvider::new().with_source(markdown_source(
            "guides/setup.md",
            "# Setup\n\nSee [docs](https://example.test/x.md) and [top](#setup).\n",
        ));

        let result = migrate_with_provider(&provider, MigrateMode::DryRun);

        assert!(
            result
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.code != DiagnosticCode::MigrateBrokenLink),
            "{:?}",
            result.diagnostics
        );
    }

    #[test]
    fn url_scheme_detection_follows_rfc_3986_scheme_grammar() {
        assert!(has_url_scheme("https://example.test/x.md"));
        assert!(has_url_scheme("mailto:team@example.test"));
        // An all-alphabetic first segment before `:` satisfies the scheme
        // grammar, so it stays skipped — links styled `Class:method.md`
        // are indistinguishable from a scheme without touching the fs.
        assert!(has_url_scheme("Class:method.md"));
        // A digit-leading segment can never be a scheme (RFC 3986 §3.1);
        // it is a relative path and must be judged.
        assert!(!has_url_scheme("2026:Q3-plan.md"));
        assert!(!has_url_scheme("notes/2026:Q3-plan.md"));
        assert!(!has_url_scheme("./missing.md"));
        assert!(!has_url_scheme(""));
    }

    #[test]
    fn existing_adoc_target_is_an_error() {
        let provider = InMemorySourceProvider::new()
            .with_source(markdown_source("guides/setup.md", "# Setup\n\nProse.\n"))
            .with_source(markdown_source(
                "guides/setup.adoc",
                "# Setup @doc(guides.setup)\n\nAlready native.\n",
            ));

        let result = migrate_with_provider(&provider, MigrateMode::DryRun);

        assert!(result.has_errors());
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == DiagnosticCode::MigrateTargetExists),
            "{:?}",
            result.diagnostics
        );
    }

    #[test]
    fn write_mode_without_a_repository_refuses_every_source() {
        let provider = InMemorySourceProvider::new()
            .with_source(markdown_source("a/one.md", "# One\n\nProse.\n"))
            .with_source(markdown_source("a/two.md", "# Two\n\nProse.\n"));

        let result = migrate_with_provider(&provider, MigrateMode::Write { force: false });

        let refusals = result
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == DiagnosticCode::MigrateSourceNotCommitted)
            .count();
        assert_eq!(refusals, 2, "{:?}", result.diagnostics);
        assert!(result.has_errors());
    }

    #[test]
    fn write_mode_with_force_skips_the_probe() {
        let provider = InMemorySourceProvider::new()
            .with_source(markdown_source("a/one.md", "# One\n\nProse.\n"));

        let result = migrate_with_provider(&provider, MigrateMode::Write { force: true });

        assert!(!result.has_errors(), "{:?}", result.diagnostics);
    }
}
