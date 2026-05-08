//! TTY and colour detection for output format resolution.
//!
//! The pure [`resolve`] function maps the user's explicit choices plus
//! environmental facts onto a concrete [`ResolvedFormat`].  It has no side
//! effects and is fully unit-testable.
//!
//! The [`detect`] convenience wrapper calls the real OS APIs (`is-terminal`,
//! `NO_COLOR` env var) and delegates to [`resolve`].

use is_terminal::IsTerminal as _;

use super::{ColorChoice, FormatChoice, ResolvedFormat};

/// Resolve the output format from explicit choices and environmental facts.
///
/// Precedence rules (matching cargo / git / ripgrep convention):
///
/// 1. `--format=json` always wins — JSON is structural and must not be
///    influenced by colour flags.
/// 2. For all non-JSON formats, `--color` takes precedence over `--format`:
///    - `--color=never`  → [`ResolvedFormat::Plain`]  (no escapes)
///    - `--color=always` → [`ResolvedFormat::Styled`] (force colour)
///    - `--color=auto`   → honour the explicit `--format` value, or fall back
///      to TTY / `NO_COLOR` detection when `--format=auto`.
///
/// # Arguments
///
/// * `format`         – The value of `--format` (from clap).
/// * `color`          – The value of `--color` (from clap).
/// * `is_stdout_tty`  – Whether stdout is an interactive terminal.
/// * `no_color_env`   – Whether the `NO_COLOR` environment variable is set
///   (presence, not value — per <https://no-color.org>).
///
/// # Examples
///
/// ```ignore
/// # use adoc_cli::presentation::{ColorChoice, FormatChoice, ResolvedFormat};
/// # use adoc_cli::presentation::terminal::resolve;
/// let resolved = resolve(FormatChoice::Auto, ColorChoice::Auto, true, false);
/// assert_eq!(resolved, ResolvedFormat::Styled);
/// ```
pub(crate) fn resolve(
    format: FormatChoice,
    color: ColorChoice,
    is_stdout_tty: bool,
    no_color_env: bool,
) -> ResolvedFormat {
    // JSON is structural: colour flags must not alter it.
    if matches!(format, FormatChoice::Json) {
        return ResolvedFormat::Json;
    }
    match color {
        ColorChoice::Never => ResolvedFormat::Plain,
        ColorChoice::Always => ResolvedFormat::Styled,
        ColorChoice::Auto => match format {
            FormatChoice::Plain => ResolvedFormat::Plain,
            FormatChoice::Styled => ResolvedFormat::Styled,
            FormatChoice::Auto => resolve_auto(is_stdout_tty, no_color_env),
            FormatChoice::Json => unreachable!("Json handled above"),
        },
    }
}

fn resolve_auto(is_stdout_tty: bool, no_color_env: bool) -> ResolvedFormat {
    // color=auto: respect TTY + NO_COLOR
    if !is_stdout_tty {
        return ResolvedFormat::Plain;
    }
    if no_color_env {
        return ResolvedFormat::Plain;
    }
    ResolvedFormat::Styled
}

