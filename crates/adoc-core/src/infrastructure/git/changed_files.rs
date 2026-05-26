//! `ChangedFilesProvider` adapter backed by the system `git` binary.
//!
//! Resolves the file set between a base ref and head via
//! `git diff --name-only <base>...[<head>]`. The three-dot form returns files
//! changed in the head side's history since the merge base with `<base>` — the
//! canonical CI shape ("what did this branch touch since main") and the form
//! V3-DESIGN.md §V3.3 names. When `head` is `Workdir` the implicit form
//! `<base>...` (resolves to `<base>...HEAD`) preserves the historical
//! workdir-comparison behaviour; when `head` is an explicit `GitRef` the
//! adapter materialises `<base>...<head>` so impact analysis honours the
//! caller-supplied ref rather than silently reporting against `HEAD`.

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
    fn changed_files(
        &self,
        base: &SnapshotSelector,
        head: &SnapshotSelector,
    ) -> Result<Vec<RelPath>, ChangedFilesError> {
        match (base, head) {
            // No base to diff against — `adoc review` against the workdir
            // alone has no meaningful changed-file set, regardless of head.
            (SnapshotSelector::Workdir, _) => Ok(Vec::new()),
            (SnapshotSelector::GitRef(b), SnapshotSelector::Workdir) => {
                self.diff_against(b.as_str(), None)
            }
            (SnapshotSelector::GitRef(b), SnapshotSelector::GitRef(h)) => {
                self.diff_against(b.as_str(), Some(h.as_str()))
            }
        }
    }
}

impl GitChangedFilesProvider {
    fn diff_against(
        &self,
        base: &str,
        head: Option<&str>,
    ) -> Result<Vec<RelPath>, ChangedFilesError> {
        let three_dot = match head {
            // `<base>...<head>` — symmetric difference resolving to the
            // changes on `<head>`'s side since the merge base with `<base>`.
            Some(head_spec) => format!("{base}...{head_spec}"),
            // `<base>...` — same shape with `HEAD` implicit, preserving the
            // workdir-comparison behaviour the V3.3 acceptance tests pin.
            None => format!("{base}..."),
        };
        let output = run_git(&self.repo_root, &["diff", "--name-only", &three_dot])?;
        if !output.status.success() {
            // Route exit-code failures through `GitError::CommandFailed` so
            // the `From<GitError> for ChangedFilesError` impl is the single
            // classification site. The arm extracts the failing spec from
            // stderr (git quotes it as `'<spec>...'`).
            return Err(GitError::CommandFailed {
                command: format!("git diff --name-only {three_dot}"),
                code: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            }
            .into());
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
    // Disable git's default C-style quoting of non-ASCII paths so we receive
    // UTF-8 directly. Without this, a file like `café.txt` would appear as
    // `"caf\303\251.txt"` in `git diff --name-only` output and silently fail
    // `RelPath::try_new`, dropping the path from the impact set entirely.
    command.arg("-c").arg("core.quotePath=false");
    for arg in args {
        command.arg(arg);
    }
    clear_git_env(&mut command);
    command
        .output()
        .map_err(|source| match source.kind() {
            std::io::ErrorKind::NotFound => GitError::GitNotFound,
            _ => GitError::CommandSpawn {
                program: "git".to_string(),
                source,
            },
        })
        .map_err(ChangedFilesError::from)
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
                .changed_files(&SnapshotSelector::Workdir, &SnapshotSelector::Workdir)
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
            .changed_files(
                &SnapshotSelector::GitRef(GitRef::new("main")),
                &SnapshotSelector::Workdir,
            )
            .expect("diff names list")
            .into_iter()
            .map(|p| p.as_str().to_string())
            .collect();
        paths.sort();

        assert_eq!(paths, vec!["a.txt".to_string(), "c.txt".to_string()]);
    }

    #[test]
    fn gitref_selector_lists_non_ascii_filename_unquoted() {
        // Regression: git's default `core.quotePath=true` would emit
        // `"caf\303\251.txt"` for café.txt, which fails `RelPath::try_new`
        // and silently drops the path. The adapter forces
        // `-c core.quotePath=false` so non-ASCII paths come through as UTF-8.
        let repo = Repo::new();
        fs::write(repo.root.join("a.txt"), "hi\n").unwrap();
        run(&repo.root, &["add", "-A"]);
        run(&repo.root, &["commit", "-m", "base"]);

        run(&repo.root, &["checkout", "-b", "feature"]);
        fs::write(repo.root.join("café.txt"), "non-ascii\n").unwrap();
        run(&repo.root, &["add", "-A"]);
        run(&repo.root, &["commit", "-m", "add cafe"]);

        let provider = GitChangedFilesProvider::new(repo.root.clone());

        let paths: Vec<String> = provider
            .changed_files(
                &SnapshotSelector::GitRef(GitRef::new("main")),
                &SnapshotSelector::Workdir,
            )
            .expect("diff names list")
            .into_iter()
            .map(|p| p.as_str().to_string())
            .collect();

        assert_eq!(paths, vec!["café.txt".to_string()]);
    }

    #[test]
    fn gitref_head_lists_changes_against_that_head_not_workdir_head() {
        // Codex P1 regression: with an explicit `head_ref`, the changed-file
        // set must reflect `base...head_ref`, NOT `base...HEAD`. Otherwise
        // `adoc review --base main --head HEAD~1` reports impact for the
        // workdir checkout, which is wrong.
        let repo = Repo::new();
        fs::write(repo.root.join("a.txt"), "hi\n").unwrap();
        run(&repo.root, &["add", "-A"]);
        run(&repo.root, &["commit", "-m", "base"]);

        // feature branch with a single committed change against main.
        run(&repo.root, &["checkout", "-b", "feature"]);
        fs::write(repo.root.join("feature-only.txt"), "feature\n").unwrap();
        run(&repo.root, &["add", "-A"]);
        run(&repo.root, &["commit", "-m", "feature commit"]);

        // Then a second commit on feature that should NOT appear when
        // diffing against HEAD~1.
        fs::write(repo.root.join("workdir-only.txt"), "workdir\n").unwrap();
        run(&repo.root, &["add", "-A"]);
        run(&repo.root, &["commit", "-m", "workdir-only commit"]);

        let provider = GitChangedFilesProvider::new(repo.root.clone());

        let mut paths: Vec<String> = provider
            .changed_files(
                &SnapshotSelector::GitRef(GitRef::new("main")),
                &SnapshotSelector::GitRef(GitRef::new("HEAD~1")),
            )
            .expect("diff names list")
            .into_iter()
            .map(|p| p.as_str().to_string())
            .collect();
        paths.sort();

        assert_eq!(
            paths,
            vec!["feature-only.txt".to_string()],
            "diff must reflect base...HEAD~1, excluding the workdir-only commit"
        );
    }

    #[test]
    fn unresolvable_ref_returns_command_failed_error() {
        let repo = Repo::new();
        fs::write(repo.root.join("a.txt"), "hi\n").unwrap();
        run(&repo.root, &["add", "-A"]);
        run(&repo.root, &["commit", "-m", "base"]);

        let provider = GitChangedFilesProvider::new(repo.root.clone());

        let error = provider
            .changed_files(
                &SnapshotSelector::GitRef(GitRef::new("definitely-not-a-real-ref")),
                &SnapshotSelector::Workdir,
            )
            .expect_err("bad ref must error");

        match error {
            ChangedFilesError::UnresolvableBase { spec, .. } => {
                assert_eq!(spec, "definitely-not-a-real-ref");
            }
            other => panic!("expected UnresolvableBase, got: {other:?}"),
        }
    }
}
