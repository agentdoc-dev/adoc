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
        status: String::new(),
        body: String::new(),
        page_id: String::new(),
        source_span: AgentJsonSourceSpan {
            path: PathBuf::new(),
            line: 0,
            column: 0,
        },
        fields: std::collections::BTreeMap::new(),
        relations: AgentJsonRelations::default(),
    };
    let _: AgentJsonRelations = AgentJsonRelations::default();
    let _: AgentJsonSourceSpan = AgentJsonSourceSpan {
        path: PathBuf::new(),
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
    let _ = DiagnosticCode::ParseUnknownBlockType;
    let _ = DiagnosticCode::ParseMalformedField;
    let _ = DiagnosticCode::ParseMalformedOpenFence;
    let _ = DiagnosticCode::SchemaMissingField;
    let _ = DiagnosticCode::SchemaDuplicateField;
    let _ = DiagnosticCode::IdInvalid;
    let _ = DiagnosticCode::IoUnreadableFile;
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
        DiagnosticCode::ParseUnknownBlockType.as_str(),
        "parse.unknown_block_type"
    );
    assert_eq!(
        DiagnosticCode::ParseMalformedField.as_str(),
        "parse.malformed_field"
    );
    assert_eq!(
        DiagnosticCode::ParseMalformedOpenFence.as_str(),
        "parse.malformed_open_fence"
    );
}
