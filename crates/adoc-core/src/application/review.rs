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
use crate::domain::knowledge_object::claim::{
    OWNER_FIELD, REVIEWED_BY_FIELD, SOURCE_FIELD, TEST_FIELD, VERIFIED_AT_FIELD,
};
use crate::domain::ports::changed_files::{ChangedFilesError, ChangedFilesProvider};
use crate::domain::ports::snapshot_workspace::{
    SnapshotError, SnapshotSelector, SnapshotWorkspaceProvider,
};
use crate::domain::review::field_change::{FieldChange, RelationKind};
use crate::domain::review::impact::{ImpactedObject, compute_impact};
use crate::domain::review::object_change::{ChangedObject, ObjectChange};
use crate::domain::review::object_diff::ObjectDiff;
use crate::domain::review::reviewer::{RequiredReviewer, required_reviewers};
use crate::infrastructure::source::fs::FsSourceProvider;

pub const DIFF_SCHEMA_VERSION: &str = "adoc.diff.v0";
pub const REVIEW_SCHEMA_VERSION: &str = "adoc.review.v0";

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
    /// V3.3 impact projection. Empty for sessions loaded via
    /// [`load_review_with_providers`] (the V3.1 path that does not run
    /// changed-files analysis).
    impact: Vec<ImpactedObject>,
    /// V3.3 required reviewers aggregated from the diff and impact list.
    required_reviewers: Vec<RequiredReviewer>,
}

impl ReviewSession {
    pub fn base(&self) -> &CompileResult {
        &self.base
    }

    pub fn head(&self) -> &CompileResult {
        &self.head
    }

    pub fn impact_analysis(&self) -> &[ImpactedObject] {
        &self.impact
    }

    pub fn required_reviewers(&self) -> &[RequiredReviewer] {
        &self.required_reviewers
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
    /// V3.3 — the `ChangedFilesProvider` adapter failed to resolve the
    /// changed-file set for `selector`.
    ChangedFiles {
        selector: SnapshotSelector,
        source: ChangedFilesError,
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
            Self::ChangedFiles { selector, .. } => {
                write!(f, "could not resolve changed-file set against {selector}")
            }
        }
    }
}

impl Error for ReviewError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::BaseSnapshot { source, .. } | Self::HeadSnapshot { source, .. } => Some(source),
            Self::ChangedFiles { source, .. } => Some(source),
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
        session: ReviewSession {
            base,
            head,
            impact: Vec::new(),
            required_reviewers: Vec::new(),
        },
        diagnostics,
    })
}

/// V3.3 loader. Layers on top of [`load_review_with_providers`] by resolving
/// the changed-file set through the supplied [`ChangedFilesProvider`] and
/// populating the session's `impact` and `required_reviewers` projections.
pub(crate) fn load_review_with_changed_files<
    S: SnapshotWorkspaceProvider,
    C: ChangedFilesProvider,
