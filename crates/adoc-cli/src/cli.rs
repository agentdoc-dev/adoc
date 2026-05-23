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
  adoc graph billing.refunds.issue-credit
  adoc patch --check patch.json
  adoc diff main
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
  adoc why billing.refunds.issue-credit --artifact dist/docs.graph.json
  adoc why billing.refunds.issue-credit --format json
";
const GRAPH_LONG_HELP: &str = "\
Examples:
  adoc graph billing.refunds.issue-credit
  adoc graph billing.refunds.issue-credit --direction outgoing
  adoc graph billing.refunds.issue-credit --relation depends_on --format json
";
const PATCH_LONG_HELP: &str = "\
Examples:
  adoc patch --check patch.json
  adoc patch --check patch.json --artifact dist/docs.graph.json
  adoc patch --check patch.json --format json
";
const DIFF_LONG_HELP: &str = "\
Examples:
  adoc diff main
  adoc diff main --format json
  adoc diff HEAD~1
";
const SEARCH_LONG_HELP: &str = "\
Examples:
  adoc search \"refund policy\"
  adoc search \"refund policy\" --kind claim --top 5
  adoc search \"refund policy\" --related-to billing.refunds.issue-credit --relation depends_on
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

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum CliGraphRelation {
    #[value(name = "depends_on")]
    DependsOn,
    Supersedes,
    #[value(name = "related_to")]
    RelatedTo,
}

impl From<CliGraphRelation> for adoc_core::GraphRelationKind {
    fn from(value: CliGraphRelation) -> Self {
        match value {
            CliGraphRelation::DependsOn => Self::DependsOn,
            CliGraphRelation::Supersedes => Self::Supersedes,
            CliGraphRelation::RelatedTo => Self::RelatedTo,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum CliGraphDirection {
    Outgoing,
    Incoming,
    Both,
}

impl From<CliGraphDirection> for adoc_core::GraphDirection {
    fn from(value: CliGraphDirection) -> Self {
        match value {
            CliGraphDirection::Outgoing => Self::Outgoing,
            CliGraphDirection::Incoming => Self::Incoming,
            CliGraphDirection::Both => Self::Both,
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
        about = "Build HTML, graph, and search artifacts.",
        after_long_help = BUILD_LONG_HELP
    )]
    Build {
        /// AgentDoc Source file or directory to build.
        #[arg(value_name = "PATH")]
        path: Option<PathBuf>,
        /// Output directory for docs.html, docs.graph.json, and docs.search.json.
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
            help = "Graph JSON artifact path (default: config outputs.graph, then dist/docs.graph.json)"
        )]
        artifact: Option<PathBuf>,
    },
    #[command(
        about = "Traverse Knowledge Object relations from graph artifacts.",
        after_long_help = GRAPH_LONG_HELP
    )]
    Graph {
        /// Object ID to use as the graph traversal root.
        #[arg(value_name = "OBJECT_ID")]
        object_id: String,
        #[arg(
            long,
            help = "Graph JSON artifact path (default: config outputs.graph, then dist/docs.graph.json)"
        )]
        artifact: Option<PathBuf>,
        #[arg(long, value_enum)]
        relation: Option<CliGraphRelation>,
        #[arg(long, value_enum)]
        direction: Option<CliGraphDirection>,
    },
    #[command(
        about = "Validate one AgentDoc patch document against graph artifacts.",
        after_long_help = PATCH_LONG_HELP
    )]
    Patch {
        /// Patch JSON document to validate.
        #[arg(long, value_name = "PATCH_JSON")]
        check: PathBuf,
        #[arg(
            long,
            help = "Graph JSON artifact path (default: config outputs.graph, then dist/docs.graph.json)"
        )]
        artifact: Option<PathBuf>,
    },
    #[command(
        about = "Diff Knowledge Objects between a git ref and the working tree.",
        after_long_help = DIFF_LONG_HELP
    )]
    Diff {
        /// Base git ref to diff against. The current working tree is the head.
        #[arg(value_name = "BASE_REF")]
        base_ref: String,
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
            help = "Graph JSON artifact path (default: config outputs.graph, then dist/docs.graph.json)"
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
        #[arg(long)]
        related_to: Option<String>,
        #[arg(long, value_enum, requires = "related_to")]
        relation: Option<CliGraphRelation>,
        #[arg(long, value_enum, requires = "related_to")]
        direction: Option<CliGraphDirection>,
        #[arg(long, default_value = "10")]
        top: NonZeroUsize,
    },
}
