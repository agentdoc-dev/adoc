//! V3.1 review orchestration.
//!
//! Compiles two snapshots of the project (via the
//! [`crate::domain::ports::snapshot_workspace::SnapshotWorkspaceProvider`]
//! port) and projects their graphs into an [`ObjectDiff`]. The application
//! layer never touches `git`; the composition root in `lib.rs` is the only
//! site that constructs the `GitWorktreeProvider` adapter.
//!
//! Wire envelope (`adoc.diff.v0`) is constructed via [`ObjectDiffEnvelope::from_diff`].

use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use serde::Serialize;

use crate::application::compile::{CompileResult, compile_with_provider};
use crate::domain::diagnostic::Diagnostic;
use crate::domain::graph::{GraphArtifactDocument, GraphKnowledgeObjectNode, GraphNode};
use crate::domain::ports::snapshot_workspace::{
    SnapshotError, SnapshotSelector, SnapshotWorkspaceProvider,
};
use crate::domain::review::object_change::ChangedObject;
use crate::domain::review::object_diff::ObjectDiff;
use crate::infrastructure::source::fs::FsSourceProvider;

pub const DIFF_SCHEMA_VERSION: &str = "adoc.diff.v0";

#[derive(Debug, Clone)]
pub struct ReviewInput {
    /// Project root used to construct the git-CLI adapter. The directory must
    /// be inside a git repository when `base` or `head` is a [`SnapshotSelector::GitRef`].
    pub project_root: PathBuf,
    pub base: SnapshotSelector,
    pub head: SnapshotSelector,
}

#[derive(Debug)]
pub struct ReviewLoadResult {
    pub session: ReviewSession,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug)]
pub struct ReviewSession {
    base: CompileResult,
    head: CompileResult,
}

impl ReviewSession {
    pub fn base(&self) -> &CompileResult {
        &self.base
    }

    pub fn head(&self) -> &CompileResult {
        &self.head
    }
}

#[non_exhaustive]
#[derive(Debug)]
pub enum ReviewError {
    BaseSnapshot {
        selector: SnapshotSelector,
        source: SnapshotError,
    },
    HeadSnapshot {
        selector: SnapshotSelector,
        source: SnapshotError,
    },
    BaseCompileBlocked {
        diagnostics: Vec<Diagnostic>,
    },
    HeadCompileBlocked {
        diagnostics: Vec<Diagnostic>,
    },
}

impl fmt::Display for ReviewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BaseSnapshot { selector, .. } => {
                write!(f, "could not materialize base snapshot ({selector})")
            }
            Self::HeadSnapshot { selector, .. } => {
                write!(f, "could not materialize head snapshot ({selector})")
            }
            Self::BaseCompileBlocked { diagnostics } => write!(
                f,
                "base snapshot failed to compile ({} diagnostics)",
                diagnostics.len()
            ),
            Self::HeadCompileBlocked { diagnostics } => write!(
                f,
                "head snapshot failed to compile ({} diagnostics)",
                diagnostics.len()
            ),
        }
    }
}

