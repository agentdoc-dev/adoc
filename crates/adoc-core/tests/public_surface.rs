//! Pins the documented public surface of `adoc-core` per ADR-0005.
//!
//! Failing to compile this test means an item that was previously importable
//! by name has been hidden — or, more concerningly, that lib.rs has gained a
//! new `pub use` that nobody reviewed. Both are signals to update ADR-0005
//! before merging.

use std::path::PathBuf;

use adoc_core::{
    AgentJsonDocument, AgentJsonObject, AgentJsonRelations, AgentJsonSourceSpan, BuildArtifacts,
    CompileInput, CompileResult, Diagnostic, DiagnosticCode, ExplainResult, JsonRetrievalFormatter,
    RetrievalEnvelope, RetrievalFormatError, RetrievalFormatter, RetrievalInput,
    RetrievalLoadResult, RetrievalMatch, RetrievalRecord, RetrievalSession, RetrievalSource,
    SearchFilters, SearchMode, SearchQuery, SearchResult, Severity, TextRetrievalFormatter,
    compile_workspace, explain_object, load_retrieval_session,
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

    let _: fn(RetrievalInput) -> RetrievalLoadResult = load_retrieval_session;
    let _: RetrievalInput = RetrievalInput {
        artifact_path: PathBuf::from("/missing-docs-agent-json-for-surface-test"),
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
    let _: SearchFilters = SearchFilters::default();
    let _: SearchQuery = SearchQuery {
        text: String::from("credits"),
        mode: SearchMode::Lexical,
        filters: SearchFilters::default(),
    };
    let _: RetrievalMatch = RetrievalMatch::lexical(1);
    let search_result = SearchResult {
        records: Vec::new(),
        diagnostics: Vec::new(),
    };
    let _: RetrievalEnvelope = RetrievalEnvelope::from(search_result);

    let explain_result = ExplainResult {
        records: Vec::new(),
        diagnostics: Vec::new(),
    };
    let _explain_records: Vec<RetrievalRecord> = explain_result.records;
    let _explain_diagnostics: Vec<Diagnostic> = explain_result.diagnostics;

    let _: fn(&RetrievalSession, &str) -> ExplainResult = explain_object;

    let envelope = RetrievalEnvelope::new(Vec::new(), Vec::new());
    let text_formatter = TextRetrievalFormatter;
    let json_formatter = JsonRetrievalFormatter;
    let _: Result<String, RetrievalFormatError> = text_formatter.render(&envelope);
    let _: Result<String, RetrievalFormatError> = json_formatter.render(&envelope);
}
