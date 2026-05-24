pub(crate) mod json;
pub(crate) mod markdown;
pub(crate) mod plain;
pub(crate) mod port;
pub(crate) mod style;
pub(crate) mod styled;
pub(crate) mod terminal;

pub(crate) use json::JsonPresenter;
pub(crate) use markdown::MarkdownReviewPresenter;
pub(crate) use plain::PlainPresenter;
pub(crate) use port::{
    ExpiresInfo, PresentationRecord, RenderMeta, RetrievalPresenter, RetrievalView,
};
pub(crate) use styled::StyledPresenter;

use adoc_core::Diagnostic;

/// The output format requested by the user via `--format`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FormatChoice {
    /// Detect automatically: styled when stdout is a TTY, plain otherwise.
    Auto,
    /// Plain uncoloured text.
    Plain,
    /// Styled text with ANSI colour codes.
    Styled,
    /// Machine-readable JSON.
    Json,
    /// GitHub-flavored Markdown for PR review comments. Only supported by
    /// `adoc diff` and `adoc review`; rejected at dispatch for other commands.
    Markdown,
}

/// The colour mode requested by the user via `--color`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColorChoice {
    /// Enable colour only when stdout is a TTY and `NO_COLOR` is unset.
    Auto,
    /// Always emit ANSI colour codes.
    Always,
    /// Never emit ANSI colour codes.
    Never,
}

/// The concrete format selected after auto-detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResolvedFormat {
    Plain,
    Styled,
    Json,
    /// Markdown is structural like Json — colour flags never alter it.
    /// Only `adoc diff` and `adoc review` accept this resolved variant; other
    /// commands reject it in `main.rs` before dispatching.
    Markdown,
}

/// Returns a presenter for the resolved format.
///
/// `load_diagnostics` are forwarded to [`JsonPresenter`] so that non-fatal
/// warnings collected during artifact loading round-trip into the JSON
/// envelope's `diagnostics` array.  Pass `Vec::new()` for non-JSON formats
/// (plain and styled emit load diagnostics to stderr before calling the
/// presenter).
pub(crate) fn make_presenter(
    resolved: ResolvedFormat,
    load_diagnostics: Vec<Diagnostic>,
) -> Box<dyn RetrievalPresenter> {
    match resolved {
        ResolvedFormat::Plain => Box::new(PlainPresenter),
        ResolvedFormat::Styled => Box::new(StyledPresenter),
        ResolvedFormat::Json => Box::new(JsonPresenter::new(load_diagnostics)),
        // Markdown is reachable only from `adoc diff` / `adoc review`, which
        // do not use the retrieval presenter port. Dispatch in `main.rs`
        // rejects markdown for every other command, so this arm cannot be hit.
        ResolvedFormat::Markdown => {
            unreachable!("markdown format is not supported by retrieval commands")
        }
    }
}