>(
    input: ReviewInput,
    snapshot_provider: &S,
    changed_files_provider: &C,
) -> Result<ReviewLoadResult, ReviewError> {
    let base_selector = input.base.clone();
    let ReviewLoadResult {
        session,
        diagnostics,
    } = load_review_with_providers(input, snapshot_provider)?;

    let changed = changed_files_provider
        .changed_files(&base_selector)
        .map_err(|source| ReviewError::ChangedFiles {
            selector: base_selector,
            source,
        })?;

    let diff = diff_objects(&session);
    let impact = compute_impact(&diff, &changed);
    let reviewers = required_reviewers(&diff, &impact);

    Ok(ReviewLoadResult {
        session: ReviewSession {
            impact,
            required_reviewers: reviewers,
            ..session
        },
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
///
/// Each `Changed` entry is decorated with its V3.2
/// [`FieldChange`] projection via [`field_changes`].
pub fn diff_objects(session: &ReviewSession) -> ObjectDiff {
    let base = extract_knowledge_objects(&session.base);
    let head = extract_knowledge_objects(&session.head);
    let mut diff = ObjectDiff::compute(&base, &head);
    for entry in diff.changed_mut() {
        let projection = project_changed(entry);
        entry.field_changes = projection;
    }
    diff
}

/// Pure projection over an [`ObjectChange`] — explains, in V3.2's typed
/// vocabulary, what differs between the base and head sides of a `Changed`
/// entry. `Created` and `Deleted` variants project to an empty vector; the
/// full before/after record already lives in the diff envelope.
///
/// See V3-DESIGN.md §V3.2 for the variant list and §"Boundary Invariants"
/// for the set-diff (not list-diff) semantics on relations.
// V3.4 will reuse this projection for obligation dispatch via
// `obligations_for_change(&ObjectChange)`. V3.2 itself decorates the diff
// inline via `project_changed`, so the public function is currently
// exercised only by tests; `#[allow(dead_code)]` documents the deferred
// consumer rather than silencing a real warning (matches the
// `ObjectChange` precedent in `domain/review/object_change.rs:20`).
#[allow(dead_code)]
pub fn field_changes(change: &ObjectChange) -> Vec<FieldChange> {
    match change {
        ObjectChange::Created { .. } | ObjectChange::Deleted { .. } => Vec::new(),
        ObjectChange::Changed(c) => project_changed(c),
    }
}

const V0_EVIDENCE_FIELDS: [&str; 3] = [SOURCE_FIELD, TEST_FIELD, REVIEWED_BY_FIELD];

fn project_changed(c: &ChangedObject) -> Vec<FieldChange> {
    let mut out = Vec::new();
    let base = &c.base;
    let head = &c.head;

    if base.body != head.body {
        out.push(FieldChange::Body {
            before: base.body.clone(),
            after: head.body.clone(),
        });
    }

    if base.status != head.status {
        out.push(FieldChange::Status {
            before: base.status.clone(),
            after: head.status.clone(),
        });
    }

    let owner_before = base.fields.get(OWNER_FIELD).cloned();
    let owner_after = head.fields.get(OWNER_FIELD).cloned();
    if owner_before != owner_after {
        out.push(FieldChange::Owner {
            before: owner_before,
            after: owner_after,
        });
    }

    let verified_at_before = base.fields.get(VERIFIED_AT_FIELD).cloned();
    let verified_at_after = head.fields.get(VERIFIED_AT_FIELD).cloned();
    if verified_at_before != verified_at_after {
        out.push(FieldChange::VerifiedAt {
            before: verified_at_before,
            after: verified_at_after,
        });
    }

    // Strict presence/absence on the V0 evidence keys. A value-only change to
    // an evidence field emits nothing — consumers see the diff in the full
    // before/after records, and V3.4's "Evidence removal → re-evidence"
    // obligation rule must not fire on edits that only update the value.
    for key in V0_EVIDENCE_FIELDS {
        let base_value = base.fields.get(key);
        let head_value = head.fields.get(key);
        match (base_value, head_value) {
            (None, Some(after)) => out.push(FieldChange::EvidenceAdded {
                field: key.to_string(),
                value: after.clone(),
            }),
            (Some(before), None) => out.push(FieldChange::EvidenceRemoved {
                field: key.to_string(),
                value: before.clone(),
            }),
            _ => {}
        }
    }

    project_relation(
        &mut out,
        RelationKind::DependsOn,
        &base.relations.depends_on,
        &head.relations.depends_on,
    );
    project_relation(
        &mut out,
        RelationKind::Supersedes,
        &base.relations.supersedes,
        &head.relations.supersedes,
    );
    project_relation(
        &mut out,
        RelationKind::RelatedTo,
        &base.relations.related_to,
        &head.relations.related_to,
    );

    project_impacts(&mut out, &base.impacts, &head.impacts);

    out
}

fn project_impacts(out: &mut Vec<FieldChange>, base: &[String], head: &[String]) {
    use std::collections::BTreeSet;
    let base_set: BTreeSet<&str> = base.iter().map(String::as_str).collect();
    let head_set: BTreeSet<&str> = head.iter().map(String::as_str).collect();
    for path in head_set.difference(&base_set) {
        out.push(FieldChange::ImpactsAdded {
            path: (*path).to_string(),
        });
    }
    for path in base_set.difference(&head_set) {
        out.push(FieldChange::ImpactsRemoved {
            path: (*path).to_string(),
        });
    }
}

fn project_relation(
    out: &mut Vec<FieldChange>,
    kind: RelationKind,
    base: &[String],
    head: &[String],
) {
    use std::collections::BTreeSet;
    let base_set: BTreeSet<&str> = base.iter().map(String::as_str).collect();
    let head_set: BTreeSet<&str> = head.iter().map(String::as_str).collect();
    for target in head_set.difference(&base_set) {
        out.push(FieldChange::RelationAdded {
            kind,
            target: (*target).to_string(),
        });
    }
    for target in base_set.difference(&head_set) {
        out.push(FieldChange::RelationRemoved {
            kind,
            target: (*target).to_string(),
        });
    }
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

    /// Changed entries in deterministic sort order. Exposed so the CLI can
    /// render the V3.2 field-level projection beneath each id.
    pub fn changed(&self) -> &[ChangedObject] {
        &self.changed
    }
}

/// Wire envelope for `adoc.review.v0` (V3.3). Embeds the V3.1 diff envelope
/// alongside the V3.3 impact and required-reviewer projections. The schema
/// stays at `v0` across V3 — later slices add optional fields (proof
/// obligations in V3.4, patch_check in V3.7); tolerant readers required.
#[derive(Debug, Clone, Serialize)]
pub struct ReviewEnvelope {
    pub schema_version: &'static str,
    pub diff: ObjectDiffEnvelope,
    pub impact: Vec<ImpactedObject>,
    pub required_reviewers: Vec<RequiredReviewer>,
    pub diagnostics: Vec<Diagnostic>,
}

impl ReviewEnvelope {
    /// Build the wire envelope from a loaded [`ReviewSession`] and the diff
    /// computed against it. The session's `impact_analysis` and
    /// `required_reviewers` are cloned in.
    pub fn from_session(session: &ReviewSession, diagnostics: Vec<Diagnostic>) -> Self {
        let diff = diff_objects(session);
        Self {
            schema_version: REVIEW_SCHEMA_VERSION,
            diff: ObjectDiffEnvelope::from_diff(diff, Vec::new()),
            impact: session.impact_analysis().to_vec(),
            required_reviewers: session.required_reviewers().to_vec(),
            diagnostics,
        }
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

    // ----- V3.2 field-level projection -----

    mod field_changes_projection {
        use std::collections::BTreeMap;

        use crate::application::review::{V0_EVIDENCE_FIELDS, field_changes, project_changed};
        use crate::domain::graph::{GraphKnowledgeObjectNode, GraphRelations, GraphSourceSpan};
        use crate::domain::review::field_change::{FieldChange, RelationKind};
        use crate::domain::review::object_change::{ChangedObject, ObjectChange};

        fn node(
            id: &str,
            content_hash: &str,
            body: &str,
            status: Option<&str>,
            fields: BTreeMap<String, String>,
            relations: GraphRelations,
        ) -> GraphKnowledgeObjectNode {
            GraphKnowledgeObjectNode {
                id: id.to_string(),
                kind: "claim".to_string(),
                content_hash: content_hash.to_string(),
                status: status.map(str::to_string),
                body: body.to_string(),
                page_id: "team.billing".to_string(),
                source_span: GraphSourceSpan {
                    path: "docs/billing.adoc".to_string(),
                    line: 1,
                    column: 1,
                },
                fields,
                relations,
                impacts: Vec::new(),
            }
        }

        fn baseline(body: &str) -> GraphKnowledgeObjectNode {
            node(
                "billing.credits",
                "sha256:base",
                body,
                Some("draft"),
                BTreeMap::new(),
                GraphRelations::default(),
            )
        }

        fn changed_from(
            base: GraphKnowledgeObjectNode,
            head: GraphKnowledgeObjectNode,
        ) -> ChangedObject {
            ChangedObject::new("billing.credits".to_string(), base, head)
        }

        #[test]
        fn identical_records_produce_empty_projection() {
            let base = baseline("Credits apply.");
            let head = baseline("Credits apply.");
            let c = changed_from(base, head);

            assert!(project_changed(&c).is_empty());
        }

        #[test]
        fn body_only_change_produces_exactly_one_body_field_change() {
            let base = baseline("Old.");
            let head = baseline("New.");
            let c = changed_from(base, head);

            assert_eq!(
                project_changed(&c),
                vec![FieldChange::Body {
                    before: "Old.".to_string(),
                    after: "New.".to_string(),
                }]
            );
        }

        #[test]
        fn status_change_emits_status_field_change_with_optional_sides() {
            let base = node(
                "billing.credits",
                "sha256:a",
                "x",
                Some("draft"),
                BTreeMap::new(),
                GraphRelations::default(),
            );
            let head = node(
                "billing.credits",
                "sha256:b",
                "x",
                Some("verified"),
                BTreeMap::new(),
                GraphRelations::default(),
            );

            assert_eq!(
                project_changed(&changed_from(base, head)),
                vec![FieldChange::Status {
                    before: Some("draft".to_string()),
                    after: Some("verified".to_string()),
                }]
            );
        }

        #[test]
        fn status_appearance_from_none_to_some_emits_status_field_change() {
            let base = node(
                "billing.credits",
                "sha256:a",
                "x",
                None,
                BTreeMap::new(),
                GraphRelations::default(),
            );
            let head = node(
                "billing.credits",
                "sha256:b",
                "x",
                Some("draft"),
                BTreeMap::new(),
                GraphRelations::default(),
            );

            assert_eq!(
                project_changed(&changed_from(base, head)),
                vec![FieldChange::Status {
                    before: None,
                    after: Some("draft".to_string()),
                }]
            );
        }

        #[test]
        fn owner_appearance_emits_owner_field_change_with_none_before() {
            let base = baseline("x");
            let mut head = baseline("x");
            head.fields
                .insert("owner".to_string(), "team-billing".to_string());

            assert_eq!(
                project_changed(&changed_from(base, head)),
                vec![FieldChange::Owner {
                    before: None,
                    after: Some("team-billing".to_string()),
                }]
            );
        }

        #[test]
        fn verified_at_removal_emits_verified_at_field_change_with_none_after() {
            let mut base = baseline("x");
            base.fields
                .insert("verified_at".to_string(), "2026-05-05".to_string());
            let head = baseline("x");

            assert_eq!(
                project_changed(&changed_from(base, head)),
                vec![FieldChange::VerifiedAt {
                    before: Some("2026-05-05".to_string()),
                    after: None,
                }]
            );
        }

        #[test]
        fn evidence_added_when_source_appears_in_head() {
            let base = baseline("x");
            let mut head = baseline("x");
            head.fields
                .insert("source".to_string(), "ledger".to_string());

            assert_eq!(
                project_changed(&changed_from(base, head)),
                vec![FieldChange::EvidenceAdded {
                    field: "source".to_string(),
                    value: "ledger".to_string(),
                }]
            );
        }

        #[test]
        fn evidence_removed_when_test_disappears_in_head() {
            let mut base = baseline("x");
            base.fields
                .insert("test".to_string(), "integration".to_string());
            let head = baseline("x");

            assert_eq!(
                project_changed(&changed_from(base, head)),
                vec![FieldChange::EvidenceRemoved {
                    field: "test".to_string(),
                    value: "integration".to_string(),
                }]
            );
        }

        #[test]
        fn evidence_value_only_change_emits_no_field_change() {
            // Strict presence/absence semantics: source: A -> source: B is
            // not an EvidenceAdded/Removed and not an "EvidenceChanged"
            // (no such variant in V3.2). Consumers must read the full
            // before/after records if they care about value-only edits.
            let mut base = baseline("x");
            base.fields
                .insert("source".to_string(), "ledger-v1".to_string());
            let mut head = baseline("x");
            head.fields
                .insert("source".to_string(), "ledger-v2".to_string());

            assert!(project_changed(&changed_from(base, head)).is_empty());
        }

        #[test]
        fn relation_added_for_new_depends_on_target() {
            let base = baseline("x");
            let mut head = baseline("x");
            head.relations.depends_on = vec!["billing.payments".to_string()];

            assert_eq!(
                project_changed(&changed_from(base, head)),
                vec![FieldChange::RelationAdded {
                    kind: RelationKind::DependsOn,
                    target: "billing.payments".to_string(),
                }]
            );
        }

        #[test]
        fn relation_removed_for_dropped_supersedes_target() {
            let mut base = baseline("x");
            base.relations.supersedes = vec!["billing.legacy-credits".to_string()];
            let head = baseline("x");

            assert_eq!(
                project_changed(&changed_from(base, head)),
                vec![FieldChange::RelationRemoved {
                    kind: RelationKind::Supersedes,
                    target: "billing.legacy-credits".to_string(),
                }]
            );
        }

        #[test]
        fn related_to_relation_uses_related_to_kind() {
            let base = baseline("x");
            let mut head = baseline("x");
            head.relations.related_to = vec!["billing.holds".to_string()];

            assert_eq!(
                project_changed(&changed_from(base, head)),
                vec![FieldChange::RelationAdded {
                    kind: RelationKind::RelatedTo,
                    target: "billing.holds".to_string(),
                }]
            );
        }

        #[test]
        fn relation_array_reorder_with_same_set_produces_empty_projection() {
            let mut base = baseline("x");
            base.relations.depends_on = vec!["b.b".to_string(), "a.a".to_string()];
            let mut head = baseline("x");
            head.relations.depends_on = vec!["a.a".to_string(), "b.b".to_string()];

            assert!(project_changed(&changed_from(base, head)).is_empty());
        }

        #[test]
        fn relation_duplicate_entries_collapse_via_set_semantics() {
            let mut base = baseline("x");
            base.relations.depends_on = vec!["a.a".to_string(), "a.a".to_string()];
            let mut head = baseline("x");
            head.relations.depends_on = vec!["a.a".to_string()];

            assert!(project_changed(&changed_from(base, head)).is_empty());
        }

        #[test]
        fn impacts_added_when_path_appears_in_head() {
            let base = baseline("x");
            let mut head = baseline("x");
            head.impacts = vec!["a.rs".to_string()];

            assert_eq!(
                project_changed(&changed_from(base, head)),
                vec![FieldChange::ImpactsAdded {
                    path: "a.rs".to_string(),
                }]
            );
        }

        #[test]
        fn impacts_removed_when_path_disappears_in_head() {
            let mut base = baseline("x");
            base.impacts = vec!["a.rs".to_string()];
            let head = baseline("x");

            assert_eq!(
                project_changed(&changed_from(base, head)),
                vec![FieldChange::ImpactsRemoved {
                    path: "a.rs".to_string(),
                }]
            );
        }

        #[test]
        fn impacts_set_reorder_with_same_set_produces_empty_projection() {
            let mut base = baseline("x");
            base.impacts = vec!["a.rs".to_string(), "b.rs".to_string()];
            let mut head = baseline("x");
            head.impacts = vec!["b.rs".to_string(), "a.rs".to_string()];

            assert!(project_changed(&changed_from(base, head)).is_empty());
        }

        #[test]
        fn impacts_added_and_removed_emit_sorted_per_kind() {
            let mut base = baseline("x");
            base.impacts = vec!["keep.rs".to_string(), "drop.rs".to_string()];
            let mut head = baseline("x");
            head.impacts = vec!["keep.rs".to_string(), "add.rs".to_string()];

            assert_eq!(
                project_changed(&changed_from(base, head)),
                vec![
                    FieldChange::ImpactsAdded {
                        path: "add.rs".to_string(),
                    },
                    FieldChange::ImpactsRemoved {
                        path: "drop.rs".to_string(),
                    },
                ]
            );
        }

        #[test]
        fn multiple_changes_appear_in_deterministic_visit_order() {
            // Order: body, status, owner, verified_at, evidence (source,
            // test, reviewed_by), relations (depends_on, supersedes,
            // related_to). Matches the documented visit order in
            // project_changed.
            let base = node(
                "billing.credits",
                "sha256:a",
                "old",
                Some("draft"),
                BTreeMap::new(),
                GraphRelations::default(),
            );
            let mut head_fields = BTreeMap::new();
            head_fields.insert("owner".to_string(), "team-billing".to_string());
            head_fields.insert("source".to_string(), "ledger".to_string());
            let head = node(
                "billing.credits",
                "sha256:b",
                "new",
                Some("verified"),
                head_fields,
                GraphRelations {
                    depends_on: vec!["billing.payments".to_string()],
                    ..GraphRelations::default()
                },
            );

            let changes = project_changed(&changed_from(base, head));

            assert_eq!(
                changes,
                vec![
                    FieldChange::Body {
                        before: "old".to_string(),
                        after: "new".to_string(),
                    },
                    FieldChange::Status {
                        before: Some("draft".to_string()),
                        after: Some("verified".to_string()),
                    },
                    FieldChange::Owner {
                        before: None,
                        after: Some("team-billing".to_string()),
                    },
                    FieldChange::EvidenceAdded {
                        field: "source".to_string(),
                        value: "ledger".to_string(),
                    },
                    FieldChange::RelationAdded {
                        kind: RelationKind::DependsOn,
                        target: "billing.payments".to_string(),
                    },
                ]
            );
        }

        #[test]
        fn field_changes_returns_empty_for_created_variant() {
            let record = baseline("x");
            let change = ObjectChange::Created { record };

            assert!(field_changes(&change).is_empty());
        }

        #[test]
        fn field_changes_returns_empty_for_deleted_variant() {
            let record = baseline("x");
            let change = ObjectChange::Deleted { record };

            assert!(field_changes(&change).is_empty());
        }

        #[test]
        fn field_changes_dispatches_to_project_changed_for_changed_variant() {
            let base = baseline("old");
            let head = baseline("new");
            let inner = changed_from(base, head);

            // Wrapping the same ChangedObject in an ObjectChange::Changed must
            // produce the same projection as calling project_changed directly.
            let projected = field_changes(&ObjectChange::Changed(Box::new(inner.clone())));

            assert_eq!(projected, project_changed(&inner));
            assert_eq!(projected.len(), 1);
        }

        #[test]
        fn v0_evidence_fields_constant_is_aligned_with_claim_module() {
            assert_eq!(V0_EVIDENCE_FIELDS, ["source", "test", "reviewed_by"]);
        }

        #[test]
        fn diff_objects_decoration_step_populates_field_changes_on_each_changed_entry() {
            // ObjectDiff::compute only emits a Changed entry when
            // content_hash differs, so base and head need distinct hashes
            // even if the rest of the record varies on its own. The actual
            // graph builder guarantees hash-vs-content correspondence —
            // tests fake it.
            let mut base = baseline("old");
            base.content_hash = "sha256:base-hash".to_string();
            let mut head = baseline("new");
            head.content_hash = "sha256:head-hash".to_string();

            let diff = crate::domain::review::object_diff::ObjectDiff::compute(
                std::slice::from_ref(&base),
                std::slice::from_ref(&head),
            );

            // Un-decorated diff: decoration is the application layer's job.
            assert_eq!(diff.changed().len(), 1);
            assert!(diff.changed()[0].field_changes().is_empty());

            // Run the same decoration step diff_objects() performs.
            let mut decorated = diff;
            for entry in decorated.changed_mut() {
                entry.field_changes = project_changed(entry);
            }
            assert_eq!(
                decorated.changed()[0].field_changes(),
                &[FieldChange::Body {
                    before: "old".to_string(),
                    after: "new".to_string(),
                }]
            );
        }
    }
}
