use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn check_accepts_minimal_prose_page() {
    let workspace = TestWorkspace::new("check-accepts-minimal-prose-page");
    let source = workspace.write(
        "guide.adoc",
        "# Getting Started @doc(getting-started)\n\nAgentDoc keeps knowledge readable.\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args(["check", source.to_str().expect("source path is utf-8")])
        .output()
        .expect("adoc check runs");

    assert!(
        output.status.success(),
        "expected check to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("0 errors"),
        "stdout should summarize successful diagnostics"
    );
}

#[test]
fn build_creates_missing_output_directory_and_writes_artifacts() {
    let workspace = TestWorkspace::new("build-writes-artifacts");
    let source = workspace.write(
        "guide.adoc",
        "# Getting Started @doc(getting-started)\n\nAgentDoc keeps knowledge readable.\n",
    );
    let output_directory = workspace.root.join("dist");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "build",
            source.to_str().expect("source path is utf-8"),
            "--out",
            output_directory
                .to_str()
                .expect("output directory path is utf-8"),
        ])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let html = fs::read_to_string(output_directory.join("docs.html")).expect("HTML is written");
    assert!(html.contains("<h1>Getting Started</h1>"));
    assert!(html.contains("<p>AgentDoc keeps knowledge readable.</p>"));

    let agent_json = fs::read_to_string(output_directory.join("docs.agent.json"))
        .expect("agent JSON is written");
    assert!(agent_json.contains("\"schema_version\": \"adoc.agent.v0\""));
    assert!(agent_json.contains("\"pages\""));
    assert!(agent_json.contains("\"objects\": []"));
    assert!(agent_json.contains("\"diagnostics\": []"));
}

struct TestWorkspace {
    pub root: PathBuf,
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
