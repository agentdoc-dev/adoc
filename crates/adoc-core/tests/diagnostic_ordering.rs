use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use adoc_core::{CompileInput, DiagnosticCode, compile_workspace};

#[test]
fn compile_workspace_returns_mixed_validation_diagnostics_in_source_order() {
    let workspace = TestWorkspace::new("diagnostic-source-order");
    let source = workspace.write(
        "guide.adoc",
        "# Guide @doc(team.guide)\n\nsee [bad](javascript:alert) first\n\n<div>raw</div>\n",
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(result.has_errors(), "invalid source should fail compilation");
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
