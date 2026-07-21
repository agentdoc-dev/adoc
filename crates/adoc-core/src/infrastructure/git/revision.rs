use std::path::Path;
use std::process::Command;

use crate::domain::ports::snapshot_workspace::{GitRef, SnapshotError, SnapshotSelector};

use super::util::clear_git_env;

#[derive(Debug)]
pub(crate) struct ResolvedReview {
    pub(crate) base: SnapshotSelector,
    pub(crate) head: SnapshotSelector,
    pub(crate) head_sha: String,
}

pub(crate) fn resolve_review(
    repo_root: &Path,
    base: &SnapshotSelector,
    head: &SnapshotSelector,
) -> Result<ResolvedReview, SnapshotError> {
    let SnapshotSelector::GitRef(base) = base else {
        return Err(SnapshotError::ProviderUnavailable {
            reason: "review base must be a git ref".to_string(),
        });
    };
    let base_sha = resolve_commit(repo_root, base.as_str())?;
    let head_sha = match head {
        SnapshotSelector::Workdir => resolve_commit(repo_root, "HEAD")?,
        SnapshotSelector::GitRef(head) => resolve_commit(repo_root, head.as_str())?,
    };
    let merge_bases = git_lines(
        repo_root,
        &["merge-base", "--all", &base_sha, &head_sha],
        true,
    )?;
    let [comparison_base] = merge_bases.as_slice() else {
        return Err(SnapshotError::ProviderUnavailable {
            reason: format!(
                "expected exactly one merge base for {base_sha} and {head_sha}, found {}",
                merge_bases.len()
            ),
        });
    };
    Ok(ResolvedReview {
        base: SnapshotSelector::GitRef(GitRef::new(comparison_base.clone())),
        head: match head {
            SnapshotSelector::Workdir => SnapshotSelector::Workdir,
            SnapshotSelector::GitRef(_) => SnapshotSelector::GitRef(GitRef::new(head_sha.clone())),
        },
        head_sha,
    })
}

fn resolve_commit(repo_root: &Path, spec: &str) -> Result<String, SnapshotError> {
    let lines = git_lines(
        repo_root,
        &["rev-parse", "--verify", &format!("{spec}^{{commit}}")],
        false,
    )
    .map_err(|error| match error {
        SnapshotError::ProviderUnavailable { reason } => SnapshotError::UnresolvableRef {
            spec: spec.to_string(),
            reason,
        },
        other => other,
    })?;
    let [sha] = lines.as_slice() else {
        return Err(SnapshotError::UnresolvableRef {
            spec: spec.to_string(),
            reason: "git did not return exactly one commit".to_string(),
        });
    };
    if sha.len() != 40 || !sha.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(SnapshotError::UnresolvableRef {
            spec: spec.to_string(),
            reason: "git returned an invalid commit id".to_string(),
        });
    }
    Ok(sha.to_ascii_lowercase())
}

