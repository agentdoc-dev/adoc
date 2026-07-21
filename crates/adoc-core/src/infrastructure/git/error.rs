//! Errors emitted by the git-CLI adapter.
//!
//! Hand-rolled per V3-DESIGN.md §"Enterprise rules" — no `thiserror`,
//! `#[non_exhaustive]` so adding a variant is not a breaking change, and
//! structured fields so consumers can inspect the cause without parsing
//! strings.

use std::error::Error;
use std::fmt;
use std::io;
use std::path::PathBuf;

#[non_exhaustive]
#[derive(Debug)]
pub enum GitError {
    GitNotFound,
    NotARepository {
        path: PathBuf,
    },
    RefNotResolvable {
        spec: String,
        stderr: String,
    },
    WorktreeCreate {
        tmp: PathBuf,
        stderr: String,
    },
    WorktreeRemove {
        tmp: PathBuf,
        stderr: String,
        source: io::Error,
    },
    CommandSpawn {
        program: String,
        source: io::Error,
    },
    CommandFailed {
        command: String,
        code: Option<i32>,
        stderr: String,
    },
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GitNotFound => f.write_str(
                "could not find a `git` binary on PATH; install git or check the environment",
            ),
            Self::NotARepository { path } => {
                write!(f, "{} is not a git repository", path.display())
            }
            Self::RefNotResolvable { spec, stderr } => {
                write!(f, "git could not resolve ref `{spec}`: {}", stderr.trim())
            }
            Self::WorktreeCreate { tmp, stderr } => write!(
                f,
                "git worktree add failed at {}: {}",
                tmp.display(),
                stderr.trim()
            ),
            Self::WorktreeRemove { tmp, stderr, .. } => write!(
                f,
                "git worktree remove failed at {}: {}",
                tmp.display(),
                stderr.trim()
            ),
            Self::CommandSpawn { program, source } => {
                write!(f, "could not spawn `{program}`: {source}")
            }
            Self::CommandFailed {
                command,
                code,
                stderr,
            } => {
                let code = code
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "<signal>".to_string());
                write!(
                    f,
                    "command failed (`{command}` exited with {code}): {}",
                    stderr.trim()
                )
            }
        }
    }
}

impl Error for GitError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::WorktreeRemove { source, .. } | Self::CommandSpawn { source, .. } => Some(source),
            _ => None,
        }
    }
}

// --- Adapter → port error mapping ---------------------------------------
//
// V3-DESIGN §Error Model rule 7 ("Map at layer boundary") requires the
// adapter to translate its own error vocabulary into the port's domain
// vocabulary. We hold these conversions next to `GitError` so the variant
// list stays exhaustive in one place — every new `GitError` variant has to
// add an arm here or fail to compile.

use crate::domain::ports::changed_files::ChangedFilesError;
use crate::domain::ports::snapshot_workspace::SnapshotError;

impl From<GitError> for SnapshotError {
    fn from(value: GitError) -> Self {
        match value {
            GitError::GitNotFound => SnapshotError::ProviderUnavailable {
                reason: "git binary not found on PATH; install git or check the environment"
                    .to_string(),
            },
            GitError::NotARepository { path } => SnapshotError::ProviderUnavailable {
                reason: format!("{} is not a git repository", path.display()),
            },
            GitError::RefNotResolvable { spec, stderr } => SnapshotError::UnresolvableRef {
                spec,
                reason: stderr.trim().to_string(),
            },
            GitError::WorktreeCreate { tmp, stderr } => SnapshotError::WorktreeUnavailable {
                tmp,
                reason: format!("git worktree add failed: {}", stderr.trim()),
                source: None,
            },
            GitError::WorktreeRemove {
                tmp,
                stderr,
                source,
            } => SnapshotError::WorktreeUnavailable {
                tmp,
                reason: format!("git worktree remove failed: {}", stderr.trim()),
                source: Some(source),
            },
            GitError::CommandSpawn { source, .. } => SnapshotError::Io(source),
            GitError::CommandFailed {
                command,
                code,
                stderr,
            } => SnapshotError::ProviderUnavailable {
                reason: format!(
                    "command failed (`{command}` exited with {}): {}",
                    code.map(|c| c.to_string())
                        .unwrap_or_else(|| "<signal>".to_string()),
                    stderr.trim()
                ),
            },
        }
    }
}