/// Detect the resolved format using the real OS APIs.
///
/// Reads `is_terminal` from `std::io::stdout()` and checks for the
/// `NO_COLOR` environment variable.
pub(crate) fn detect(format: FormatChoice, color: ColorChoice) -> ResolvedFormat {
    let is_stdout_tty = std::io::stdout().is_terminal();
    let no_color_env = std::env::var_os("NO_COLOR").is_some();
    resolve(format, color, is_stdout_tty, no_color_env)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::{ColorChoice, FormatChoice, ResolvedFormat};

    // -------------------------------------------------------- json overrides

    /// JSON is structural; colour flags must never change it.
    #[test]
    fn format_json_overrides_color_never_to_keep_json() {
        assert_eq!(
            resolve(FormatChoice::Json, ColorChoice::Never, true, false),
            ResolvedFormat::Json
        );
    }

    #[test]
    fn format_json_overrides_color_always_to_keep_json() {
        assert_eq!(
            resolve(FormatChoice::Json, ColorChoice::Always, true, false),
            ResolvedFormat::Json
        );
    }

    #[test]
    fn format_json_overrides_color_auto_to_keep_json() {
        assert_eq!(
            resolve(FormatChoice::Json, ColorChoice::Auto, true, false),
            ResolvedFormat::Json
        );
    }

    /// Exhaustive check: Json wins over every colour × tty × no_color combo.
    #[test]
    fn json_wins_over_any_combination() {
        for &color in &[ColorChoice::Auto, ColorChoice::Always, ColorChoice::Never] {
            for tty in [true, false] {
                for no_color in [true, false] {
                    assert_eq!(
                        resolve(FormatChoice::Json, color, tty, no_color),
                        ResolvedFormat::Json,
                        "json should always resolve to Json (color={color:?}, tty={tty}, no_color={no_color})"
                    );
                }
            }
        }
    }

    // ------------------------------------------------- color=never → Plain

    /// `--color=never` forces plain output even when format=styled.
    /// This is the key reviewer test: NO_COLOR=1 script wrappers can safely
    /// pass `--format=styled` and still get no ANSI escapes.
    #[test]
    fn format_styled_with_color_never_becomes_plain() {
        assert_eq!(
            resolve(FormatChoice::Styled, ColorChoice::Never, true, false),
            ResolvedFormat::Plain
        );
    }

    #[test]
    fn format_plain_with_color_never_stays_plain() {
        assert_eq!(
            resolve(FormatChoice::Plain, ColorChoice::Never, true, false),
            ResolvedFormat::Plain
        );
    }

    #[test]
    fn format_auto_with_color_never_becomes_plain() {
        assert_eq!(
            resolve(FormatChoice::Auto, ColorChoice::Never, true, false),
            ResolvedFormat::Plain
        );
    }

    // ------------------------------------------------ color=always → Styled

    /// `--color=always` forces styled output even when format=plain.
    #[test]
    fn format_plain_with_color_always_becomes_styled() {
        assert_eq!(
            resolve(FormatChoice::Plain, ColorChoice::Always, true, false),
            ResolvedFormat::Styled
        );
    }

    #[test]
    fn format_styled_with_color_always_stays_styled() {
        assert_eq!(
            resolve(FormatChoice::Styled, ColorChoice::Always, true, false),
            ResolvedFormat::Styled
        );
    }

    #[test]
    fn format_auto_with_color_always_becomes_styled() {
        assert_eq!(
            resolve(FormatChoice::Auto, ColorChoice::Always, false, false),
            ResolvedFormat::Styled
        );
    }

    // ------------------------------------------- color=auto honours format

    #[test]
    fn format_plain_with_color_auto_stays_plain() {
        assert_eq!(
            resolve(FormatChoice::Plain, ColorChoice::Auto, true, false),
            ResolvedFormat::Plain
        );
    }

    #[test]
    fn format_styled_with_color_auto_stays_styled() {
        assert_eq!(
            resolve(FormatChoice::Styled, ColorChoice::Auto, true, false),
            ResolvedFormat::Styled
        );
    }

    // -------------------------------- color=auto, format=auto → TTY/NO_COLOR

    #[test]
    fn auto_tty_no_no_color_auto_color_gives_styled() {
        assert_eq!(
            resolve(FormatChoice::Auto, ColorChoice::Auto, true, false),
            ResolvedFormat::Styled
        );
    }

    #[test]
    fn auto_tty_no_color_env_auto_color_gives_plain() {
        assert_eq!(
            resolve(FormatChoice::Auto, ColorChoice::Auto, true, true),
            ResolvedFormat::Plain
        );
    }

    #[test]
    fn auto_no_tty_auto_color_gives_plain() {
        assert_eq!(
            resolve(FormatChoice::Auto, ColorChoice::Auto, false, false),
            ResolvedFormat::Plain
        );
    }
}
