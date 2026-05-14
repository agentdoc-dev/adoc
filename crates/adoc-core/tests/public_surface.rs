//! Pins the documented public surface of `adoc-core` per ADR-0005.
//!
//! Failing to compile this test means an item that was previously importable
//! by name has been hidden — or, more concerningly, that lib.rs has gained a
//! new `pub use` that nobody reviewed. Both are signals to update ADR-0005
//! before merging.

use std::num::NonZeroUsize;
use std::path::PathBuf;

use adoc_core::{
    AgentJsonDocument, AgentJsonObject, AgentJsonRelations, AgentJsonSourceSpan, BuildArtifacts,
    CompileInput, CompileResult, Diagnostic, DiagnosticCode, GraphArtifactDocument, GraphDirection,
    GraphEdge, GraphInput, GraphLoadResult, GraphNode, GraphRelationKind, GraphSession,
    GraphTraversalEnvelope, GraphTraversalQuery, GraphTraversalResult, RetrievalEnvelope,
    RetrievalInput, RetrievalLoadResult, RetrievalMatch, RetrievalRecord, RetrievalSession,
    RetrievalSource, SearchFilters, SearchMode, SearchQuery, SearchResult, Severity, WhyResult,
    compile_workspace, load_graph_session, load_retrieval_session, search, traverse_graph,
    why_object,
};

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
        // Both BuildArtifacts fields are publicly readable.
        let _: String = artifacts.html;
        let _: AgentJsonDocument = artifacts.agent_json;
        let _: GraphArtifactDocument = artifacts.graph_json;
    }

    // AgentJsonObject and its sub-types are part of the public surface.
    let _: AgentJsonObject = AgentJsonObject {
        id: String::new(),
        kind: String::new(),
        status: Some(String::new()),
        body: String::new(),
        page_id: String::new(),
        source_span: AgentJsonSourceSpan {
            path: String::new(),
            line: 0,
            column: 0,
        },
        fields: std::collections::BTreeMap::new(),
        relations: AgentJsonRelations::default(),
    };
    let _: AgentJsonRelations = AgentJsonRelations::default();
    let _: AgentJsonSourceSpan = AgentJsonSourceSpan {
        path: String::new(),
        line: 0,
        column: 0,
    };

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
    let _ = DiagnosticCode::GraphHashDrift;
    let _ = DiagnosticCode::GraphObjectNotFound;
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
    assert_eq!(DiagnosticCode::GraphHashDrift.as_str(), "graph.hash_drift");
    assert_eq!(
        DiagnosticCode::GraphObjectNotFound.as_str(),
        "graph.object_not_found"
    );

    let graph_doc = GraphArtifactDocument {
        schema_version: "adoc.graph.v0".to_string(),
        agent_artifact_hash: "sha256:agent".to_string(),
        nodes: vec![GraphNode {
            id: "billing.credits".to_string(),
            kind: "claim".to_string(),
            status: Some("verified".to_string()),
            page_id: "team.billing".to_string(),
        }],
        edges: vec![GraphEdge {
            source: "billing.root".to_string(),
            target: "billing.credits".to_string(),
            relation: GraphRelationKind::DependsOn,
        }],
    };
    let _: GraphArtifactDocument = graph_doc;
    let _: GraphRelationKind = GraphRelationKind::Supersedes;
    let _: GraphDirection = GraphDirection::Both;
    let _: GraphInput = GraphInput {
        agent_artifact_path: PathBuf::from("/missing-docs-agent-json-for-surface-test"),
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
        artifact_path: PathBuf::from("/missing-docs-agent-json-for-surface-test"),
        search_artifact_path: None,
        graph_artifact_path: None,
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
        relations: AgentJsonRelations::default(),
        search_match: None,
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
    };
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
    ] {
        assert!(
            !lib_rs.contains(removed),
            "adoc-core must not re-export CLI-only retrieval presentation/service type `{removed}`"
        );
    }
}
