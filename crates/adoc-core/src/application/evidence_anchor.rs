//! Evidence Anchor verification pass (V8.5.1, ADR-0048).
//!
//! Re-hashes every file a path-target `source` object anchors via its
//! `hash` field and emits `evidence.*` warnings when the cited bytes
//! drifted, the target is gone, the value is malformed, or the anchor sits
//! on a url source. Runs only from the check entry point — build, review,
//! diff, and patch recompiles never read evidence files. Opt-in: a source
//! without `hash` costs zero reads and zero diagnostics.

use std::collections::BTreeMap;

use crate::application::hashing::sha256_prefixed;
use crate::domain::ast::{BlockAst, WorkspaceAst};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::knowledge_object::KnowledgeObject;
use crate::domain::ports::evidence_file::{EvidenceFileRead, EvidenceFileReader};
use crate::domain::value_objects::anchor_hash::AnchorHash;

const HASH_FIELD: &str = "hash";

/// One read + hash per distinct cited path, however many sources cite it.
enum CachedRead {
    Hash(String),
    Missing,
    Unreadable(String),
}

pub(crate) fn check_evidence_anchors(
    workspace: &WorkspaceAst,
    reader: &dyn EvidenceFileReader,
) -> Vec<Diagnostic> {
    let mut cache: BTreeMap<&str, CachedRead> = BTreeMap::new();
    let mut diagnostics = Vec::new();

    for page in &workspace.pages {
        for block in &page.blocks {
            let BlockAst::KnowledgeObject(ko) = block else {
                continue;
            };
            let KnowledgeObject::Source(source) = ko.as_ref() else {
                continue;
            };
            let Some(authored) = source.fields().get(HASH_FIELD) else {
                continue;
            };
            let id = source.id().as_str();

            let Some(path) = source.path() else {
                diagnostics.push(
                    Diagnostic::warning(
                        DiagnosticCode::EvidenceHashUnverifiable,
                        format!(
                            "source `{id}` has a `hash` anchor but a `url` target; \
                             url evidence cannot be verified offline"
                        ),
                    )
                    .with_span(source.span().clone())
                    .with_object_id(id),
                );
                continue;
            };

            let read = cache
                .entry(path.as_str())
                .or_insert_with(|| match reader.read(path) {
                    EvidenceFileRead::Found(bytes) => CachedRead::Hash(sha256_prefixed(&bytes)),
                    EvidenceFileRead::Missing => CachedRead::Missing,
                    EvidenceFileRead::Unreadable(message) => CachedRead::Unreadable(message),
                });

            match AnchorHash::try_new(authored) {
                Err(_) => {
                    let mut help = DiagnosticCode::EvidenceHashInvalid.default_help().to_string();
                    if let CachedRead::Hash(actual) = read {
                        help = format!(
                            "{help} The actual content hash of `{path}` is `{actual}`.",
                            path = path.as_str()
                        );
                    }
                    diagnostics.push(
                        Diagnostic::warning(
                            DiagnosticCode::EvidenceHashInvalid,
                            format!("source `{id}` has invalid `hash` value `{authored}`"),
                        )
                        .with_span(source.span().clone())
                        .with_object_id(id)
                        .with_help(help),
                    );
                }
                Ok(anchor) => match read {
                    CachedRead::Missing => diagnostics.push(
                        Diagnostic::warning(
                            DiagnosticCode::EvidenceHashTargetMissing,
                            format!(
                                "source `{id}` anchors `hash` to `{path}`, which does not exist",
                                path = path.as_str()
                            ),
                        )
                        .with_span(source.span().clone())
                        .with_object_id(id),
                    ),
                    CachedRead::Unreadable(message) => diagnostics.push(
                        Diagnostic::warning(
                            DiagnosticCode::EvidenceHashTargetMissing,
                            format!(
                                "source `{id}` anchors `hash` to `{path}`, which could not be read: {message}",
                                path = path.as_str()
                            ),
                        )
                        .with_span(source.span().clone())
                        .with_object_id(id),
                    ),
                    CachedRead::Hash(actual) if actual.as_str() != anchor.as_str() => diagnostics
                        .push(
                            Diagnostic::warning(
                                DiagnosticCode::EvidenceHashDrift,
                                format!(
                                    "source `{id}` evidence anchor drifted: the content of `{path}` \
                                     changed since it was verified",
                                    path = path.as_str()
                                ),
                            )
                            .with_span(source.span().clone())
                            .with_object_id(id)
                            .with_help(format!(
                                "Re-verify the knowledge citing this source, then update `hash:` \
                                 (and `last_seen_at`). Expected `{expected}`, actual `{actual}`.",
                                expected = anchor.as_str()
                            )),
                        ),
                    CachedRead::Hash(_) => {}
                },
            }
        }
    }

    diagnostics
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::application::compile::compile_with_provider_anchored_for_date;
    use crate::domain::diagnostic::Severity;
    use crate::domain::source::SourceFile;
    use crate::domain::value_objects::rel_path::RelPath;
    use crate::infrastructure::source::in_memory::InMemorySourceProvider;
    use chrono::NaiveDate;
    use std::path::PathBuf;

    const CITED_BYTES: &[u8] = b"fn consume() {}\n";

    fn cited_hash() -> String {
        sha256_prefixed(CITED_BYTES)
    }

    fn fixed_today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 5, 8).expect("valid test date")
    }

    /// In-memory reader that records every read so tests can assert the
    /// no-anchor ⇒ no-reads invariant and the one-read-per-path cache.
    struct RecordingReader {
        files: BTreeMap<String, EvidenceFileRead>,
        reads: RefCell<Vec<String>>,
    }

    impl RecordingReader {
        fn new(files: BTreeMap<String, EvidenceFileRead>) -> Self {
            Self {
                files,
                reads: RefCell::new(Vec::new()),
            }
        }

        fn with_cited_file() -> Self {
            Self::new(BTreeMap::from([(
                "src/consume.ts".to_string(),
                EvidenceFileRead::Found(CITED_BYTES.to_vec()),
            )]))
        }

        fn reads(&self) -> Vec<String> {
            self.reads.borrow().clone()
        }
    }

    impl EvidenceFileReader for RecordingReader {
        fn read(&self, path: &RelPath) -> EvidenceFileRead {
            self.reads.borrow_mut().push(path.as_str().to_string());
            self.files
                .get(path.as_str())
                .cloned()
                .unwrap_or(EvidenceFileRead::Missing)
        }
    }

    fn source_page(hash_line: &str) -> String {
        format!(
            concat!(
                "# Guide @doc(team.guide)\n\n",
                "::source billing.consume\n",
                "kind: source_code\n",
                "path: src/consume.ts\n",
                "{hash_line}",
                "--\n",
                "Implementation of credit consumption.\n",
                "::\n",
            ),
            hash_line = hash_line
        )
    }

    fn compile_anchored(text: &str, reader: &RecordingReader) -> Vec<Diagnostic> {
        let provider =
            InMemorySourceProvider::new().with_source(SourceFile::new_with_identity_path(
                PathBuf::from("/work/guide.adoc"),
                text.to_string(),
                PathBuf::from("guide.adoc"),
            ));
        compile_with_provider_anchored_for_date(&provider, reader, fixed_today()).diagnostics
    }

    #[test]
    fn fresh_anchor_emits_no_diagnostics() {
        let reader = RecordingReader::with_cited_file();
        let text = source_page(&format!("hash: {}\n", cited_hash()));

        let diagnostics = compile_anchored(&text, &reader);

        assert!(diagnostics.is_empty(), "diagnostics: {diagnostics:?}");
        assert_eq!(reader.reads(), vec!["src/consume.ts".to_string()]);
    }

    #[test]
    fn drifted_anchor_warns_with_expected_and_actual_in_help() {
        let reader = RecordingReader::with_cited_file();
        let stale = sha256_prefixed(b"older bytes");
        let text = source_page(&format!("hash: {stale}\n"));

        let diagnostics = compile_anchored(&text, &reader);

        assert_eq!(diagnostics.len(), 1, "diagnostics: {diagnostics:?}");
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.code, DiagnosticCode::EvidenceHashDrift);
        assert_eq!(diagnostic.severity, Severity::Warning);
        assert_eq!(diagnostic.object_id.as_deref(), Some("billing.consume"));
        assert!(
            diagnostic.span.is_some(),
            "drift warning carries the block span"
        );
        let help = diagnostic.help.as_deref().expect("help");
        assert!(
            help.contains(&stale),
            "help names the expected hash: {help}"
        );
        assert!(
            help.contains(&cited_hash()),
            "help names the actual hash: {help}"
        );
    }

    #[test]
    fn missing_target_warns_target_missing() {
        let reader = RecordingReader::new(BTreeMap::new());
        let text = source_page(&format!("hash: {}\n", cited_hash()));

        let diagnostics = compile_anchored(&text, &reader);

        assert_eq!(diagnostics.len(), 1, "diagnostics: {diagnostics:?}");
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::EvidenceHashTargetMissing
        );
        assert!(diagnostics[0].message.contains("does not exist"));
    }

    #[test]
    fn unreadable_target_warns_target_missing_with_io_message() {
        let reader = RecordingReader::new(BTreeMap::from([(
            "src/consume.ts".to_string(),
            EvidenceFileRead::Unreadable("permission denied".to_string()),
        )]));
        let text = source_page(&format!("hash: {}\n", cited_hash()));

        let diagnostics = compile_anchored(&text, &reader);

        assert_eq!(diagnostics.len(), 1, "diagnostics: {diagnostics:?}");
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::EvidenceHashTargetMissing
        );
        assert!(diagnostics[0].message.contains("permission denied"));
    }

    #[test]
    fn malformed_hash_warns_invalid_and_offers_actual_hash() {
        let reader = RecordingReader::with_cited_file();
        let text = source_page("hash: sha256:0\n");

        let diagnostics = compile_anchored(&text, &reader);

        assert_eq!(diagnostics.len(), 1, "diagnostics: {diagnostics:?}");
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.code, DiagnosticCode::EvidenceHashInvalid);
        assert!(
            diagnostic
                .help
                .as_deref()
                .expect("help")
                .contains(&cited_hash()),
            "bootstrap path: help carries the actual hash"
        );
    }

    #[test]
    fn hash_on_url_source_warns_unverifiable() {
        let reader = RecordingReader::new(BTreeMap::new());
        let text = concat!(
            "# Guide @doc(team.guide)\n\n",
            "::source billing.docs\n",
            "kind: external_url\n",
            "url: https://example.com/spec\n",
            "hash: sha256:0\n",
            "--\n",
            "External spec.\n",
            "::\n",
        );

        let diagnostics = compile_anchored(text, &reader);

        assert_eq!(diagnostics.len(), 1, "diagnostics: {diagnostics:?}");
        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::EvidenceHashUnverifiable
        );
        assert!(reader.reads().is_empty(), "url sources are never read");
    }

    #[test]
    fn source_without_hash_costs_zero_reads_and_zero_diagnostics() {
        let reader = RecordingReader::with_cited_file();
        let text = source_page("");

        let diagnostics = compile_anchored(&text, &reader);

        assert!(diagnostics.is_empty(), "diagnostics: {diagnostics:?}");
        assert!(reader.reads().is_empty(), "no anchor must mean no reads");
    }

    #[test]
    fn two_sources_citing_one_path_read_it_once() {
        let reader = RecordingReader::with_cited_file();
        let stale = sha256_prefixed(b"older bytes");
        let text = format!(
            concat!(
                "# Guide @doc(team.guide)\n\n",
                "::source billing.consume\n",
                "kind: source_code\n",
                "path: src/consume.ts\n",
                "hash: {stale}\n",
                "--\n",
                "First citation.\n",
                "::\n\n",
                "::source billing.consume-test\n",
                "kind: test\n",
                "path: src/consume.ts\n",
                "hash: {stale}\n",
                "--\n",
                "Second citation.\n",
                "::\n",
            ),
            stale = stale
        );

        let diagnostics = compile_anchored(&text, &reader);

        assert_eq!(diagnostics.len(), 2, "one warning per source");
        assert!(
            diagnostics
                .iter()
                .all(|d| d.code == DiagnosticCode::EvidenceHashDrift)
        );
        assert_eq!(
            reader.reads(),
            vec!["src/consume.ts".to_string()],
            "one read per distinct path"
        );
    }
}
