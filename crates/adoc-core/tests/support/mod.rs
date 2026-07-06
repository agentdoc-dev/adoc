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
}

impl Drop for TestWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
