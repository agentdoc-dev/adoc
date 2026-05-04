use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use adoc_core::{CompileInput, DiagnosticCode, compile_workspace};

#[test]
fn raw_html_with_quoted_greater_than_spans_to_tag_close() {
    let workspace = TestWorkspace::new("raw-html-quoted-greater-than");
    let source = workspace.write(
        "guide.adoc",
        "# Guide @doc(team.guide)\n\n<a href=\"x>y\">link</a>\n",
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(result.has_errors(), "raw HTML should fail compilation");
    assert_eq!(result.diagnostics.len(), 1);
    let diagnostic = &result.diagnostics[0];
    assert_eq!(diagnostic.code, DiagnosticCode::ParseRawHtml);
    assert_eq!(
        diagnostic.span.as_ref().map(|span| span.end.column),
        Some(15),
        "span should end after the closing bracket outside the quoted attribute"
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
