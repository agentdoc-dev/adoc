use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use adoc_core::{CompileInput, DiagnosticCode, compile_workspace};

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

struct TestWorkspace {
    root: PathBuf,
}

impl TestWorkspace {
    fn new(name: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock is after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("adoc-{name}-{nonce}"));
        fs::create_dir_all(&root).expect("test workspace can be created");
        Self { root }
    }

    fn write(&self, relative_path: &str, contents: &str) -> PathBuf {
        let path = self.root.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent directory can be created");
        }
        fs::write(&path, contents).expect("test source can be written");
        path
    }
}

impl Drop for TestWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
