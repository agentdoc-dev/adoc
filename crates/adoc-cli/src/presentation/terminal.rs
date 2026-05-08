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
/// ```
/// # use adoc_cli::presentation::{ColorChoice, FormatChoice, ResolvedFormat};
/// # use adoc_cli::presentation::terminal::resolve;
/// let resolved = resolve(FormatChoice::Auto, ColorChoice::Auto, true, false);
/// assert_eq!(resolved, ResolvedFormat::Styled);
/// ```
pub fn resolve(
    format: FormatChoice,
    color: ColorChoice,
    is_stdout_tty: bool,
    no_color_env: bool,
) -> ResolvedFormat {
    match format {
        FormatChoice::Plain => ResolvedFormat::Plain,
        FormatChoice::Styled => ResolvedFormat::Styled,
        FormatChoice::Json => ResolvedFormat::Json,
        FormatChoice::Auto => resolve_auto(color, is_stdout_tty, no_color_env),
    }
}

fn resolve_auto(color: ColorChoice, is_stdout_tty: bool, no_color_env: bool) -> ResolvedFormat {
    match color {
        ColorChoice::Always => return ResolvedFormat::Styled,
        ColorChoice::Never => return ResolvedFormat::Plain,
        ColorChoice::Auto => {}
    }
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
pub fn detect(format: FormatChoice, color: ColorChoice) -> ResolvedFormat {
    let is_stdout_tty = std::io::stdout().is_terminal();
    let no_color_env = std::env::var_os("NO_COLOR").is_some();
    resolve(format, color, is_stdout_tty, no_color_env)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::{ColorChoice, FormatChoice, ResolvedFormat};

    // ------------------------------------------------------------------ auto

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
    fn auto_tty_color_never_gives_plain() {
        assert_eq!(
            resolve(FormatChoice::Auto, ColorChoice::Never, true, false),
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

    #[test]
    fn auto_no_tty_color_always_gives_styled() {
        // --color=always overrides the TTY check
        assert_eq!(
            resolve(FormatChoice::Auto, ColorChoice::Always, false, false),
            ResolvedFormat::Styled
        );
    }

    // ----------------------------------------------------------------- plain

    #[test]
    fn plain_ignores_tty_and_color() {
        assert_eq!(
            resolve(FormatChoice::Plain, ColorChoice::Always, true, false),
            ResolvedFormat::Plain
        );
        assert_eq!(
            resolve(FormatChoice::Plain, ColorChoice::Never, false, true),
            ResolvedFormat::Plain
        );
    }

    // ---------------------------------------------------------------- styled

    #[test]
    fn styled_ignores_tty_and_color() {
        assert_eq!(
            resolve(FormatChoice::Styled, ColorChoice::Never, false, true),
            ResolvedFormat::Styled
        );
        assert_eq!(
            resolve(FormatChoice::Styled, ColorChoice::Auto, true, false),
            ResolvedFormat::Styled
        );
    }

    // ------------------------------------------------------------------ json

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
}
