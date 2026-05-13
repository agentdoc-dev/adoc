use std::num::NonZeroUsize;
use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use crate::presentation::{ColorChoice, FormatChoice};

const ROOT_LONG_HELP: &str = "\
Examples:
  adoc init
  adoc check docs
  adoc build docs --out dist
  adoc why billing.refunds.issue-credit
  adoc search \"refund policy\"
";
const INIT_LONG_HELP: &str = "\
Examples:
  adoc init
";
const CHECK_LONG_HELP: &str = "\
Examples:
  adoc check
  adoc check docs
  adoc check docs/refunds.adoc
";
const BUILD_LONG_HELP: &str = "\
Examples:
  adoc build
  adoc build docs --out dist
  adoc build docs --out dist --no-embeddings
";
const WHY_LONG_HELP: &str = "\
Examples:
  adoc why billing.refunds.issue-credit
  adoc why billing.refunds.issue-credit --artifact dist/docs.agent.json
  adoc why billing.refunds.issue-credit --format json
";
const SEARCH_LONG_HELP: &str = "\
Examples:
  adoc search \"refund policy\"
  adoc search \"refund policy\" --kind claim --top 5
  adoc search billing.refunds --lexical
";

/// The output format requested on the command line (`--format`).
#[derive(Clone, Copy, Default, ValueEnum)]
pub(crate) enum CliFormat {
    /// Auto-detect: styled when stdout is a TTY, plain otherwise.
    #[default]
    Auto,
    /// Plain uncoloured text.
    Plain,
    /// Styled text with ANSI colour codes.
    Styled,
    /// Machine-readable JSON.
    Json,
}

impl From<CliFormat> for FormatChoice {
    fn from(f: CliFormat) -> Self {
        match f {
            CliFormat::Auto => Self::Auto,
            CliFormat::Plain => Self::Plain,
            CliFormat::Styled => Self::Styled,
            CliFormat::Json => Self::Json,
        }
    }
}

/// The colour mode requested on the command line (`--color`).
#[derive(Clone, Copy, Default, ValueEnum)]
pub(crate) enum CliColor {
    /// Enable colour only when stdout is a TTY and `NO_COLOR` is unset.
    #[default]
    Auto,
    /// Always emit ANSI colour codes.
    Always,
    /// Never emit ANSI colour codes.
    Never,
}

impl From<CliColor> for ColorChoice {
    fn from(c: CliColor) -> Self {
        match c {
            CliColor::Auto => Self::Auto,
            CliColor::Always => Self::Always,
            CliColor::Never => Self::Never,
        }
    }
}

#[derive(Parser)]
#[command(
    name = "adoc",
    version,
    about = "AgentDoc Local CLI for checking, building, and querying AgentDoc Source.",
    after_long_help = ROOT_LONG_HELP
)]
pub(crate) struct Cli {
    /// Output format.  `auto` selects `styled` when stdout is a TTY and
    /// `NO_COLOR` is unset, otherwise `plain`.
    #[arg(long, global = true, value_enum, default_value = "auto")]
    pub(crate) format: CliFormat,

    /// Colour output.  `auto` enables colour only on a TTY without `NO_COLOR`.
    /// `always` overrides the TTY check.  `never` disables colour.
    #[arg(long, global = true, value_enum, default_value = "auto")]
    pub(crate) color: CliColor,

    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    #[command(
        about = "Create AgentDoc config and starter docs.",
        after_long_help = INIT_LONG_HELP
    )]
    Init,
    #[command(
        about = "Check AgentDoc Source for strict-mode diagnostics.",
        after_long_help = CHECK_LONG_HELP
    )]
    Check {
        /// AgentDoc Source file or directory to check.
        #[arg(value_name = "PATH")]
        path: Option<PathBuf>,
    },
    #[command(
        about = "Build human and agent-facing artifacts.",
        after_long_help = BUILD_LONG_HELP
    )]
    Build {
        /// AgentDoc Source file or directory to build.
        #[arg(value_name = "PATH")]
        path: Option<PathBuf>,
        /// Output directory for docs.html, docs.agent.json, and docs.search.json.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Skip embedding generation and search artifact writes.
        #[arg(long)]
        no_embeddings: bool,
    },
    #[command(
        about = "Explain one Knowledge Object from a compiled artifact.",
        after_long_help = WHY_LONG_HELP
    )]
    Why {
        /// Object ID to explain.
        #[arg(value_name = "OBJECT_ID")]
        object_id: String,
        #[arg(
            long,
            help = "Agent JSON artifact path (default: config outputs.agent_json, then dist/docs.agent.json)"
        )]
        artifact: Option<PathBuf>,
    },
    #[command(
        about = "Search compiled Knowledge Objects.",
        after_long_help = SEARCH_LONG_HELP
    )]
    Search {
        /// Query text or Object ID prefix to search for.
        #[arg(value_name = "QUERY")]
        query: String,
        #[arg(
            long,
            help = "Agent JSON artifact path (default: config outputs.agent_json, then dist/docs.agent.json)"
        )]
        artifact: Option<PathBuf>,
        #[arg(
            long,
            help = "Search artifact path (default: config outputs.search, then dist/docs.search.json)"
        )]
        search_artifact: Option<PathBuf>,
        #[arg(long, conflicts_with = "lexical")]
        semantic: bool,
        /// Reserved for the V1.5/V1.6 hybrid slice; today this is the default
        /// when neither --semantic nor --lexical is set, so the flag is a no-op.
        #[arg(long, conflicts_with = "semantic")]
        lexical: bool,
        #[arg(long)]
        kind: Option<String>,
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        owner: Option<String>,
        #[arg(long)]
        source_path: Option<String>,
        #[arg(long, default_value = "10")]
        top: NonZeroUsize,
    },
}
