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
    BuildInput, BuildOutcome, BuildOutputs, BuildUseCase, CheckInput, CheckOutcome, CheckUseCase,
    ContradictionsInput, ContradictionsOutcome, ContradictionsUseCase, DiffInput, DiffOutcome,
    DiffUseCase, GraphInput, GraphOutcome, GraphUseCase, ImpactedChangedSet, ImpactedInput,
    ImpactedOutcome, ImpactedUseCase, InitInput, InitOutcome, InitUseCase, PatchCheckInput,
    PatchCheckOutcome, PatchCheckUseCase, ProjectArtifactLoadStatus, ProjectArtifactStatus,
    ProjectStatusArtifacts, ProjectStatusConfig, ProjectStatusInput, ProjectStatusOutcome,
    ProjectStatusPaths, ProjectStatusReadiness, ProjectStatusRefresh, ProjectStatusRefreshReport,
    ProjectStatusUseCase, ResolvedRetrievalRecord, ReviewInput, ReviewOutcome, ReviewPatchSource,
    ReviewUseCase, SearchInput, SearchOutcome, SearchUseCase, StaleInput, StaleOutcome,
    StaleUseCase, WhyInput, WhyOutcome, WhyUseCase,
};