impl Error for ReviewError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::BaseSnapshot { source, .. } | Self::HeadSnapshot { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Load both base and head snapshots via the provided
/// [`SnapshotWorkspaceProvider`], compile each into a graph, and return the
/// resulting session. Errors short-circuit; partial results are not returned.
///
/// The snapshot handles drop at the end of this function (after compile
/// runs), which triggers the RAII cleanup of any temporary git worktrees.
pub(crate) fn load_review_with_providers<S: SnapshotWorkspaceProvider>(
    input: ReviewInput,
    snapshot_provider: &S,
) -> Result<ReviewLoadResult, ReviewError> {
    let base = compile_snapshot(snapshot_provider, &input.base).map_err(|source| {
        ReviewError::BaseSnapshot {
            selector: input.base.clone(),
            source,
        }
    })?;
    if base.has_errors() {
        return Err(ReviewError::BaseCompileBlocked {
            diagnostics: base.diagnostics,
        });
    }

    let head = compile_snapshot(snapshot_provider, &input.head).map_err(|source| {
        ReviewError::HeadSnapshot {
            selector: input.head.clone(),
            source,
        }
    })?;
    if head.has_errors() {
        return Err(ReviewError::HeadCompileBlocked {
            diagnostics: head.diagnostics,
        });
    }

    let mut diagnostics = Vec::with_capacity(base.diagnostics.len() + head.diagnostics.len());
    diagnostics.extend(base.diagnostics.iter().cloned());
    diagnostics.extend(head.diagnostics.iter().cloned());

    Ok(ReviewLoadResult {
        session: ReviewSession { base, head },
        diagnostics,
    })
}

/// Constant identity prefix used to rebase both base- and head-side source
/// paths onto the same logical root. Without this rebase, `content_hash`
/// values would carry the temporary worktree path on one side and the
/// project workdir path on the other, and every unchanged Knowledge Object
/// would appear in the diff's `changed[]` array.
const REVIEW_IDENTITY_PREFIX: &str = "<review>";

fn compile_snapshot<S: SnapshotWorkspaceProvider>(
    snapshot_provider: &S,
    selector: &SnapshotSelector,
) -> Result<CompileResult, SnapshotError> {
    let workspace = snapshot_provider.checkout(selector)?;
    let source_provider = FsSourceProvider::with_identity_prefix(
        workspace.path().to_path_buf(),
        PathBuf::from(REVIEW_IDENTITY_PREFIX),
    );
    let result = compile_with_provider(&source_provider);
    drop(workspace); // explicit RAII cleanup before returning
    Ok(result)
}

/// Project the two compiled graphs in `session` into an [`ObjectDiff`].
///
/// Pure projection — does not allocate I/O. Knowledge Object scope only;
/// pages, prose blocks, and edges are excluded per V3-DESIGN.md §V3.1.
pub fn diff_objects(session: &ReviewSession) -> ObjectDiff {
    let base = extract_knowledge_objects(&session.base);
    let head = extract_knowledge_objects(&session.head);
    ObjectDiff::compute(&base, &head)
}

fn extract_knowledge_objects(result: &CompileResult) -> Vec<GraphKnowledgeObjectNode> {
    let Some(artifacts) = &result.artifacts else {
        return Vec::new();
    };
    let document: GraphArtifactDocument = serde_json::from_str(&artifacts.graph_json)
        .expect("compile output graph_json is well-formed (compile pipeline invariant)");
    document
        .nodes
        .into_iter()
        .filter_map(|node| match node {
            GraphNode::KnowledgeObject(ko) => Some(ko),
            _ => None,
        })
        .collect()
}

/// Wire envelope for `adoc.diff.v0`. The CLI and (V3.6) MCP layers serialize
/// this struct directly to JSON.
#[derive(Debug, Clone, Serialize)]
pub struct ObjectDiffEnvelope {
    pub schema_version: &'static str,
    pub(crate) created: Vec<GraphKnowledgeObjectNode>,
    pub(crate) deleted: Vec<GraphKnowledgeObjectNode>,
    pub(crate) changed: Vec<ChangedObject>,
    pub diagnostics: Vec<Diagnostic>,
}

impl ObjectDiffEnvelope {
    pub fn from_diff(diff: ObjectDiff, diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            schema_version: DIFF_SCHEMA_VERSION,
            created: diff.created,
            deleted: diff.deleted,
            changed: diff.changed,
            diagnostics,
        }
    }

    /// Number of Knowledge Objects only present on the head side.
    pub fn created_count(&self) -> usize {
        self.created.len()
    }

    /// Number of Knowledge Objects only present on the base side.
    pub fn deleted_count(&self) -> usize {
        self.deleted.len()
    }

    /// Number of Knowledge Objects whose `content_hash` differs between
    /// base and head.
    pub fn changed_count(&self) -> usize {
        self.changed.len()
    }

    /// Object IDs of created entries, in deterministic sort order.
    pub fn created_ids(&self) -> impl Iterator<Item = &str> {
        self.created.iter().map(|node| node.id.as_str())
    }

    /// Object IDs of deleted entries, in deterministic sort order.
    pub fn deleted_ids(&self) -> impl Iterator<Item = &str> {
        self.deleted.iter().map(|node| node.id.as_str())
    }

