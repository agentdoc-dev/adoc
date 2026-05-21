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
    GraphInput, GraphOutcome, GraphUseCase, InitInput, InitOutcome, InitUseCase, PatchCheckInput,
    PatchCheckOutcome, PatchCheckUseCase, ProjectArtifactLoadStatus, ProjectArtifactStatus,
    ProjectStatusArtifacts, ProjectStatusConfig, ProjectStatusInput, ProjectStatusOutcome,
    ProjectStatusPaths, ProjectStatusReadiness, ProjectStatusRefresh, ProjectStatusRefreshReport,
    ProjectStatusUseCase, ResolvedRetrievalRecord, SearchInput, SearchOutcome, SearchUseCase,
    WhyInput, WhyOutcome, WhyUseCase,
};
