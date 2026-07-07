use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) struct TestWorkspace {
    root: PathBuf,
}

impl TestWorkspace {
    pub(crate) fn new(name: &str) -> Self {
        // The timestamp alone is not unique: parallel tests sharing a `name`
        // can observe the same clock reading (coarse resolution), collide on
        // the root, and delete each other's files on Drop. Match the
        // adoc-cli/adoc-mcp support pattern: pid + per-process counter.
        static WORKSPACE_COUNTER: AtomicU64 = AtomicU64::new(0);
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

    pub(crate) fn write(&self, relative_path: &str, contents: &str) -> PathBuf {
        let path = self.root.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent directory can be created");
        }
        fs::write(&path, contents).expect("test source can be written");
        path
    }

    /// Used by `tests/migrate.rs`; other test binaries including this shared
    /// module drive the workspace through `write` alone.
    #[allow(dead_code)]
    pub(crate) fn root(&self) -> &std::path::Path {
        &self.root
    }
}

/// Recursively copy a fixture tree into a workspace, so mutating tests never
/// touch the checked-in fixture (the adoc-cli support pattern). Used by
/// `tests/migrate.rs`; dead code from the other test binaries' view.
#[allow(dead_code)]
pub(crate) fn copy_tree(source: &std::path::Path, destination: &std::path::Path) {
    fs::create_dir_all(destination).expect("destination directory can be created");
    for entry in fs::read_dir(source).expect("fixture directory is readable") {
        let entry = entry.expect("fixture entry is readable");
        let target = destination.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("fixture file can be copied");
        }
    }
}

impl Drop for TestWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
