use std::fs;
use std::path::Path;

use adoc_local::{PathPolicy, ProjectRootPathPolicy, UnrestrictedPathPolicy};

fn write(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent directory can be created");
    }
    fs::write(path, contents).expect("file can be written");
}

#[test]
fn unrestricted_policy_returns_paths_unchanged() {
    let policy = UnrestrictedPathPolicy;
    let path = Path::new("../outside/docs.adoc");

    assert_eq!(policy.resolve_read_path(path).unwrap(), path);
    assert_eq!(policy.resolve_write_path(path).unwrap(), path);
}

#[test]
fn project_root_policy_rejects_parent_escape() {
    let workspace = tempfile::tempdir().expect("workspace");
    let policy = ProjectRootPathPolicy::new(workspace.path()).expect("policy");

    let error = policy
        .resolve_read_path(Path::new("../outside.adoc"))
        .expect_err("parent escape rejected");

    assert!(error.to_string().contains("path_outside_project"));
}

#[test]
fn project_root_policy_rejects_absolute_path_outside_root() {
    let workspace = tempfile::tempdir().expect("workspace");
    let outside = tempfile::tempdir().expect("outside");
    let policy = ProjectRootPathPolicy::new(workspace.path()).expect("policy");

    let error = policy
        .resolve_write_path(&outside.path().join("docs.html"))
        .expect_err("outside absolute output rejected");

    assert!(error.to_string().contains("path_outside_project"));
}

#[test]
fn project_root_policy_accepts_missing_write_path_inside_root() {
    let workspace = tempfile::tempdir().expect("workspace");
    let policy = ProjectRootPathPolicy::new(workspace.path()).expect("policy");

    let path = policy
        .resolve_write_path(Path::new("dist/nested/docs.html"))
        .expect("missing inside-root output allowed");

    assert_eq!(
        path,
        fs::canonicalize(workspace.path())
            .expect("workspace canonicalizes")
            .join("dist/nested/docs.html")
    );
}

#[cfg(unix)]
#[test]
fn project_root_policy_rejects_symlink_escape_for_reads() {
    let workspace = tempfile::tempdir().expect("workspace");
    let outside = tempfile::tempdir().expect("outside");
    write(&outside.path().join("artifact.json"), "{}");
    std::os::unix::fs::symlink(outside.path(), workspace.path().join("linked"))
        .expect("symlink can be created");
    let policy = ProjectRootPathPolicy::new(workspace.path()).expect("policy");

    let error = policy
        .resolve_read_path(Path::new("linked/artifact.json"))
        .expect_err("symlink escape rejected");

    assert!(error.to_string().contains("path_outside_project"));
}

#[cfg(unix)]
#[test]
fn project_root_policy_rejects_symlink_escape_for_writes() {
    let workspace = tempfile::tempdir().expect("workspace");
    let outside = tempfile::tempdir().expect("outside");
    std::os::unix::fs::symlink(outside.path(), workspace.path().join("linked"))
        .expect("symlink can be created");
    let policy = ProjectRootPathPolicy::new(workspace.path()).expect("policy");

    let error = policy
        .resolve_write_path(Path::new("linked/output.json"))
        .expect_err("symlink output escape rejected");

    assert!(error.to_string().contains("path_outside_project"));
    assert!(!outside.path().join("output.json").exists());
}
