use std::num::NonZeroUsize;
use std::path::PathBuf;

use adoc_core::{GraphDirection, GraphRelationKind, SearchRecordScope};
use adoc_local::{
    BuildInput, BuildOutcome, CheckInput, CheckOutcome, GraphInput, GraphOutcome, InitOutcome,
    LocalContext, LocalError, PatchCheckInput, PatchCheckOutcome, ProjectStatusInput,
    ProjectStatusOutcome, ProjectStatusRefresh, SearchInput, SearchOutcome, UnrestrictedPathPolicy,
    WhyInput, WhyOutcome,
};

type Ctx = LocalContext<UnrestrictedPathPolicy>;

/// Pins the Local Workflow Layer's public surface: one method per local
/// operation on `LocalContext`, taking the operation's `*Input` record and
/// returning its `*Outcome`. Callers hold exactly one type; the former
/// per-operation `*UseCase` wrapper structs are gone.
#[test]
fn local_public_surface_is_method_oriented() {
    let _: fn(&Ctx) -> Result<InitOutcome, LocalError> = Ctx::init;
    let _: fn(&Ctx, CheckInput) -> Result<CheckOutcome, LocalError> = Ctx::check;
    let _: fn(&Ctx, BuildInput) -> Result<BuildOutcome, LocalError> = Ctx::build;
    let _: fn(&Ctx, WhyInput) -> Result<WhyOutcome, LocalError> = Ctx::why;
    let _: fn(&Ctx, GraphInput) -> Result<GraphOutcome, LocalError> = Ctx::graph;
    let _: fn(&Ctx, SearchInput) -> Result<SearchOutcome, LocalError> = Ctx::search;
    let _: fn(&Ctx, PatchCheckInput) -> Result<PatchCheckOutcome, LocalError> = Ctx::patch_check;
    let _: fn(&Ctx, ProjectStatusInput) -> Result<ProjectStatusOutcome, LocalError> =
        Ctx::project_status;

    let _: Ctx = LocalContext::new(PathBuf::from("."), UnrestrictedPathPolicy);

    let _: CheckInput = CheckInput {
        path: None,
        as_of: None,
    };
    let _: BuildInput = BuildInput {
        path: None,
        out: None,
        no_embeddings: true,
        as_of: None,
    };
    let _: WhyInput = WhyInput {
        object_id: "billing.ready".to_string(),
        artifact: None,
    };
    let _: GraphInput = GraphInput {
        object_id: "billing.ready".to_string(),
        artifact: None,
        relation: Some(GraphRelationKind::DependsOn),
        direction: Some(GraphDirection::Outgoing),
    };
    let _: SearchInput = SearchInput {
        query: "billing".to_string(),
        artifact: None,
        search_artifact: None,
        semantic: false,
        lexical: true,
        kind: None,
        status: None,
        owner: None,
        source_path: None,
        related_to: None,
        relation: None,
        direction: None,
        top: NonZeroUsize::new(5).expect("nonzero"),
        scope: SearchRecordScope::Blended,
    };
    let _: PatchCheckInput = PatchCheckInput {
        patch_path: PathBuf::from("patch.json"),
        artifact: None,
        as_of: None,
    };
    let _: ProjectStatusInput = ProjectStatusInput {
        refresh: ProjectStatusRefresh::None,
        no_embeddings: true,
    };
    let _: ProjectStatusRefresh = ProjectStatusRefresh::Check;
    let _: ProjectStatusRefresh = ProjectStatusRefresh::Build;
    let _: adoc_local::EmbeddingsProvider = adoc_local::EmbeddingsProvider::Deterministic;
}