impl From<GitError> for ChangedFilesError {
    fn from(value: GitError) -> Self {
        match value {
            GitError::GitNotFound => ChangedFilesError::ProviderUnavailable {
                reason: "git binary not found on PATH; install git or check the environment"
                    .to_string(),
            },
            GitError::NotARepository { path } => ChangedFilesError::ProviderUnavailable {
                reason: format!("{} is not a git repository", path.display()),
            },
            GitError::RefNotResolvable { spec, stderr } => ChangedFilesError::UnresolvableBase {
                spec,
                reason: stderr.trim().to_string(),
            },
            GitError::CommandFailed {
                command,
                code,
                stderr,
            } => {
                // V3.3 only invokes one git command for changed-files —
                // `git diff --name-only <ref>...` — so a `CommandFailed`
                // here always reflects an unresolvable base ref. This is
                // the single classification site for git-diff failures
                // after `diff_against` routes exit-code-nonzero through
                // `GitError::CommandFailed`.
                let _ = command;
                let _ = code;
                ChangedFilesError::UnresolvableBase {
                    spec: extract_base_spec(&stderr).unwrap_or_default(),
                    reason: stderr.trim().to_string(),
                }
            }
            GitError::CommandSpawn { source, .. } => ChangedFilesError::Io(source),
            // Structurally exhaustive, adapter-unreachable from
            // `ChangedFiles`: the changed-files adapter never creates
            // worktrees. Fall through to provider-level failure so the
            // caller sees a coherent message if a future code path inside
            // this adapter ever starts producing one of these variants.
            GitError::WorktreeCreate { tmp, stderr }
            | GitError::WorktreeRemove { tmp, stderr, .. } => {
                ChangedFilesError::ProviderUnavailable {
                    reason: format!("unexpected worktree error at {}: {}", tmp.display(), stderr),
                }
            }
        }
    }
}

/// Best-effort extraction of the ref spec from a `git diff` stderr line.
/// Used by the `From<GitError> for ChangedFilesError` mapping for
/// `CommandFailed`, where the structured fields no longer carry the spec
/// but stderr typically embeds it (e.g. "fatal: bad revision 'foo'" or the
/// three-dot form "fatal: ambiguous argument 'foo...'"). Strips trailing
/// range dots so the recovered spec matches what the caller originally
/// passed in.
fn extract_base_spec(stderr: &str) -> Option<String> {
    let trimmed = stderr.trim();
    let after_quote = trimmed.split_once('\'')?.1;
    let inner = after_quote.split_once('\'')?.0;
    Some(
        inner
            .split_once("..")
            .map_or(inner, |(base, _)| base)
            .trim_end_matches('.')
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_not_found_renders_actionable_message() {
        let error = GitError::GitNotFound;
        let message = format!("{error}");
        assert!(message.contains("git"));
        assert!(message.contains("PATH"));
        assert!(error.source().is_none());
    }

    #[test]
    fn not_a_repository_renders_path() {
        let error = GitError::NotARepository {
            path: PathBuf::from("/tmp/not-a-repo"),
        };
        assert!(format!("{error}").contains("/tmp/not-a-repo"));
        assert!(error.source().is_none());
    }

    #[test]
    fn ref_not_resolvable_renders_spec_and_stderr() {
        let error = GitError::RefNotResolvable {
            spec: "nonexistent".to_string(),
            stderr: "fatal: bad revision\n".to_string(),
        };
        let message = format!("{error}");
        assert!(message.contains("nonexistent"));
        assert!(message.contains("bad revision"));
        assert!(error.source().is_none());
    }

    #[test]
    fn worktree_create_renders_tmp_and_stderr() {
        let error = GitError::WorktreeCreate {
            tmp: PathBuf::from("/tmp/wt"),
            stderr: "fatal: already locked\n".to_string(),
        };
        let message = format!("{error}");
        assert!(message.contains("/tmp/wt"));
        assert!(message.contains("already locked"));
        assert!(error.source().is_none());
    }

    #[test]
    fn worktree_remove_exposes_io_source() {
        let io_error = io::Error::other("file in use");
        let error = GitError::WorktreeRemove {
            tmp: PathBuf::from("/tmp/wt"),
            stderr: "<stderr>".to_string(),
            source: io_error,
        };
        assert!(format!("{error}").contains("/tmp/wt"));
        assert!(error.source().is_some());
    }

    #[test]
    fn command_spawn_exposes_io_source() {
        let io_error = io::Error::from(io::ErrorKind::PermissionDenied);
        let error = GitError::CommandSpawn {
            program: "git".to_string(),
            source: io_error,
        };
        assert!(format!("{error}").contains("git"));
        assert!(error.source().is_some());
    }

    #[test]
    fn command_failed_renders_code_and_stderr() {
        let error = GitError::CommandFailed {
            command: "git rev-parse HEAD".to_string(),
            code: Some(128),
            stderr: "fatal: not a git repository\n".to_string(),
        };
        let message = format!("{error}");
        assert!(message.contains("git rev-parse HEAD"));
        assert!(message.contains("128"));
        assert!(message.contains("not a git repository"));
    }

    #[test]
    fn command_failed_with_no_exit_code_renders_signal_placeholder() {
        let error = GitError::CommandFailed {
            command: "git fetch".to_string(),
            code: None,
            stderr: "killed".to_string(),
        };
        let message = format!("{error}");
        assert!(message.contains("<signal>"));
    }
}
