use std::io;

use adoc_core::ExplainView;

/// Presenter trait for the `explain` output.
///
/// Each concrete presenter takes an [`ExplainView`] and writes its rendering
/// to the provided [`io::Write`] sink.  The trait is object-safe so callers
/// can hold a `Box<dyn ExplainPresenter>` when the format is chosen at runtime.
pub(crate) trait ExplainPresenter {
    /// Write the view to `out`.
    ///
    /// # Errors
    ///
    /// Returns an [`io::Error`] if the write fails.
    fn present(&self, view: &ExplainView, out: &mut dyn io::Write) -> io::Result<()>;
}