    /// Object IDs of changed entries, in deterministic sort order.
    pub fn changed_ids(&self) -> impl Iterator<Item = &str> {
        self.changed.iter().map(|entry| entry.id.as_str())
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;

    use crate::domain::ports::snapshot_workspace::{
        GitRef, SnapshotError, SnapshotSelector, SnapshotWorkspace, SnapshotWorkspaceProvider,
    };
    use crate::infrastructure::git::error::GitError;

    use super::*;

    struct InMemorySnapshotWorkspaceProvider {
        workdir: PathBuf,
        refs: HashMap<String, PathBuf>,
        record: RefCell<Vec<SnapshotSelector>>,
    }

    impl InMemorySnapshotWorkspaceProvider {
        fn new(workdir: PathBuf) -> Self {
            Self {
                workdir,
                refs: HashMap::new(),
                record: RefCell::new(Vec::new()),
            }
        }

        fn with_ref(mut self, spec: &str, path: PathBuf) -> Self {
            self.refs.insert(spec.to_string(), path);
            self
        }

        fn recorded_selectors(&self) -> Vec<SnapshotSelector> {
            self.record.borrow().clone()
        }
    }

    impl SnapshotWorkspaceProvider for InMemorySnapshotWorkspaceProvider {
        fn checkout(
            &self,
            selector: &SnapshotSelector,
        ) -> Result<SnapshotWorkspace, SnapshotError> {
            self.record.borrow_mut().push(selector.clone());
            match selector {
                SnapshotSelector::Workdir => Ok(SnapshotWorkspace::workdir(self.workdir.clone())),
                SnapshotSelector::GitRef(spec) => self
                    .refs
                    .get(spec.as_str())
                    .map(|path| SnapshotWorkspace::workdir(path.clone()))
                    .ok_or_else(|| {
                        SnapshotError::Git(GitError::RefNotResolvable {
                            spec: spec.as_str().to_string(),
                            stderr: "ref not seeded in test double".to_string(),
                        })
                    }),
            }
        }
    }

    fn write_billing_source(root: &std::path::Path, body: &str) {
        let docs = root.join("docs");
        fs::create_dir_all(&docs).expect("docs dir");
        let source = format!(
            concat!(
                "# Billing @doc(team.billing)\n",
                "\n",
                "::claim billing.credits\n",
                "status: draft\n",
                "--\n",
                "{body}\n",
                "::\n",
            ),
            body = body,
        );
        fs::write(docs.join("billing.adoc"), source).expect("write billing.adoc");
    }

    fn fresh_workspace(label: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "adoc-review-test-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create workspace");
        path
    }

    #[test]
    fn load_review_with_providers_checks_out_base_and_head_in_order() {
        let base_root = fresh_workspace("base");
        write_billing_source(&base_root, "Credits apply after payment.");
        let head_root = fresh_workspace("head");
        write_billing_source(&head_root, "Credits apply after ledger commit.");

        let provider = InMemorySnapshotWorkspaceProvider::new(head_root.clone())
            .with_ref("main", base_root.clone());

        let result = load_review_with_providers(
            ReviewInput {
                project_root: head_root.clone(),
                base: SnapshotSelector::GitRef(GitRef::new("main")),
                head: SnapshotSelector::Workdir,
            },
            &provider,
        )
        .expect("load review succeeds");

        let recorded = provider.recorded_selectors();
        assert_eq!(recorded.len(), 2);
        assert!(matches!(recorded[0], SnapshotSelector::GitRef(_)));
        assert!(matches!(recorded[1], SnapshotSelector::Workdir));
        assert!(result.session.base().artifacts.is_some());
        assert!(result.session.head().artifacts.is_some());

        let _ = (base_root, head_root); // keep tempdirs alive for compile
    }

    #[test]
    fn load_review_returns_base_compile_blocked_when_base_compile_has_errors() {
        let base_root = fresh_workspace("base-blocked");
        fs::create_dir_all(base_root.join("docs")).expect("docs");
        // Raw HTML inside an .adoc source triggers a compile error.
        fs::write(
            base_root.join("docs/bad.adoc"),
            "# Guide @doc(team.guide)\n\n<div>raw</div>\n",
        )
        .expect("write bad source");
        let head_root = fresh_workspace("head-blocked-fine");
        write_billing_source(&head_root, "Clean head.");

        let provider = InMemorySnapshotWorkspaceProvider::new(head_root.clone())
            .with_ref("main", base_root.clone());

        let error = load_review_with_providers(
            ReviewInput {
                project_root: head_root.clone(),
                base: SnapshotSelector::GitRef(GitRef::new("main")),
                head: SnapshotSelector::Workdir,
            },
            &provider,
        )
        .expect_err("base compile errors must propagate");

        match error {
            ReviewError::BaseCompileBlocked { diagnostics } => {
                assert!(
                    !diagnostics.is_empty(),
                    "diagnostics must explain the error"
                );
            }
            other => panic!("expected BaseCompileBlocked, got: {other:?}"),
        }

        let _ = (base_root, head_root);
    }

