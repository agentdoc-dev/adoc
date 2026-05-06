//! Pins the documented public surface of `adoc-core` per ADR-0005.
//!
//! Failing to compile this test means an item that was previously importable
//! by name has been hidden — or, more concerningly, that lib.rs has gained a
//! new `pub use` that nobody reviewed. Both are signals to update ADR-0005
//! before merging.

use std::path::PathBuf;

use adoc_core::{
    AgentJsonDocument, AgentJsonObject, AgentJsonRelations, AgentJsonSourceSpan, BuildArtifacts,
    CompileInput, CompileResult, Diagnostic, DiagnosticCode, Severity, compile_workspace,
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
    // The wire string remains available for hosts that serialize manually.
    let _: &'static str = DiagnosticCode::ParseRawHtml.as_str();
    assert_eq!(
        DiagnosticCode::SchemaMissingField.as_str(),
        "schema.missing_field"
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
}
