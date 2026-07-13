//! Pins the documented public surface of `adoc-core` per ADR-0005.
//!
//! Failing to compile this test means an item that was previously importable
//! by name has been hidden — or, more concerningly, that lib.rs has gained a
//! new `pub use` that nobody reviewed. Both are signals to update ADR-0005
//! before merging.

use std::num::NonZeroUsize;
use std::path::PathBuf;

use adoc_core::{
    AffectedRelation, BuildArtifacts, CompileInput, CompileResult, Diagnostic, DiagnosticCode,
    GraphDirection, GraphInput, GraphLoadResult, GraphRelationKind, GraphSession,
    GraphTraversalEnvelope, GraphTraversalQuery, GraphTraversalResult, PATCH_CHECK_SCHEMA_VERSION,
    PatchCheckResult, PatchDiff, PatchInput, PatchJsonInput, PatchOperation, ProofObligation,
    ProseBlockKind, ProseRecord, RetrievalEntry, RetrievalEnvelope, RetrievalInput,
    RetrievalLoadResult, RetrievalMatch, RetrievalRecord, RetrievalRelations, RetrievalSession,
    RetrievalSource, SearchFilters, SearchMode, SearchQuery, SearchRecordScope, SearchResult,
    Severity, WhyResult, check_patch, check_patch_json, compile_workspace, load_graph_session,
    load_retrieval_session, search, traverse_graph, why_object,
};

#[test]
fn patch_application_layer_does_not_reference_infrastructure() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let patch_source =
        std::fs::read_to_string(manifest_dir.join("src/application/patch.rs")).unwrap();

    assert!(
        !patch_source.contains("crate::infrastructure"),
        "application/patch.rs must stay independent from infrastructure adapters"
    );
}

#[test]
fn review_and_apply_do_not_round_trip_compiler_graph_json() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for relative in ["src/application/review.rs", "src/application/apply.rs"] {
        let source = std::fs::read_to_string(manifest_dir.join(relative)).unwrap();
        let production = source.split("#[cfg(test)]").next().unwrap_or(&source);
        assert!(
            !production.contains("serde_json::from_str"),
            "{relative} must consume the compiler's typed projection instead of reparsing graph JSON"
        );
    }
}

