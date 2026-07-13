//! V8.1.1 committed-clean probe for `adoc migrate --write` (ADR-0043).
//!
//! `--write` removes the source `.md` after writing `<name>.adoc`; a
//! committed source is what makes that removal reversible. The probe answers
//! one question — is this file tracked, unmodified, and inside a git work
//! tree? — and never panics: any failure (no git binary, outside a
//! repository, unreadable path) is `false`, which the caller surfaces as the
//! `migrate.source_not_committed` refusal.

use std::path::Path;

use crate::domain::ports::committed_source::CommittedSourceProbe;

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct GitCommittedSourceProbe;

impl CommittedSourceProbe for GitCommittedSourceProbe {
    fn is_committed_and_clean(&self, source: &Path) -> bool {
        is_committed_and_clean(source)
    }
}
use std::process::Command;

use super::util::clear_git_env;

/// Returns `true` only when `file` is inside a git work tree, tracked, and
/// has no uncommitted changes (staged or unstaged).
///
/// Every git call runs with `-C <parent>` and the bare file name as the
/// pathspec: `-C` changes git's working directory, so a caller-relative path
/// (`./api/auth.md`) would silently stop matching anything.
pub(crate) fn is_committed_and_clean(file: &Path) -> bool {
    let (Some(parent), Some(name)) = (file.parent(), file.file_name()) else {
        return false;
    };
    let name = Path::new(name);
    is_inside_work_tree(parent) && is_tracked(parent, name) && has_clean_status(parent, name)
}

fn is_inside_work_tree(directory: &Path) -> bool {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(directory)
        .args(["rev-parse", "--is-inside-work-tree"]);
    clear_git_env(&mut command);
    matches!(command.output(), Ok(output) if output.status.success())
}

/// `git status --porcelain` is silent for untracked-and-ignored files, so an
/// explicit tracked check (`ls-files --error-unmatch`) closes that gap.
fn is_tracked(directory: &Path, file: &Path) -> bool {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(directory)
        .args(["ls-files", "--error-unmatch", "--"])
        .arg(file);
    clear_git_env(&mut command);
    matches!(command.output(), Ok(output) if output.status.success())
}

fn has_clean_status(directory: &Path, file: &Path) -> bool {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(directory)
        .args(["status", "--porcelain", "--"])
        .arg(file);
    clear_git_env(&mut command);
    matches!(
        command.output(),
        Ok(output) if output.status.success() && output.stdout.is_empty()
    )
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

    fn committed_repo() -> tempfile::TempDir {
        let temp = tempfile::Builder::new()
            .prefix("adoc-clean-source-")
            .tempdir()
            .expect("tempdir");
        let root = temp.path();
        run(root, &["init", "--initial-branch=main"]);
        run(root, &["config", "user.email", "test@adoc.dev"]);
        run(root, &["config", "user.name", "adoc tests"]);
        run(root, &["config", "commit.gpgsign", "false"]);
        fs::write(root.join("guide.md"), "# Guide\n").expect("write guide");
        run(root, &["add", "-A"]);
        run(root, &["commit", "-m", "initial"]);
        temp
    }

    #[test]
    fn committed_clean_file_passes() {
        let repo = committed_repo();
        assert!(is_committed_and_clean(&repo.path().join("guide.md")));
    }

    #[test]
    fn modified_file_fails() {
        let repo = committed_repo();
        fs::write(repo.path().join("guide.md"), "# Guide\n\nEdited.\n").expect("edit guide");
        assert!(!is_committed_and_clean(&repo.path().join("guide.md")));
    }

    #[test]
    fn untracked_file_fails() {
        let repo = committed_repo();
        fs::write(repo.path().join("new.md"), "# New\n").expect("write new");
        assert!(!is_committed_and_clean(&repo.path().join("new.md")));
    }

    #[test]
    fn file_outside_a_repository_fails() {
        let temp = tempfile::Builder::new()
            .prefix("adoc-clean-source-norepo-")
            .tempdir()
            .expect("tempdir");
        let file = temp.path().join("guide.md");
        fs::write(&file, "# Guide\n").expect("write guide");
        assert!(!is_committed_and_clean(&file));
    }

    #[test]
    fn missing_file_fails() {
        assert!(!is_committed_and_clean(&PathBuf::from(
            "/this/path/should/not/exist/guide.md"
        )));
    }
}
