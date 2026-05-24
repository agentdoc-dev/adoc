//! `ChangedFilesProvider` adapter backed by the system `git` binary.
//!
//! Resolves the file set between a base ref and HEAD via
//! `git diff --name-only <ref>...`. The three-dot form returns files changed
//! in the current branch's history since the merge base with `<ref>` — the
//! canonical CI shape ("what did this branch touch since main") and the form
//! V3-DESIGN.md §V3.3 names.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::domain::ports::changed_files::{ChangedFilesError, ChangedFilesProvider};
use crate::domain::ports::snapshot_workspace::SnapshotSelector;
use crate::domain::value_objects::rel_path::RelPath;

use super::error::GitError;
use super::util::clear_git_env;

/// V3.3 git-CLI adapter for [`ChangedFilesProvider`].
pub(crate) struct GitChangedFilesProvider {
    repo_root: PathBuf,
}

impl GitChangedFilesProvider {
    pub(crate) fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
        }
    }
}

impl ChangedFilesProvider for GitChangedFilesProvider {
    fn changed_files(&self, base: &SnapshotSelector) -> Result<Vec<RelPath>, ChangedFilesError> {
        match base {
            // No base to diff against — `adoc review` against the workdir
            // alone has no meaningful changed-file set.
            SnapshotSelector::Workdir => Ok(Vec::new()),
            SnapshotSelector::GitRef(spec) => self.diff_against(spec.as_str()),
        }
    }
}

impl GitChangedFilesProvider {
    fn diff_against(&self, spec: &str) -> Result<Vec<RelPath>, ChangedFilesError> {
        let three_dot = format!("{spec}...");
        let output = run_git(&self.repo_root, &["diff", "--name-only", &three_dot])?;
        if !output.status.success() {
            return Err(ChangedFilesError::Git(GitError::CommandFailed {
                command: format!("git diff --name-only {three_dot}"),
                code: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            }));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut paths = Vec::new();
        for line in stdout.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            // `git diff --name-only` always emits repo-relative paths with
            // forward slashes. If a value somehow fails `RelPath::try_new`
            // we drop it — alerting the caller would falsely block diff/review
            // on a malformed path that the user did not author.
            if let Ok(path) = RelPath::try_new(trimmed) {
                paths.push(path);
            }
        }
        Ok(paths)
    }
}

fn run_git(repo_root: &Path, args: &[&str]) -> Result<Output, ChangedFilesError> {
    let mut command = Command::new("git");
    command.arg("-C").arg(repo_root);
    for arg in args {
        command.arg(arg);
    }
    clear_git_env(&mut command);
    command.output().map_err(|source| match source.kind() {
        std::io::ErrorKind::NotFound => ChangedFilesError::Git(GitError::GitNotFound),
        _ => ChangedFilesError::Git(GitError::CommandSpawn {
            program: "git".to_string(),
            source,
        }),
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::domain::ports::snapshot_workspace::GitRef;

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

    struct Repo {
        _tempdir: tempfile::TempDir,
        root: PathBuf,
    }

    impl Repo {
        fn new() -> Self {
            let temp = tempfile::Builder::new()
                .prefix("adoc-git-changed-files-test-")
                .tempdir()
                .expect("tempdir");
            let root = temp.path().to_path_buf();
            run(&root, &["init", "--initial-branch=main"]);
            run(&root, &["config", "user.email", "test@adoc.dev"]);
            run(&root, &["config", "user.name", "adoc tests"]);
            run(&root, &["config", "commit.gpgsign", "false"]);
            Self {
                _tempdir: temp,
                root,
            }
        }
    }

    #[test]
    fn workdir_selector_returns_empty_changed_set() {
        let repo = Repo::new();
        fs::write(repo.root.join("a.txt"), "hi\n").unwrap();
        run(&repo.root, &["add", "-A"]);
        run(&repo.root, &["commit", "-m", "base"]);

        let provider = GitChangedFilesProvider::new(repo.root.clone());

        assert!(
            provider
                .changed_files(&SnapshotSelector::Workdir)
                .expect("workdir is ok")
                .is_empty()
        );
    }

    #[test]
    fn gitref_selector_lists_files_committed_on_a_feature_branch() {
        let repo = Repo::new();
        fs::write(repo.root.join("a.txt"), "hi\n").unwrap();
        run(&repo.root, &["add", "-A"]);
        run(&repo.root, &["commit", "-m", "base"]);

        // Switch to a feature branch and commit two changes there.
        run(&repo.root, &["checkout", "-b", "feature"]);
        fs::write(repo.root.join("a.txt"), "hi edited\n").unwrap();
        fs::write(repo.root.join("c.txt"), "feature\n").unwrap();
        run(&repo.root, &["add", "-A"]);
        run(&repo.root, &["commit", "-m", "feature commit"]);

        let provider = GitChangedFilesProvider::new(repo.root.clone());

        let mut paths: Vec<String> = provider
            .changed_files(&SnapshotSelector::GitRef(GitRef::new("main")))
            .expect("diff names list")
            .into_iter()
            .map(|p| p.as_str().to_string())
            .collect();
        paths.sort();

        assert_eq!(paths, vec!["a.txt".to_string(), "c.txt".to_string()]);
    }

    #[test]
    fn unresolvable_ref_returns_command_failed_error() {
        let repo = Repo::new();
        fs::write(repo.root.join("a.txt"), "hi\n").unwrap();
        run(&repo.root, &["add", "-A"]);
        run(&repo.root, &["commit", "-m", "base"]);

        let provider = GitChangedFilesProvider::new(repo.root.clone());

        let error = provider
            .changed_files(&SnapshotSelector::GitRef(GitRef::new(
                "definitely-not-a-real-ref",
            )))
            .expect_err("bad ref must error");

        match error {
            ChangedFilesError::Git(GitError::CommandFailed { command, .. }) => {
                assert!(command.contains("definitely-not-a-real-ref"));
            }
            other => panic!("expected CommandFailed, got: {other:?}"),
        }
    }
}
