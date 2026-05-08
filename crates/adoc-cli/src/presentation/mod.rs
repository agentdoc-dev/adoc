pub(crate) mod json;
pub(crate) mod plain;
pub(crate) mod port;
pub(crate) mod terminal;

pub(crate) use json::JsonPresenter;
pub(crate) use plain::PlainPresenter;
pub(crate) use port::ExplainPresenter;

/// The output format requested by the user via `--format`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FormatChoice {
    /// Detect automatically: styled when stdout is a TTY, plain otherwise.
    Auto,
    /// Plain uncoloured text.
    Plain,
    /// Styled text (alias for plain in this slice; full styling lands in slice 4).
    Styled,
    /// Machine-readable JSON.
    Json,
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
    /// Styled is an alias for plain in this slice.
    Styled,
    Json,
}

/// Returns a presenter for the resolved format.
///
/// `Styled` currently aliases `Plain`; a real `StyledPresenter` arrives in slice 4.
pub(crate) fn make_presenter(resolved: ResolvedFormat) -> Box<dyn ExplainPresenter> {
    match resolved {
        ResolvedFormat::Plain | ResolvedFormat::Styled => Box::new(PlainPresenter),
        ResolvedFormat::Json => Box::new(JsonPresenter),
    }
}