    #[test]
    fn load_review_returns_base_snapshot_error_when_provider_fails() {
        let head_root = fresh_workspace("head-snap-error");
        write_billing_source(&head_root, "Clean head.");

        let provider = InMemorySnapshotWorkspaceProvider::new(head_root.clone());

        let error = load_review_with_providers(
            ReviewInput {
                project_root: head_root.clone(),
                base: SnapshotSelector::GitRef(GitRef::new("unseeded")),
                head: SnapshotSelector::Workdir,
            },
            &provider,
        )
        .expect_err("unseeded base ref must error");

        match error {
            ReviewError::BaseSnapshot { selector, .. } => match selector {
                SnapshotSelector::GitRef(spec) => assert_eq!(spec.as_str(), "unseeded"),
                other => panic!("expected GitRef selector, got: {other:?}"),
            },
            other => panic!("expected BaseSnapshot, got: {other:?}"),
        }
        let _ = head_root;
    }

    #[test]
    fn diff_objects_returns_three_arrays_against_a_real_compile() {
        let base_root = fresh_workspace("diff-base");
        let head_root = fresh_workspace("diff-head");

        // Base: one claim that will change body, one that will be deleted.
        fs::create_dir_all(base_root.join("docs")).expect("docs");
        fs::write(
            base_root.join("docs/billing.adoc"),
            concat!(
                "# Billing @doc(team.billing)\n",
                "\n",
                "::claim billing.credits\n",
                "status: draft\n",
                "--\n",
                "Old credits behaviour.\n",
                "::\n",
                "\n",
                "::claim billing.legacy-credits\n",
                "status: draft\n",
                "--\n",
                "Legacy credits, slated for removal.\n",
                "::\n",
            ),
        )
        .expect("write base source");

        // Head: changed credits body, no legacy claim, new holds claim.
        fs::create_dir_all(head_root.join("docs")).expect("docs");
        fs::write(
            head_root.join("docs/billing.adoc"),
            concat!(
                "# Billing @doc(team.billing)\n",
                "\n",
                "::claim billing.credits\n",
                "status: draft\n",
                "--\n",
                "New credits behaviour after ledger refactor.\n",
                "::\n",
                "\n",
                "::claim billing.holds\n",
                "status: draft\n",
                "--\n",
                "Holds delay disbursement.\n",
                "::\n",
            ),
        )
        .expect("write head source");

        let provider = InMemorySnapshotWorkspaceProvider::new(head_root.clone())
            .with_ref("main", base_root.clone());

        let load = load_review_with_providers(
            ReviewInput {
                project_root: head_root.clone(),
                base: SnapshotSelector::GitRef(GitRef::new("main")),
                head: SnapshotSelector::Workdir,
            },
            &provider,
        )
        .expect("load review succeeds");

        let diff = diff_objects(&load.session);

        assert_eq!(diff.created().len(), 1, "created: {:#?}", diff.created());
        assert_eq!(diff.created()[0].id, "billing.holds");
        assert_eq!(diff.deleted().len(), 1, "deleted: {:#?}", diff.deleted());
        assert_eq!(diff.deleted()[0].id, "billing.legacy-credits");
        assert_eq!(diff.changed().len(), 1, "changed: {:#?}", diff.changed());
        assert_eq!(diff.changed()[0].id, "billing.credits");
        assert_ne!(
            diff.changed()[0].base.content_hash,
            diff.changed()[0].head.content_hash
        );

        let _ = (base_root, head_root);
    }

    #[test]
    fn envelope_from_diff_includes_schema_version_constant() {
        let envelope = ObjectDiffEnvelope::from_diff(ObjectDiff::compute(&[], &[]), Vec::new());

        let value = serde_json::to_value(&envelope).expect("envelope serializes");
        assert_eq!(value["schema_version"], "adoc.diff.v0");
        assert!(value["created"].as_array().expect("created").is_empty());
        assert!(value["deleted"].as_array().expect("deleted").is_empty());
        assert!(value["changed"].as_array().expect("changed").is_empty());
        assert!(
            value["diagnostics"]
                .as_array()
                .expect("diagnostics")
                .is_empty()
        );
    }
}
