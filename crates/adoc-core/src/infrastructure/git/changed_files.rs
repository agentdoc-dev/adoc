//! `ChangedFilesProvider` adapter backed by the system `git` binary.
//!
//! Resolves committed changes between the comparison base and exact head.
//! Workdir review unions that pinned range with staged, unstaged, and
//! untracked non-ignored paths; explicit-head review reads only the immutable
//! committed range.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::domain::ports::changed_files::{ChangedFilesError, ChangedFilesProvider};
use crate::domain::ports::snapshot_workspace::SnapshotSelector;
use crate::domain::source::LogicalPath;
use crate::domain::value_objects::rel_path::RelPath;

use super::error::GitError;
use super::util::clear_git_env;

/// V3.3 git-CLI adapter for [`ChangedFilesProvider`].
pub(crate) struct GitChangedFilesProvider {
    repo_root: PathBuf,
    expected_workdir_head: Option<String>,
}

impl GitChangedFilesProvider {
    pub(crate) fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
            expected_workdir_head: None,
        }
    }

    pub(crate) fn with_expected_workdir_head(mut self, sha: String) -> Self {
        self.expected_workdir_head = Some(sha);
        self
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
                self.workdir_changes(b.as_str())
            }
            (SnapshotSelector::GitRef(b), SnapshotSelector::GitRef(h)) => {
                self.committed_changes(b.as_str(), h.as_str())
            }
        }
    }
}

impl GitChangedFilesProvider {
    fn committed_changes(&self, base: &str, head: &str) -> Result<Vec<RelPath>, ChangedFilesError> {
        let range = format!("{base}..{head}");
        self.paths_from(&[
            "diff",
            "--relative",
            "--name-only",
            "-z",
            "--no-renames",
            &range,
        ])
    }

    fn workdir_changes(&self, base: &str) -> Result<Vec<RelPath>, ChangedFilesError> {
        let head = self.expected_workdir_head.as_deref().unwrap_or("HEAD");
        let range = format!("{base}..{head}");
        let mut paths = BTreeSet::new();
        for args in [
            vec![
                "diff",
                "--relative",
                "--name-only",
                "-z",
                "--no-renames",
                &range,
            ],
            vec![
                "diff",
                "--relative",
                "--cached",
                "--name-only",
                "-z",
                "--no-renames",
                "HEAD",
            ],
            vec!["diff", "--relative", "--name-only", "-z", "--no-renames"],
            vec!["ls-files", "-z", "--others", "--exclude-standard"],
        ] {
            paths.extend(self.paths_from(&args)?);
        }
        Ok(paths.into_iter().collect())
    }

    fn paths_from(&self, args: &[&str]) -> Result<Vec<RelPath>, ChangedFilesError> {
        let output = run_git(&self.repo_root, args)?;
        if !output.status.success() {
            // Route exit-code failures through `GitError::CommandFailed` so
            // the `From<GitError> for ChangedFilesError` impl is the single
            // classification site. The arm extracts the failing spec from
            // stderr (git quotes it as `'<spec>...'`).
            return Err(GitError::CommandFailed {
                command: format!("git {}", args.join(" ")),
                code: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            }
            .into());
        }
        parse_paths(&output.stdout)
    }
}

fn parse_paths(stdout: &[u8]) -> Result<Vec<RelPath>, ChangedFilesError> {
    if stdout.is_empty() {
        return Ok(Vec::new());
    }
    if !stdout.ends_with(&[0]) {
        return Err(ChangedFilesError::InvalidPath {
            reason: "NUL-delimited git output was not terminated".to_string(),
        });
    }
    stdout[..stdout.len() - 1]
        .split(|byte| *byte == 0)
        .map(|raw| {
            let value = std::str::from_utf8(raw).map_err(|_| ChangedFilesError::InvalidPath {
                reason: "path is not valid UTF-8".to_string(),
            })?;
            LogicalPath::parse(value).map_err(|_| ChangedFilesError::InvalidPath {
                reason: format!("{value:?} is not a portable repository-relative path"),
            })?;
            RelPath::try_new(value).map_err(|error| ChangedFilesError::InvalidPath {
                reason: error.to_string(),
            })
        })
        .collect()
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

    fn run(cwd: &Path, args: &[&str]) -> String {
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
        String::from_utf8(output.stdout)
            .expect("git stdout is UTF-8")
            .trim()
            .to_string()
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
    fn workdir_committed_changes_use_the_expected_head_revision() {
        let repo = Repo::new();
        fs::write(repo.root.join("base.txt"), "base\n").unwrap();
        run(&repo.root, &["add", "-A"]);
        run(&repo.root, &["commit", "-m", "base"]);

        run(&repo.root, &["checkout", "-b", "feature"]);
        fs::write(repo.root.join("first.txt"), "first\n").unwrap();
        run(&repo.root, &["add", "-A"]);
        run(&repo.root, &["commit", "-m", "first"]);
        let expected_head = run(&repo.root, &["rev-parse", "HEAD"]);

        fs::write(repo.root.join("later.txt"), "later\n").unwrap();
        run(&repo.root, &["add", "-A"]);
        run(&repo.root, &["commit", "-m", "later"]);

        let provider = GitChangedFilesProvider::new(repo.root.clone())
            .with_expected_workdir_head(expected_head);
        let paths = provider
            .changed_files(
                &SnapshotSelector::GitRef(GitRef::new("main")),
                &SnapshotSelector::Workdir,
            )
            .expect("pinned workdir comparison succeeds")
            .into_iter()
            .map(|path| path.as_str().to_string())
            .collect::<Vec<_>>();

        assert_eq!(paths, vec!["first.txt"]);
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

    #[test]
    fn nul_path_parser_accepts_non_ascii_and_embedded_newline_is_rejected() {
        assert_eq!(
            parse_paths("café.rs\0".as_bytes())
                .expect("valid UTF-8 path")
                .into_iter()
                .map(|path| path.as_str().to_string())
                .collect::<Vec<_>>(),
            vec!["café.rs"]
        );
        assert!(matches!(
            parse_paths(b"line\nbreak.rs\0"),
            Err(ChangedFilesError::InvalidPath { .. })
        ));
    }

    #[test]
    fn nul_path_parser_rejects_malformed_or_unsafe_records() {
        for output in [
            b"unterminated".as_slice(),
            b"\xff\0".as_slice(),
            b"/absolute.rs\0".as_slice(),
            b"../escape.rs\0".as_slice(),
            b"back\\slash.rs\0".as_slice(),
            b" edge.rs\0".as_slice(),
            b"a.rs\0\0".as_slice(),
        ] {
            assert!(
                matches!(
                    parse_paths(output),
                    Err(ChangedFilesError::InvalidPath { .. })
                ),
                "unsafe output should fail: {output:?}"
            );
        }
    }
}
