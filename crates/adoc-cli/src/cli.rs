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
  adoc stale --within 30d
  adoc contradictions --all
  adoc impacted-by --ref main
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
--check validates without writing; --apply validates, then rewrites exactly
the targeted source spans (formatting-preserving), re-checks, and reports.
Apply writes to the working tree only and never auto-reverts: review the
result with git diff, undo with git checkout. After a successful apply the
graph artifact is stale — run `adoc build`.

Apply exit codes: 0 applied and post-check clean; 1 refused, nothing written;
2 applied but the post-check found new errors (stop and review).

Examples:
  adoc patch --check patch.json
  adoc patch --check patch.json --artifact dist/docs.graph.json
  adoc patch --check patch.json --format json
  adoc patch --apply patch.json
  cat patch.json | adoc patch --apply @- --format json
";
const DIFF_LONG_HELP: &str = "\
Examples:
  adoc diff main
  adoc diff main --format json
  adoc diff main --format markdown
  adoc diff HEAD~1
";
const REVIEW_LONG_HELP: &str = "\
Examples:
  adoc review main
  adoc review main --format json
  adoc review main --format markdown
  adoc review HEAD~1
";
const SEARCH_LONG_HELP: &str = "\
Search returns one blended, RRF-ranked list of Knowledge Object and prose
records (adoc.retrieval.v1). Object ID pins stay on top and ride above the
--top budget (--top bounds scored hits; pinned ids are always included);
prose records are orientation context, never citable knowledge. Setting any Knowledge Object
metadata filter (--kind, --status, --owner, --source-path, --related-to)
implies object intent and suppresses prose records.

Examples:
  adoc search \"refund policy\"
  adoc search \"refund policy\" --kind claim --top 5
  adoc search \"refund policy\" --related-to billing.refunds.issue-credit --relation depends_on
  adoc search billing.refunds --lexical
  adoc search \"getting started\" --prose-only
  adoc search \"refund policy\" --objects-only --format json
";
const STALE_LONG_HELP: &str = "\
Staleness and review-overdue-ness are re-derived from authored fields at the
time of the query, not read from the artifact's build-time projection. The
command is a query, not a gate: it exits 0 whether or not records exist.

Examples:
  adoc stale
  adoc stale --within 30d
  adoc stale --within 30d --format json
  adoc stale --artifact dist/docs.graph.json
";
const CONTRADICTIONS_LONG_HELP: &str = "\
Lists every unresolved contradiction plus every contradicted claim — implicated
by an unresolved contradiction or authored as contradicted — with the
contradiction ids that implicate it, so consumers never join the two lists.
The output is a pure function of the graph artifact (no clock). The command is
a query, not a gate: it exits 0 whether or not findings exist.

Examples:
  adoc contradictions
  adoc contradictions --all
  adoc contradictions --format json
  adoc contradictions --artifact dist/docs.graph.json
";

const IMPACTED_BY_LONG_HELP: &str = "\
Answers \"this code changed — which knowledge is now suspect?\" over the graph
artifact: verified claims and accepted decisions whose declared impacts: paths
or evidence paths (inline source_code/test values, or the path of a referenced
source object) exactly match a changed file. No recompile, no globs. The
command is a query, not a gate: it exits 0 whether or not anything is
impacted.

Exactly one input shape: explicit changed paths, or --ref <git-ref> to derive
the changed set from git (the base ref against the working tree, the same
shape as `adoc review <ref>`).

On input or environment errors (exit 1/2), --format json still emits a valid
envelope with the diagnostics; --format markdown writes a blockquote error
to stdout; plain/styled write fix-oriented messages to stderr only. Use
--format json for unattended runs.

Examples:
  adoc impacted-by crates/billing/src/refund.rs
  adoc impacted-by src/a.rs src/b.rs --format json
  adoc impacted-by --ref main
  adoc impacted-by --ref main --format markdown
";

/// Parse the `--within <N>d` horizon grammar (same `[0-9]+d` shape as the
/// `review_interval:` field) into a day count.
fn parse_within_days(value: &str) -> Result<u32, String> {
    let error = || format!("expected a day count like `30d`, got `{value}`");
    let days = value.strip_suffix('d').ok_or_else(error)?;
    if days.is_empty() || !days.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(error());
    }
    days.parse::<u32>().map_err(|_| error())
}

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
    /// PR-comment-ready GitHub-flavored Markdown (only supported by
    /// `adoc diff`, `adoc review`, and `adoc impacted-by`).
    Markdown,
}

