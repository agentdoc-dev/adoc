use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

use adoc_core::{Diagnostic, ProseRecord, RetrievalRecord};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExpiresInfo {
    pub(crate) date: chrono::NaiveDate,
    pub(crate) days_until: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RenderMeta {
    pub(crate) artifact: PathBuf,
    pub(crate) trust: Option<String>,
    pub(crate) duration: Duration,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PresentationRecord {
    pub(crate) record: RetrievalRecord,
    pub(crate) related_statuses: BTreeMap<String, Option<String>>,
    pub(crate) expires: Option<ExpiresInfo>,
}

/// V1.7.1 (ADR-0040): one renderable search result — a Knowledge Object
/// record with its presentation extras, or a prose record rendered as
/// orientation context.
// Size asymmetry follows the `RetrievalEntry` precedent: records carry all
// fields inline; boxing adds indirection for no benefit.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PresentationEntry {
    KnowledgeObject(PresentationRecord),
    Prose(ProseRecord),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RetrievalView {
    pub(crate) records: Vec<PresentationEntry>,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) footer: Option<RenderMeta>,
}

/// Presenter trait for retrieval output.
///
/// Each concrete presenter takes a [`RetrievalView`] and writes its rendering
/// to the provided [`io::Write`] sink. The trait is object-safe so callers can
/// hold a `Box<dyn RetrievalPresenter>` when the format is chosen at runtime.
pub(crate) trait RetrievalPresenter {
    /// Write the view to `out`.
    ///
    /// # Errors
    ///
    /// Returns an [`io::Error`] if the write fails.
    fn present(&self, view: &RetrievalView, out: &mut dyn io::Write) -> io::Result<()>;
}
