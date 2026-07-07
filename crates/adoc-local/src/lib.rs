//! Local AgentDoc workflow services shared by CLI and protocol adapters.

mod config;
mod context;
mod error;
mod path_policy;
mod use_cases;

pub use config::{ConfigOutputs, EmbeddingsProvider, ProjectConfig};
pub use context::LocalContext;
pub use error::LocalError;
pub use path_policy::{PathPolicy, ProjectRootPathPolicy, UnrestrictedPathPolicy};
pub use use_cases::{
    BuildInput, BuildOutcome, BuildOutputs, CheckInput, CheckOutcome, ContradictionsInput,
    ContradictionsOutcome, DiffInput, DiffOutcome, GraphInput, GraphOutcome, ImpactedChangedSet,
    ImpactedInput, ImpactedOutcome, InitOutcome, MigrateInput, MigrateOutcome, MigrateReport,
    MigrateReportFile, PatchApplyInput, PatchApplyOutcome, PatchApplySource, PatchCheckInput,
    PatchCheckOutcome, ProjectArtifactLoadStatus, ProjectArtifactStatus, ProjectStatusArtifacts,
    ProjectStatusConfig, ProjectStatusInput, ProjectStatusOutcome, ProjectStatusPaths,
    ProjectStatusReadiness, ProjectStatusRefresh, ProjectStatusRefreshReport,
    ResolvedRetrievalRecord, ResolvedSearchEntry, ReviewInput, ReviewOutcome, ReviewPatchSource,
    SearchInput, SearchOutcome, StaleInput, StaleOutcome, WhyInput, WhyOutcome,
};