fn git_lines(
    repo_root: &Path,
    args: &[&str],
    no_result_is_empty: bool,
) -> Result<Vec<String>, SnapshotError> {
    let mut command = Command::new("git");
    command.arg("-C").arg(repo_root).args(args);
    clear_git_env(&mut command);
    let output = command.output().map_err(SnapshotError::Io)?;
    if !(output.status.success() || no_result_is_empty && output.status.code() == Some(1)) {
        return Err(SnapshotError::ProviderUnavailable {
            reason: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    let stdout =
        std::str::from_utf8(&output.stdout).map_err(|_| SnapshotError::ProviderUnavailable {
            reason: "git returned non-UTF-8 revision output".to_string(),
        })?;
    Ok(stdout.lines().map(str::to_string).collect())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::process::Command;

    use super::*;

    fn git(root: &Path, args: &[&str]) -> String {
        let mut command = Command::new("git");
        command.arg("-C").arg(root).args(args);
        clear_git_env(&mut command);
        let output = command.output().expect("git runs");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .expect("git output is UTF-8")
            .trim()
            .to_string()
    }

    fn repo() -> tempfile::TempDir {
        let repo = tempfile::tempdir().expect("tempdir");
        git(repo.path(), &["init", "--initial-branch=main"]);
        git(repo.path(), &["config", "user.email", "test@adoc.dev"]);
        git(repo.path(), &["config", "user.name", "adoc tests"]);
        git(repo.path(), &["config", "commit.gpgsign", "false"]);
        fs::write(repo.path().join("root"), "root\n").expect("root file");
        git(repo.path(), &["add", "-A"]);
        git(repo.path(), &["commit", "-m", "root"]);
        repo
    }

    #[test]
    fn resolve_review_uses_the_unique_merge_base_and_full_head_sha() {
        let repo = repo();
        git(repo.path(), &["checkout", "-b", "feature"]);
        fs::write(repo.path().join("feature"), "feature\n").expect("feature file");
        git(repo.path(), &["add", "-A"]);
        git(repo.path(), &["commit", "-m", "feature"]);
        let head = git(repo.path(), &["rev-parse", "HEAD"]);
        let base = git(repo.path(), &["merge-base", "main", "HEAD"]);

        let resolved = resolve_review(
            repo.path(),
            &SnapshotSelector::GitRef(GitRef::new("main")),
            &SnapshotSelector::Workdir,
        )
        .expect("unique comparison");

        assert_eq!(resolved.head_sha, head);
        match resolved.base {
            SnapshotSelector::GitRef(actual) => assert_eq!(actual.as_str(), base),
            SnapshotSelector::Workdir => panic!("base must be immutable"),
        }
    }

    #[test]
    fn resolve_review_rejects_unrelated_histories() {
        let repo = repo();
        git(repo.path(), &["checkout", "--orphan", "orphan"]);
        git(repo.path(), &["rm", "-rf", "."]);
        fs::write(repo.path().join("orphan"), "orphan\n").expect("orphan file");
        git(repo.path(), &["add", "-A"]);
        git(repo.path(), &["commit", "-m", "orphan"]);

        let error = resolve_review(
            repo.path(),
            &SnapshotSelector::GitRef(GitRef::new("main")),
            &SnapshotSelector::Workdir,
        )
        .expect_err("zero merge bases must fail");

        assert!(format!("{error}").contains("found 0"));
    }

    #[test]
    fn resolve_review_rejects_multiple_merge_bases() {
        let repo = repo();
        let root = git(repo.path(), &["rev-parse", "HEAD"]);
        git(repo.path(), &["checkout", "-b", "a"]);
        fs::write(repo.path().join("a"), "a\n").expect("a file");
        git(repo.path(), &["add", "-A"]);
        git(repo.path(), &["commit", "-m", "a"]);
        let a = git(repo.path(), &["rev-parse", "HEAD"]);
        git(repo.path(), &["checkout", "-b", "b", &root]);
        fs::write(repo.path().join("b"), "b\n").expect("b file");
        git(repo.path(), &["add", "-A"]);
        git(repo.path(), &["commit", "-m", "b"]);
        let b = git(repo.path(), &["rev-parse", "HEAD"]);
        let tree = git(repo.path(), &["rev-parse", &format!("{a}^{{tree}}")]);
        let left = git(
            repo.path(),
            &["commit-tree", &tree, "-p", &a, "-p", &b, "-m", "left"],
        );
        let right = git(
            repo.path(),
            &["commit-tree", &tree, "-p", &b, "-p", &a, "-m", "right"],
        );
        git(repo.path(), &["update-ref", "refs/heads/left", &left]);
        git(repo.path(), &["update-ref", "refs/heads/right", &right]);

        let error = resolve_review(
            repo.path(),
            &SnapshotSelector::GitRef(GitRef::new("left")),
            &SnapshotSelector::GitRef(GitRef::new("right")),
        )
        .expect_err("multiple merge bases must fail");

        assert!(format!("{error}").contains("found 2"));
    }
}
