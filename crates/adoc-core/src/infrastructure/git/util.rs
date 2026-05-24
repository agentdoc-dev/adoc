//! Shared helpers across V3 git adapters.

use std::process::Command;

/// Strip inherited `GIT_*` environment variables so the spawned git always
/// operates on the explicit repo we pass via `-C`. Tools like prek run the
/// test suite from inside a pre-commit hook that exports `GIT_DIR` pointing at
/// the outer repository's `.git`; without this scrub, every invocation here
/// would target the outer repo regardless of `repo_root`.
///
/// Lifted out of `worktree.rs` in V3.3 so the new `GitChangedFilesProvider`
/// adapter can reuse exactly the same scrub list.
pub(super) fn clear_git_env(command: &mut Command) {
    for var in [
        "GIT_DIR",
        "GIT_INDEX_FILE",
        "GIT_WORK_TREE",
        "GIT_NAMESPACE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_COMMON_DIR",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_PREFIX",
    ] {
        command.env_remove(var);
    }
}
