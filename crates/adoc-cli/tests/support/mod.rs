pub mod v1_4;

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static WORKSPACE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[allow(dead_code)]
pub(crate) fn fixture_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(relative)
}

#[allow(dead_code)]
pub(crate) fn workspace_fixture_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
        .join(relative)
}

/// Recursively copy a fixture tree into a test workspace. V6.4 TB5: apply
/// tests mutate source files, so they always run against a tempdir copy —
/// the in-repo fixture stays pristine.
#[allow(dead_code)]
pub(crate) fn copy_tree(source: &std::path::Path, destination: &std::path::Path) {
    fs::create_dir_all(destination).expect("destination directory can be created");
    for entry in fs::read_dir(source).expect("source directory is readable") {
        let entry = entry.expect("directory entry is readable");
        let target = destination.join(entry.file_name());
        if entry.file_type().expect("file type is readable").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("fixture file copies");
        }
    }
}

#[allow(dead_code)]
pub(crate) fn adoc_command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_adoc"));
    command.env("ADOC_TEST_EMBEDDING_PROVIDER", "deterministic");
    command
}

#[allow(dead_code)]
pub(crate) fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[allow(dead_code)]
pub(crate) fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

pub(crate) struct TestWorkspace {
    pub(crate) root: PathBuf,
}

impl TestWorkspace {
    pub(crate) fn new(name: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock is after epoch")
            .as_nanos();
        let counter = WORKSPACE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "adoc-{name}-{}-{counter}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("test workspace can be created");
        Self { root }
    }

    #[allow(dead_code)]
    pub(crate) fn write(&self, relative_path: &str, contents: &str) -> PathBuf {
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