impl From<CliFormat> for FormatChoice {
    fn from(f: CliFormat) -> Self {
        match f {
            CliFormat::Auto => Self::Auto,
            CliFormat::Plain => Self::Plain,
            CliFormat::Styled => Self::Styled,
            CliFormat::Json => Self::Json,
            CliFormat::Markdown => Self::Markdown,
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
        about = "List stale, review-overdue, and expiring Knowledge Objects from graph artifacts.",
        after_long_help = STALE_LONG_HELP
    )]
    Stale {
        #[arg(
            long,
            help = "Graph JSON artifact path (default: config outputs.graph, then dist/docs.graph.json)"
        )]
        artifact: Option<PathBuf>,
        /// Additionally list verified objects expiring within the next N days,
        /// e.g. `--within 30d`.
        #[arg(long, value_name = "Nd", value_parser = parse_within_days)]
        within: Option<u32>,
    },
    #[command(
        about = "List unresolved contradictions and contradicted claims from graph artifacts.",
        after_long_help = CONTRADICTIONS_LONG_HELP
    )]
    Contradictions {
        #[arg(
            long,
            help = "Graph JSON artifact path (default: config outputs.graph, then dist/docs.graph.json)"
        )]
        artifact: Option<PathBuf>,
        /// Include resolved and dismissed contradictions in the listing.
        #[arg(long)]
        all: bool,
    },
    #[command(
        name = "impacted-by",
        about = "List Knowledge Objects implicated by changed source paths.",
        after_long_help = IMPACTED_BY_LONG_HELP
    )]
    ImpactedBy {
        /// Changed repo-relative file paths (as emitted by
        /// `git diff --name-only`). Mutually exclusive with `--ref`.
        #[arg(
            value_name = "PATH",
            required_unless_present = "git_ref",
            conflicts_with = "git_ref"
        )]
        paths: Vec<String>,
        /// Derive the changed set from git: the base ref against the working
        /// tree (the `adoc review <ref>` shape).
        #[arg(long = "ref", value_name = "GIT_REF")]
        git_ref: Option<String>,
        #[arg(
            long,
            help = "Graph JSON artifact path (default: config outputs.graph, then dist/docs.graph.json)"
        )]
        artifact: Option<PathBuf>,
    },
    #[command(
        about = "Validate one AgentDoc patch document against graph artifacts, or apply it to source.",
        after_long_help = PATCH_LONG_HELP
    )]
    Patch {
        /// Patch JSON document to validate (read-only).
        #[arg(
            long,
            value_name = "PATCH_JSON",
            conflicts_with = "apply",
            required_unless_present = "apply"
        )]
        check: Option<PathBuf>,
        /// Patch JSON document to validate and apply to AgentDoc source.
        /// Pass a path, or `@-` to read the document from stdin.
        #[arg(long, value_name = "PATCH_JSON_OR_@-")]
        apply: Option<String>,
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
        about = "Review Knowledge Object changes with source-path impact and required reviewers.",
        after_long_help = REVIEW_LONG_HELP
    )]
    Review {
        /// Base git ref to review against. The current working tree is the head.
        #[arg(value_name = "BASE_REF")]
        base_ref: String,
        /// Optional adoc.patch.v0 JSON file to validate against the head graph.
        /// When supplied, the review envelope embeds an adoc.patch.check.v0
        /// report and unions patch-driven proof obligations into the
        /// top-level obligation list.
        #[arg(long, value_name = "PATCH_JSON")]
        patch: Option<PathBuf>,
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
        /// Force deterministic BM25 + Object ID ranking, skipping vectors.
        /// Hybrid fusion is the default when neither --semantic nor
        /// --lexical is set.
        #[arg(long, conflicts_with = "semantic")]
        lexical: bool,
        /// Return only Knowledge Object records (the pre-V1.7 result set).
        #[arg(long, conflicts_with = "prose_only")]
        objects_only: bool,
        /// Return only prose records. Prose has no Knowledge Object metadata,
        /// so this conflicts with the metadata filters. Semantic prose search
        /// works since V1.7.2 (adoc.search.v1 prose vectors).
        #[arg(
            long,
            conflicts_with_all = [
                "objects_only", "kind", "status", "owner",
                "source_path", "related_to",
            ]
        )]
        prose_only: bool,
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
