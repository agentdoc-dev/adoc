use std::io;

use adoc_core::RetrievalEnvelope;

/// Presenter trait for the `explain` (and search) output.
///
/// Each concrete presenter takes a [`RetrievalEnvelope`] and writes its
/// rendering to the provided [`io::Write`] sink.  The trait is object-safe so
/// callers can hold a `Box<dyn ExplainPresenter>` when the format is chosen at
/// runtime.
pub(crate) trait ExplainPresenter {
    fn present(&self, envelope: &RetrievalEnvelope, out: &mut dyn io::Write) -> io::Result<()>;
}
