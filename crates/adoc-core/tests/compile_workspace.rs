mod support;

use adoc_core::{CompileInput, DiagnosticCode, compile_workspace};
use support::TestWorkspace;

#[test]
fn compile_workspace_returns_mixed_validation_diagnostics_in_source_order() {
    let workspace = TestWorkspace::new("diagnostic-source-order");
    let source = workspace.write(
        "guide.adoc",
        "# Guide @doc(team.guide)\n\nsee [bad](javascript:alert) first\n\n<div>raw</div>\n",
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(
        result.has_errors(),
        "invalid source should fail compilation"
    );
    let codes: Vec<_> = result
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect();
    assert_eq!(
        codes,
        [
            DiagnosticCode::ParseUnsafeLink,
            DiagnosticCode::ParseRawHtml,
        ],
        "diagnostics should be ordered by source position"
    );
}

#[test]
fn compile_workspace_rejects_invalid_explicit_page_id() {
    let workspace = TestWorkspace::new("invalid-explicit-page-id");
    let source = workspace.write(
        "guide.adoc",
        "# Guide @doc(guide)\n\nSingle-segment page IDs are invalid.\n",
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(
        result.has_errors(),
        "invalid page ID should fail compilation"
    );
    assert!(result.artifacts.is_none(), "errors must block artifacts");
    assert_eq!(result.diagnostics.len(), 1);
    let diagnostic = &result.diagnostics[0];
    assert_eq!(diagnostic.code, DiagnosticCode::IdInvalid);
    assert_eq!(
        diagnostic
            .span
            .as_ref()
            .map(|span| (span.start.line, span.start.column)),
        Some((1, 14)),
        "diagnostic should point at the invalid id value"
    );
}

#[test]
fn compile_workspace_rejects_invalid_path_derived_page_id() {
    let workspace = TestWorkspace::new("invalid-path-derived-page-id");
    let source = workspace.write(
        "guide.adoc",
        "# Guide\n\nA single file name derives a single-segment page ID.\n",
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(
        result.has_errors(),
        "invalid derived page ID should fail compilation"
    );
    assert!(result.artifacts.is_none(), "errors must block artifacts");
    assert_eq!(result.diagnostics.len(), 1);
    let diagnostic = &result.diagnostics[0];
    assert_eq!(diagnostic.code, DiagnosticCode::IdInvalid);
    assert!(
        diagnostic.message.contains("guide"),
        "diagnostic should quote the invalid derived ID: {}",
        diagnostic.message
    );
    assert_eq!(
        diagnostic
            .span
            .as_ref()
            .map(|span| (span.start.line, span.start.column)),
        Some((1, 1)),
        "path-derived identity diagnostics should point at the file start"
    );
}
