//! V3.6 readiness probe for the `adoc diff` / `adoc review` surface.
//!
//! Used by the `adoc.project.status.v0` envelope to set `readiness.review`.
//! Returns `true` only when the `git` binary is on `PATH` and the supplied
//! `repo_root` resolves a default `HEAD` ref — the minimum precondition the
//! V3 review pipeline needs to compute an `adoc.diff.v0` envelope. The probe
//! runs two cheap reads (`git --version` and `git -C <root> rev-parse HEAD`)
//! and never spawns a worktree.

use std::path::Path;
use std::process::Command;

use super::util::clear_git_env;

/// Returns `true` if the local environment can run the V3 review pipeline
/// against `repo_root`. False on any failure — this probe must never panic
/// or surface errors, since it feeds a boolean readiness flag.
pub(crate) fn is_review_available(repo_root: &Path) -> bool {
    git_version_succeeds() && repo_has_head(repo_root)
}

fn git_version_succeeds() -> bool {
    let mut command = Command::new("git");
    command.arg("--version");
    clear_git_env(&mut command);
    matches!(command.output(), Ok(output) if output.status.success())
}

fn repo_has_head(repo_root: &Path) -> bool {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(repo_root)
        .args(["rev-parse", "--verify", "HEAD"]);
    clear_git_env(&mut command);
    matches!(command.output(), Ok(output) if output.status.success())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn run(cwd: &Path, args: &[&str]) {
        let mut command = Command::new("git");
        command.arg("-C").arg(cwd).args(args);
        clear_git_env(&mut command);
        let output = command
            .output()
            .unwrap_or_else(|error| panic!("spawn `git {args:?}`: {error}"));
        assert!(
            output.status.success(),
            "git {args:?} failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }

    #[test]
    fn empty_dir_is_not_review_ready() {
        let temp = tempfile::Builder::new()
            .prefix("adoc-readiness-empty-")
            .tempdir()
            .expect("tempdir");
        assert!(!is_review_available(temp.path()));
    }

    #[test]
    fn non_existent_dir_is_not_review_ready() {
        let bogus = PathBuf::from("/this/path/should/not/exist/anywhere/adoc");
        assert!(!is_review_available(&bogus));
    }

    #[test]
    fn fresh_repo_without_commit_is_not_review_ready() {
        let temp = tempfile::Builder::new()
            .prefix("adoc-readiness-no-commit-")
            .tempdir()
            .expect("tempdir");
        run(temp.path(), &["init", "--initial-branch=main"]);
        // No commit yet: rev-parse HEAD must fail.
        assert!(!is_review_available(temp.path()));
    }

    #[test]
    fn repo_with_one_commit_is_review_ready() {
        let temp = tempfile::Builder::new()
            .prefix("adoc-readiness-ok-")
            .tempdir()
            .expect("tempdir");
        let root = temp.path();
        run(root, &["init", "--initial-branch=main"]);
        run(root, &["config", "user.email", "test@adoc.dev"]);
        run(root, &["config", "user.name", "adoc tests"]);
        run(root, &["config", "commit.gpgsign", "false"]);
        fs::write(root.join("hello.txt"), "hello\n").expect("write hello");
        run(root, &["add", "-A"]);
        run(root, &["commit", "-m", "initial"]);
        assert!(is_review_available(root));
    }
}