#[test]
fn public_surface_compiles_with_only_documented_imports() {
    // Construct a CompileInput so the type is exercised, not just imported.
    let input = CompileInput {
        root: PathBuf::from("/nonexistent-for-surface-test"),
    };
    let result: CompileResult = compile_workspace(input);

    // The diagnostics field is reachable as Vec<Diagnostic>.
    let _diagnostics: &Vec<Diagnostic> = &result.diagnostics;
    // The artifacts field is Option<BuildArtifacts>; either branch typechecks.
    let _artifacts: &Option<BuildArtifacts> = &result.artifacts;
    if let Some(artifacts) = result.artifacts {
        // BuildArtifacts fields are publicly readable.
        let _: String = artifacts.html;
        let _: String = artifacts.graph_json;
        let _: Option<String> = artifacts.search_json;
    }

    // Severity discriminants are reachable as documented.
    let _ = Severity::Error;
    let _ = Severity::Warning;
    let _ = Severity::Info;

    // DiagnosticCode discriminants are reachable as documented (ADR-0005 v0.x).
    let _ = DiagnosticCode::ParseRawHtml;
    let _ = DiagnosticCode::ParseUnsafeLink;
    let _ = DiagnosticCode::ParseUnclosedFence;
    let _ = DiagnosticCode::ParseMalformedPageAnnotation;
    let _ = DiagnosticCode::ParseNestedTypedBlock;
    let _ = DiagnosticCode::ParseMalformedField;
    let _ = DiagnosticCode::ParseMalformedOpenFence;
    let _ = DiagnosticCode::SchemaUnknownKind;
    let _ = DiagnosticCode::SchemaMissingField;
    let _ = DiagnosticCode::SchemaDuplicateField;
    let _ = DiagnosticCode::SchemaInvalidStatus;
    let _ = DiagnosticCode::ClaimVerifiedMissingEvidence;
    let _ = DiagnosticCode::ClaimStatusCasing;
    let _ = DiagnosticCode::LifecycleExpired;
    let _ = DiagnosticCode::LifecycleInvalidExpiresAt;
    let _ = DiagnosticCode::IdDuplicate;
    let _ = DiagnosticCode::IdInvalid;
    let _ = DiagnosticCode::RefBroken;
    let _ = DiagnosticCode::IoUnreadableFile;
    let _ = DiagnosticCode::IoUnreadableDirectory;
    let _ = DiagnosticCode::IoUnsupportedSourceExtension;
    let _ = DiagnosticCode::IoArtifactMissing;
    let _ = DiagnosticCode::IoArtifactUnreadable;
    let _ = DiagnosticCode::IoArtifactMalformed;
    let _ = DiagnosticCode::SchemaUnsupportedVersion;
    let _ = DiagnosticCode::IdDuplicateInArtifact;
    let _ = DiagnosticCode::RetrievalObjectNotFound;
    let _ = DiagnosticCode::SearchInvalidFilter;
    let _ = DiagnosticCode::BuildEmbeddingsCacheIgnored;
    let _ = DiagnosticCode::BuildArtifactSerializationFailed;
    let _ = DiagnosticCode::GraphObjectNotFound;
    let _ = DiagnosticCode::PatchInvalidDocument;
    let _ = DiagnosticCode::PatchValidationFailed;
    let _ = DiagnosticCode::PatchBaseHashMismatch;
    let _ = DiagnosticCode::PatchTargetAlreadyExists;
    let _ = DiagnosticCode::PatchPlacementInvalid;
    // The wire string remains available for hosts that serialize manually.
    let _: &'static str = DiagnosticCode::ParseRawHtml.as_str();
    let _: &'static str = DiagnosticCode::ParseRawHtml.default_help();
    assert_eq!(
        DiagnosticCode::SchemaMissingField.as_str(),
        "schema.missing_field"
    );
    assert_eq!(
        DiagnosticCode::IdDuplicate.default_help(),
        "Give each object a unique ID across the compiled workspace."
    );
    assert_eq!(
        DiagnosticCode::SchemaDuplicateField.as_str(),
        "schema.duplicate_field"
    );
    assert_eq!(
        DiagnosticCode::SchemaInvalidStatus.as_str(),
        "schema.invalid_status"
    );
    assert_eq!(
        DiagnosticCode::ClaimVerifiedMissingEvidence.as_str(),
        "claim.verified_missing_evidence"
    );
    assert_eq!(
        DiagnosticCode::ClaimStatusCasing.as_str(),
        "claim.status_casing"
    );
    assert_eq!(
        DiagnosticCode::LifecycleExpired.as_str(),
        "lifecycle.expired"
    );
    assert_eq!(
        DiagnosticCode::LifecycleInvalidExpiresAt.as_str(),
        "lifecycle.invalid_expires_at"
    );
    assert_eq!(DiagnosticCode::IdDuplicate.as_str(), "id.duplicate");
    assert_eq!(DiagnosticCode::RefBroken.as_str(), "ref.broken");
    assert_eq!(
        DiagnosticCode::IoUnreadableDirectory.as_str(),
        "io.unreadable_directory"
    );
    assert_eq!(
        DiagnosticCode::ParseNestedTypedBlock.as_str(),
        "parse.nested_typed_block"
    );
    assert_eq!(
        DiagnosticCode::SchemaUnknownKind.as_str(),
        "schema.unknown_kind"
    );
    assert_eq!(
        DiagnosticCode::ParseMalformedField.as_str(),
        "parse.malformed_field"
    );
    assert_eq!(
        DiagnosticCode::ParseMalformedOpenFence.as_str(),
        "parse.malformed_open_fence"
    );
    assert_eq!(
        DiagnosticCode::IoUnsupportedSourceExtension.as_str(),
        "io.unsupported_source_extension"
    );
    assert_eq!(
        DiagnosticCode::IoArtifactMissing.as_str(),
        "io.artifact_missing"
    );
    assert_eq!(
        DiagnosticCode::IoArtifactUnreadable.as_str(),
        "io.artifact_unreadable"
    );
    assert_eq!(
        DiagnosticCode::IoArtifactMalformed.as_str(),
        "io.artifact_malformed"
    );
    assert_eq!(
        DiagnosticCode::SchemaUnsupportedVersion.as_str(),
        "schema.unsupported_version"
    );
    assert_eq!(
        DiagnosticCode::IdDuplicateInArtifact.as_str(),
        "id.duplicate_in_artifact"
    );
    assert_eq!(
        DiagnosticCode::RetrievalObjectNotFound.as_str(),
        "retrieval.object_not_found"
    );
    assert_eq!(
        DiagnosticCode::SearchInvalidFilter.as_str(),
        "search.invalid_filter"
    );
    assert_eq!(
        DiagnosticCode::BuildEmbeddingsCacheIgnored.as_str(),
        "build.embeddings_cache_ignored"
    );
    assert_eq!(
        DiagnosticCode::BuildArtifactSerializationFailed.as_str(),
        "build.artifact_serialization_failed"
    );
    assert_eq!(
        DiagnosticCode::GraphObjectNotFound.as_str(),
        "graph.object_not_found"
    );
    assert_eq!(
        DiagnosticCode::PatchInvalidDocument.as_str(),
        "patch.invalid_document"
    );
    assert_eq!(
        DiagnosticCode::PatchInvalidDocument.default_help(),
        "Use the adoc.patch.v0 schema with exactly one supported operation and its required fields."
    );
    assert_eq!(
        DiagnosticCode::PatchValidationFailed.as_str(),
        "patch.validation_failed"
    );
    assert_eq!(
        DiagnosticCode::PatchValidationFailed.default_help(),
        "Adjust the patch intent so it satisfies AgentDoc patch validation rules."
    );
    assert_eq!(
        DiagnosticCode::PatchBaseHashMismatch.as_str(),
        "patch.base_hash_mismatch"
    );
    assert_eq!(
        DiagnosticCode::PatchBaseHashMismatch.default_help(),
        "Rebuild docs.graph.json or regenerate the patch against the current target content_hash."
    );
    assert_eq!(
        DiagnosticCode::PatchTargetAlreadyExists.as_str(),
        "patch.target_already_exists"
    );
    assert_eq!(
        DiagnosticCode::PatchTargetAlreadyExists.default_help(),
        "Use create_object only for a new Object ID, or choose an update operation for an existing object."
    );
    assert_eq!(
        DiagnosticCode::PatchPlacementInvalid.as_str(),
        "patch.placement_invalid"
    );
    assert_eq!(
        DiagnosticCode::PatchPlacementInvalid.default_help(),
        "Use an existing page_id and, when after is supplied, an object already on that page."
    );

    let _: GraphRelationKind = GraphRelationKind::Supersedes;
    let _: GraphDirection = GraphDirection::Both;
    let _: GraphInput = GraphInput {
        graph_artifact_path: PathBuf::from("/missing-docs-graph-json-for-surface-test"),
    };
    let graph_load = GraphLoadResult {
        session: None,
        diagnostics: Vec::new(),
    };
    let _graph_diagnostics: Vec<Diagnostic> = graph_load.diagnostics;
    let _maybe_graph_session: Option<GraphSession> = graph_load.session;
    let graph_traversal = GraphTraversalResult {
        root: "billing.root".to_string(),
        nodes: Vec::new(),
        edges: Vec::new(),
        diagnostics: Vec::new(),
    };
    let _: GraphTraversalEnvelope = GraphTraversalEnvelope::from(graph_traversal);
    let _: GraphTraversalQuery = GraphTraversalQuery {
        root_id: "billing.root".to_string(),
        direction: GraphDirection::Both,
        relations: Vec::new(),
    };

    let _: fn(RetrievalInput) -> RetrievalLoadResult = load_retrieval_session;
    let _: RetrievalInput = RetrievalInput {
        artifact_path: PathBuf::from("/missing-docs-graph-json-for-surface-test"),
        search_artifact_path: None,
    };
    let retrieval_result = RetrievalLoadResult {
        session: None,
        diagnostics: Vec::new(),
    };
    let _retrieval_diagnostics: Vec<Diagnostic> = retrieval_result.diagnostics;
    let _maybe_session: Option<RetrievalSession> = retrieval_result.session;

    let record = RetrievalRecord {
        id: String::new(),
        kind: String::new(),
        status: None,
        severity: None,
        trust: None,
        content_hash: String::new(),
        owner: None,
        verified_at: None,
        body: String::new(),
        source: RetrievalSource {
            path: String::new(),
            line: 0,
            column: 0,
        },
        evidence: std::collections::BTreeMap::new(),
        fields: std::collections::BTreeMap::new(),
        relations: RetrievalRelations::default(),
        search_match: None,
        effective_status: None,
        effective_reason: None,
        evidence_quality: None,
        resolved_questions: Vec::new(),
    };
    let _: RetrievalRecord = record;

    let _: SearchMode = SearchMode::Lexical;
    let _: SearchMode = SearchMode::Hybrid;
    let _: SearchFilters = SearchFilters {
        kind: None,
        status: None,
        owner: None,
        source_path: None,
        related_to: None,
        relation: None,
        direction: None,
    };
    let _: SearchQuery = SearchQuery {
        text: String::from("credits"),
        mode: SearchMode::Lexical,
        filters: SearchFilters::default(),
        top: NonZeroUsize::new(10).expect("non-zero top"),
        query_vector: None,
        scope: SearchRecordScope::Blended,
    };
    let _: SearchRecordScope = SearchRecordScope::ObjectsOnly;
    let _: SearchRecordScope = SearchRecordScope::ProseOnly;
    // V1.7.1 (ADR-0040): the discriminated retrieval entry and prose record.
    let prose_record = ProseRecord {
        id: String::from("guides.page#block-0001"),
        page_id: String::from("guides.page"),
        block_kind: ProseBlockKind::Paragraph,
        text: String::from("Credits burn on completion."),
        heading_context: Some(String::from("Billing basics")),
        source: RetrievalSource {
            path: String::from("docs/guide.md"),
            line: 3,
            column: 1,
        },
        search_match: None,
    };
    let entry: RetrievalEntry = RetrievalEntry::Prose(prose_record);
    let _: &str = entry.id();
    let _: Option<&RetrievalRecord> = entry.as_knowledge_object();
    let _: Option<&RetrievalMatch> = entry.search_match();
    let _: RetrievalMatch = RetrievalMatch::lexical(1, Some(1));
    let _: RetrievalMatch = RetrievalMatch::hybrid(1, 0.0312, Some(2), Some(1));
    let search_result = SearchResult {
        records: Vec::new(),
        diagnostics: Vec::new(),
    };
    let _: RetrievalEnvelope = RetrievalEnvelope::from(search_result);

    let why_result = WhyResult {
        records: Vec::new(),
        diagnostics: Vec::new(),
    };
    let _why_records: Vec<RetrievalRecord> = why_result.records;
    let _why_diagnostics: Vec<Diagnostic> = why_result.diagnostics;

    let _: fn(&RetrievalSession, &str) -> WhyResult = why_object;
    let _: fn(&RetrievalSession, SearchQuery) -> SearchResult = search;
    let _: fn(GraphInput) -> GraphLoadResult = load_graph_session;
    let _: fn(&GraphSession, GraphTraversalQuery) -> GraphTraversalResult = traverse_graph;

    let _: RetrievalEnvelope = RetrievalEnvelope::new(Vec::new(), Vec::new());

    let _: &'static str = PATCH_CHECK_SCHEMA_VERSION;
    let _: PatchOperation = PatchOperation::ReplaceBody;
    let _: PatchInput = PatchInput {
        graph_artifact_path: PathBuf::from("/missing-docs-graph-json-for-surface-test"),
        patch_path: PathBuf::from("/missing-patch-json-for-surface-test"),
    };
    let _: PatchJsonInput = PatchJsonInput {
        graph_artifact_path: PathBuf::from("/missing-docs-graph-json-for-surface-test"),
        patch: serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "revoke",
            "target": "billing.old",
            "base_hash": "sha256:old",
            "changes": {},
            "reason": "retired"
        }),
    };
    let patch_result = PatchCheckResult {
        schema_version: PATCH_CHECK_SCHEMA_VERSION,
        valid: false,
        accepted_for_review: false,
        target: None,
        operation: String::new(),
        diffs: vec![PatchDiff {
            field: "body".to_string(),
            old: Some(serde_json::json!("old")),
            new: Some(serde_json::json!("new")),
        }],
        affected_relations: vec![AffectedRelation {
            source: "billing.new".to_string(),
            relation: GraphRelationKind::Supersedes,
            target: "billing.old".to_string(),
            action: "add".to_string(),
        }],
        proof_obligations: vec![ProofObligation {
            object_id: "billing.claim".to_string(),
            reason: "review evidence".to_string(),
            required_evidence: vec!["source".to_string()],
        }],
        required_follow_up: Vec::new(),
        diagnostics: Vec::new(),
    };
    let _patch_diagnostics: Vec<Diagnostic> = patch_result.diagnostics;
    let _: fn(PatchInput) -> PatchCheckResult = check_patch;
    let _: fn(PatchJsonInput) -> PatchCheckResult = check_patch_json;
}

#[test]
fn retrieval_public_surface_does_not_reexport_cli_service_types() {
    let lib_rs = include_str!("../src/lib.rs");

    for removed in [
        "WhyService",
        "WhyView",
        "WhyError",
        "Clock",
        "RecordResolver",
        "ResolverError",
        "ExpiresInfo",
        "RenderMeta",
        "GraphArtifactDocument",
        "GraphNode",
        "GraphEdge",
        "GraphKnowledgeObjectNode",
        "GraphRelations",
        "GraphSourceSpan",
        "SearchArtifactDocument",
    ] {
        assert!(
            !lib_rs.contains(removed),
            "adoc-core must not re-export CLI-only retrieval presentation/service type `{removed}`"
        );
    }
}
