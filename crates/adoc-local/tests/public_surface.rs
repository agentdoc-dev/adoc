use std::num::NonZeroUsize;
use std::path::PathBuf;

use adoc_core::{GraphDirection, GraphRelationKind};
use adoc_local::{
    BuildInput, BuildUseCase, CheckInput, CheckUseCase, GraphInput, GraphUseCase, InitInput,
    InitUseCase, LocalContext, PatchCheckInput, PatchCheckUseCase, SearchInput, SearchUseCase,
    UnrestrictedPathPolicy, WhyInput, WhyUseCase,
};

#[test]
fn local_public_surface_is_use_case_oriented() {
    let context = LocalContext::new(PathBuf::from("."), UnrestrictedPathPolicy);

    let _: InitUseCase<_> = InitUseCase::new(context.clone());
    let _: CheckUseCase<_> = CheckUseCase::new(context.clone());
    let _: BuildUseCase<_> = BuildUseCase::new(context.clone());
    let _: WhyUseCase<_> = WhyUseCase::new(context.clone());
    let _: GraphUseCase<_> = GraphUseCase::new(context.clone());
    let _: SearchUseCase<_> = SearchUseCase::new(context.clone());
    let _: PatchCheckUseCase<_> = PatchCheckUseCase::new(context);

    let _: InitInput = InitInput;
    let _: CheckInput = CheckInput { path: None };
    let _: BuildInput = BuildInput {
        path: None,
        out: None,
        no_embeddings: true,
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
    };
    let _: PatchCheckInput = PatchCheckInput {
        patch_path: PathBuf::from("patch.json"),
        artifact: None,
    };
}
