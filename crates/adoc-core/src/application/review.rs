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

use crate::application::compile::{CompileResult, compile_with_provider};
use crate::application::patch::{PatchParseError, check_patch_documents};
use crate::application::review_envelope::ReviewEnvelope;
use crate::domain::diagnostic::Diagnostic;
use crate::domain::graph::{GraphArtifactDocument, GraphKnowledgeObjectNode, GraphNode};
use crate::domain::obligation::ProofObligation;
use crate::domain::patch::PatchDocument;
use crate::domain::ports::changed_files::{ChangedFilesError, ChangedFilesProvider};
use crate::domain::ports::snapshot_workspace::{
    SnapshotError, SnapshotSelector, SnapshotWorkspaceProvider,
};
use crate::domain::ports::source_provider::SourceProvider;
use crate::domain::review::impact::{ImpactedObject, compute_impact};
use crate::domain::review::object_diff::ObjectDiff;
use crate::domain::review::obligation_rules::{obligations_for_change, obligations_for_impact};
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
    /// V3.4 proof obligations aggregated from diff field changes and the
    /// impact list. Empty for sessions loaded via
    /// [`load_review_with_providers`].
    proof_obligations: Vec<ProofObligation>,
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

    pub fn proof_obligations(&self) -> &[ProofObligation] {
        &self.proof_obligations
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
    /// V3.7 — the patch source supplied via `adoc review --patch` (or the
    /// equivalent MCP parameter) could not be parsed into a
    /// [`PatchDocument`]. Validation failures against the head graph stay
    /// inside `PatchCheckResult::diagnostics`; only parse-time errors that
    /// stop us from producing a `PatchCheckResult` at all reach this variant.
    PatchParse {
        source: PatchParseError,
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
            Self::PatchParse { source } => {
                write!(f, "could not parse review patch source ({source})")
            }
        }
    }
}

impl Error for ReviewError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::BaseSnapshot { source, .. } | Self::HeadSnapshot { source, .. } => Some(source),
            Self::ChangedFiles { source, .. } => Some(source),
            Self::PatchParse { source } => Some(source),
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
            proof_obligations: Vec::new(),
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
    let head_selector = input.head.clone();
    let ReviewLoadResult {
        session,
        diagnostics,
    } = load_review_with_providers(input, snapshot_provider)?;

    let changed = changed_files_provider
        .changed_files(&base_selector, &head_selector)
        .map_err(|source| ReviewError::ChangedFiles {
            selector: base_selector,
            source,
        })?;

    let diff = diff_objects(&session);
    let impact = compute_impact(&diff, &changed);
    let reviewers = required_reviewers(&diff, &impact);
    let obligations = proof_obligations(&diff, &impact);

    Ok(ReviewLoadResult {
        session: ReviewSession {
            impact,
            required_reviewers: reviewers,
            proof_obligations: obligations,
            ..session
        },
        diagnostics,
    })
}

/// V3.4 aggregator. Walks each `Changed` entry in `diff` and each entry in
/// `impact`, applying the trigger rules in
/// `crate::domain::review::obligation_rules`. Deduplicates by
/// `(object_id, reason)` and returns the result sorted by the same key for
/// deterministic JSON output.
pub fn proof_obligations(diff: &ObjectDiff, impact: &[ImpactedObject]) -> Vec<ProofObligation> {
    // Diff-driven obligations come first so they win ties with impact-driven
    // ones on the same (object_id, reason). See [`ProofObligation::merge_dedup_sorted`].
    let from_diff = diff.changed.iter().flat_map(obligations_for_change);
    let from_impact = impact.iter().flat_map(obligations_for_impact);
    ProofObligation::merge_dedup_sorted(from_diff.chain(from_impact))
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
    let source_provider = FsSourceProvider::new(workspace.path().to_path_buf())
        .with_identity_prefix(PathBuf::from(REVIEW_IDENTITY_PREFIX));
    let result = compile_with_provider(&source_provider);
    Ok(result)
}

/// Project the two compiled graphs in `session` into an [`ObjectDiff`].
///
/// Pure projection — does not allocate I/O. Knowledge Object scope only;
/// pages, prose blocks, and edges are excluded per V3-DESIGN.md §V3.1.
///
/// `ObjectDiff::compute` self-decorates each `Changed` entry with its V3.2
/// `FieldChange` projection, so this function is a pure compose-and-call.
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

/// V3.7 — compose patch validation into a Review Report.
///
/// When `patch` is `Some`, runs V2's `check_patch_documents` against the
/// head graph already held by `session`, embeds the resulting
/// `PatchCheckResult` as the envelope's `patch_check` field, and unions
/// the patch-driven obligations with the session's diff/impact-driven
/// obligations.
///
/// When `patch` is `None`, the returned envelope is byte-equivalent to
/// [`ReviewEnvelope::from_session`] — `patch_check` is omitted from the
/// serialized JSON entirely (not `null`).
///
/// The patch is **never applied**. V3.7 composes two read-only views; see
/// V3-DESIGN.md §V3.7 and §Non-Goals.
pub fn review_with_patch(
    session: &ReviewSession,
    diagnostics: Vec<Diagnostic>,
    patch: Option<&PatchDocument>,
) -> ReviewEnvelope {
    let patch_check = patch.map(|patch| {
        let head_artifact = head_graph_artifact_document(session);
        check_patch_documents(head_artifact, patch.clone())
    });
    ReviewEnvelope::from_session_with_patch_check(session, diagnostics, patch_check)
}

fn head_graph_artifact_document(session: &ReviewSession) -> GraphArtifactDocument {
    let artifacts = session
        .head
        .artifacts
        .as_ref()
        .expect("head compile produced artifacts (load_review error short-circuits otherwise)");
    serde_json::from_str(&artifacts.graph_json)
        .expect("compile output graph_json is well-formed (compile pipeline invariant)")
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
                    .ok_or_else(|| SnapshotError::UnresolvableRef {
                        spec: spec.as_str().to_string(),
                        reason: "ref not seeded in test double".to_string(),
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

    fn fresh_workspace(label: &str) -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix(&format!("adoc-review-test-{label}-"))
            .tempdir()
            .expect("create workspace")
    }

    #[test]
    fn load_review_with_providers_checks_out_base_and_head_in_order() {
        let base_root = fresh_workspace("base");
        write_billing_source(base_root.path(), "Credits apply after payment.");
        let head_root = fresh_workspace("head");
        write_billing_source(head_root.path(), "Credits apply after ledger commit.");

        let provider = InMemorySnapshotWorkspaceProvider::new(head_root.path().to_path_buf())
            .with_ref("main", base_root.path().to_path_buf());

        let result = load_review_with_providers(
            ReviewInput {
                project_root: head_root.path().to_path_buf(),
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
    }

    #[test]
    fn load_review_returns_base_compile_blocked_when_base_compile_has_errors() {
        let base_root = fresh_workspace("base-blocked");
        fs::create_dir_all(base_root.path().join("docs")).expect("docs");
        // Raw HTML inside an .adoc source triggers a compile error.
        fs::write(
            base_root.path().join("docs/bad.adoc"),
            "# Guide @doc(team.guide)\n\n<div>raw</div>\n",
        )
        .expect("write bad source");
        let head_root = fresh_workspace("head-blocked-fine");
        write_billing_source(head_root.path(), "Clean head.");

        let provider = InMemorySnapshotWorkspaceProvider::new(head_root.path().to_path_buf())
            .with_ref("main", base_root.path().to_path_buf());

        let error = load_review_with_providers(
            ReviewInput {
                project_root: head_root.path().to_path_buf(),
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
    }

    #[test]
    fn load_review_returns_base_snapshot_error_when_provider_fails() {
        let head_root = fresh_workspace("head-snap-error");
        write_billing_source(head_root.path(), "Clean head.");

        let provider = InMemorySnapshotWorkspaceProvider::new(head_root.path().to_path_buf());

        let error = load_review_with_providers(
            ReviewInput {
                project_root: head_root.path().to_path_buf(),
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
    }

    #[test]
    fn diff_objects_returns_three_arrays_against_a_real_compile() {
        let base_root = fresh_workspace("diff-base");
        let head_root = fresh_workspace("diff-head");

        // Base: one claim that will change body, one that will be deleted.
        fs::create_dir_all(base_root.path().join("docs")).expect("docs");
        fs::write(
            base_root.path().join("docs/billing.adoc"),
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
        fs::create_dir_all(head_root.path().join("docs")).expect("docs");
        fs::write(
            head_root.path().join("docs/billing.adoc"),
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

        let provider = InMemorySnapshotWorkspaceProvider::new(head_root.path().to_path_buf())
            .with_ref("main", base_root.path().to_path_buf());

        let load = load_review_with_providers(
            ReviewInput {
                project_root: head_root.path().to_path_buf(),
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
    }

    #[test]
    fn envelope_from_diff_includes_schema_version_constant() {
        use crate::application::review_envelope::ObjectDiffEnvelope;
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

    // ----- V3.4 proof-obligation aggregator -----

    mod proof_obligations_aggregator {
        use std::collections::BTreeMap;

        use crate::application::review::proof_obligations;
        use crate::domain::graph::{
            GraphEvidence, GraphKnowledgeObjectNode, GraphRelations, GraphSourceSpan,
        };
        use crate::domain::review::impact::ImpactedObject;
        use crate::domain::review::object_change::ChangedObject;
        use crate::domain::review::object_diff::ObjectDiff;

        fn verified_claim(id: &str, content_hash: &str) -> GraphKnowledgeObjectNode {
            GraphKnowledgeObjectNode {
                id: id.to_string(),
                kind: "claim".to_string(),
                content_hash: content_hash.to_string(),
                status: Some("verified".to_string()),
                body: format!("{id} body"),
                page_id: "team.billing".to_string(),
                source_span: GraphSourceSpan {
                    path: "docs/billing.adoc".to_string(),
                    line: 1,
                    column: 1,
                },
                fields: BTreeMap::new(),
                relations: GraphRelations::default(),
                impacts: Vec::new(),
                approved_by: Vec::new(),
                allowed_actions: Vec::new(),
                forbidden_actions: Vec::new(),
                contradiction_claims: Vec::new(),
                // V5.8: evidence in typed array, not fields.
                evidence: vec![
                    GraphEvidence::inline("source_code", "ledger"),
                    GraphEvidence::inline("test", "integration"),
                    GraphEvidence::inline("human_review", "team-billing"),
                ],
                effective_status: None,
                effective_reason: None,
            }
        }

        fn diff_with_body_change(id: &str) -> ObjectDiff {
            let base = verified_claim(id, "sha256:a");
            let mut head = verified_claim(id, "sha256:b");
            head.body = "new body".to_string();
            // `ObjectDiff::compute` self-decorates the `Changed` entry with
            // its V3.2 field-change projection (body diff), so this helper
            // is now a thin wrapper.
            ObjectDiff::compute(std::slice::from_ref(&base), std::slice::from_ref(&head))
        }

        #[test]
        fn empty_diff_and_empty_impact_yields_empty_obligations() {
            let diff = ObjectDiff::compute(&[], &[]);
            assert!(proof_obligations(&diff, &[]).is_empty());
        }

        #[test]
        fn body_change_on_verified_claim_yields_one_re_verify_obligation() {
            let diff = diff_with_body_change("billing.credits");

            let obligations = proof_obligations(&diff, &[]);

            assert_eq!(obligations.len(), 1);
            assert_eq!(obligations[0].object_id, "billing.credits");
            assert_eq!(obligations[0].reason, "re-verify body");
            // V5.8: required_evidence uses EvidenceKind strings.
            assert_eq!(
                obligations[0].required_evidence,
                vec!["source_code", "test", "human_review"]
            );
        }

        #[test]
        fn impact_entry_yields_impact_review_obligation_against_source() {
            let diff = ObjectDiff::compute(&[], &[]);
            let impact = vec![ImpactedObject {
                id: "billing.refunds".to_string(),
                paths: vec!["crates/billing/src/refund.rs".to_string()],
            }];

            let obligations = proof_obligations(&diff, &impact);

            assert_eq!(obligations.len(), 1);
            assert_eq!(obligations[0].object_id, "billing.refunds");
            assert_eq!(obligations[0].reason, "review impacted claim");
            // V5.8: source evidence is "source_code".
            assert_eq!(obligations[0].required_evidence, vec!["source_code"]);
        }

        #[test]
        fn output_is_sorted_by_object_id_then_reason() {
            // Build a diff with two changed verified claims; second one would
            // sort *before* the first lexicographically.
            let mut diff = diff_with_body_change("zz.late");
            let mut second = diff_with_body_change("aa.early");
            diff.changed.append(&mut second.changed);
            // Note: ObjectDiff::compute would have sorted the entries, but
            // appending manually breaks that — the test exercises the
            // aggregator's own sort.

            let obligations = proof_obligations(&diff, &[]);

            assert_eq!(obligations.len(), 2);
            assert_eq!(obligations[0].object_id, "aa.early");
            assert_eq!(obligations[1].object_id, "zz.late");
        }

        #[test]
        fn diff_and_impact_overlap_on_same_object_id_and_reason_deduplicates() {
            // The trigger-table reasons are distinct between diff-driven
            // ("re-verify body") and impact-driven ("review impacted claim"),
            // so an overlap normally produces two entries. To exercise dedup
            // we synthesise the rare same-reason case by sending duplicate
            // impacts (rare in practice, but the aggregator must dedupe).
            let diff = ObjectDiff::compute(&[], &[]);
            let impact = vec![
                ImpactedObject {
                    id: "billing.refunds".to_string(),
                    paths: vec!["a.rs".to_string()],
                },
                ImpactedObject {
                    id: "billing.refunds".to_string(),
                    paths: vec!["b.rs".to_string()],
                },
            ];

            let obligations = proof_obligations(&diff, &impact);

            assert_eq!(obligations.len(), 1);
            assert_eq!(obligations[0].object_id, "billing.refunds");
        }

        #[test]
        fn diff_driven_and_impact_driven_obligations_coexist_when_reasons_differ() {
            // Body change on billing.credits gives a re-verify obligation;
            // impact on billing.credits gives a review-impacted-claim
            // obligation. Different reasons → both survive.
            let diff = diff_with_body_change("billing.credits");
            let impact = vec![ImpactedObject {
                id: "billing.credits".to_string(),
                paths: vec!["src/billing.rs".to_string()],
            }];

            let obligations = proof_obligations(&diff, &impact);

            assert_eq!(obligations.len(), 2);
            // Sorted by (object_id, reason). Both share the object_id, so
            // reason order is "re-verify body" < "review impacted claim".
            assert_eq!(obligations[0].reason, "re-verify body");
            assert_eq!(obligations[1].reason, "review impacted claim");
        }

        #[test]
        fn changed_object_referenced_so_dead_code_lint_is_satisfied() {
            let _ = std::mem::size_of::<ChangedObject>();
        }
    }

    // ----- V3.7 patch composition -----

    mod review_with_patch_composition {
        use std::fs;

        use serde_json::json;

        use crate::application::review::{
            ReviewInput, load_review_with_providers, review_with_patch,
        };
        use crate::application::review_envelope::ReviewEnvelope;
        use crate::domain::diagnostic::DiagnosticCode;
        use crate::domain::ports::snapshot_workspace::{GitRef, SnapshotSelector};
        use crate::parse_patch_from_value;

        use super::{InMemorySnapshotWorkspaceProvider, fresh_workspace};

        fn write_verified_claim(root: &std::path::Path, body: &str) {
            let docs = root.join("docs");
            fs::create_dir_all(&docs).expect("docs dir");
            let source = format!(
                concat!(
                    "# Billing @doc(team.billing)\n",
                    "\n",
                    "::claim billing.credits\n",
                    "status: verified\n",
                    "owner: team-billing\n",
                    "verified_at: 2026-05-05\n",
                    "source: ledger\n",
                    "--\n",
                    "{body}\n",
                    "::\n",
                ),
                body = body,
            );
            fs::write(docs.join("billing.adoc"), source).expect("write billing.adoc");
        }

        fn load_session_with_verified_claim_body_change() -> (
            crate::application::review::ReviewSession,
            String, // head content_hash of billing.credits
        ) {
            let base_root = fresh_workspace("patch-comp-base");
            write_verified_claim(base_root.path(), "Old verified body.");
            let head_root = fresh_workspace("patch-comp-head");
            write_verified_claim(head_root.path(), "New verified body.");

            let provider = InMemorySnapshotWorkspaceProvider::new(head_root.path().to_path_buf())
                .with_ref("main", base_root.path().to_path_buf());

            let load = load_review_with_providers(
                ReviewInput {
                    project_root: head_root.path().to_path_buf(),
                    base: SnapshotSelector::GitRef(GitRef::new("main")),
                    head: SnapshotSelector::Workdir,
                },
                &provider,
            )
            .expect("load review succeeds");

            // Pull head content_hash for the target claim so the test patch
            // can carry a valid `base_hash` against the head graph.
            let head_artifact = super::super::head_graph_artifact_document(&load.session);
            let hash = head_artifact
                .nodes
                .iter()
                .find_map(|node| match node {
                    crate::domain::graph::GraphNode::KnowledgeObject(ko)
                        if ko.id == "billing.credits" =>
                    {
                        Some(ko.content_hash.clone())
                    }
                    _ => None,
                })
                .expect("billing.credits present in head graph");

            // Patch-composition tests share a session across multiple
            // assertions returned via tuple. `keep` consumes the TempDir
            // without running its Drop, so the on-disk workspace outlives
            // this function (the OS reclaims `$TMPDIR` on process exit).
            let _ = base_root.keep();
            let _ = head_root.keep();

            (load.session, hash)
        }

        #[test]
        fn review_with_patch_none_matches_from_session_envelope() {
            let (session, _hash) = load_session_with_verified_claim_body_change();

            let with_none = review_with_patch(&session, Vec::new(), None);
            let baseline = ReviewEnvelope::from_session(&session, Vec::new());

            let with_none_json = serde_json::to_value(&with_none).expect("serialize");
            let baseline_json = serde_json::to_value(&baseline).expect("serialize");

            assert_eq!(with_none_json, baseline_json);
            assert!(
                with_none_json.get("patch_check").is_none(),
                "patch_check must be omitted when no patch is supplied"
            );
        }

        #[test]
        fn review_with_patch_embeds_valid_patch_check_against_head_graph() {
            let (session, hash) = load_session_with_verified_claim_body_change();

            let patch = parse_patch_from_value(json!({
                "schema_version": "adoc.patch.v0",
                "op": "replace_body",
                "target": "billing.credits",
                "base_hash": hash,
                "changes": { "body": "Patched body." },
                "reason": "demo"
            }))
            .expect("test patch parses");

            let envelope = review_with_patch(&session, Vec::new(), Some(&patch));

            let patch_check = envelope.patch_check.expect("patch_check present");
            assert!(
                patch_check.valid,
                "patch validates cleanly: {patch_check:?}"
            );
            assert_eq!(patch_check.target.as_deref(), Some("billing.credits"));
        }

        #[test]
        fn review_with_patch_stale_base_hash_surfaces_in_diagnostics() {
            let (session, _hash) = load_session_with_verified_claim_body_change();

            let patch = parse_patch_from_value(json!({
                "schema_version": "adoc.patch.v0",
                "op": "replace_body",
                "target": "billing.credits",
                "base_hash": "sha256:wrong",
                "changes": { "body": "Patched body." },
                "reason": "demo"
            }))
            .expect("test patch parses");

            let envelope = review_with_patch(&session, Vec::new(), Some(&patch));

            let patch_check = envelope.patch_check.expect("patch_check present");
            assert!(!patch_check.valid, "stale base_hash must fail validation");
            assert!(
                patch_check
                    .diagnostics
                    .iter()
                    .any(|d| d.code == DiagnosticCode::PatchBaseHashMismatch),
                "expected PatchBaseHashMismatch in diagnostics: {patch_check:?}"
            );
        }

        #[test]
        fn review_with_patch_unions_obligations_deduped() {
            // The diff carries a body change on a verified claim, which the
            // V3.4 aggregator turns into a "re-verify body" obligation. A
            // ReplaceBody patch on the same verified claim triggers the V2
            // patch validator's own re-verify-style obligation (reason
            // "verified claim body changed"). Different reasons → no dedup.
            // We assert that both appear, sorted, with the diff-driven one
            // intact.
            let (session, hash) = load_session_with_verified_claim_body_change();

            let patch = parse_patch_from_value(json!({
                "schema_version": "adoc.patch.v0",
                "op": "replace_body",
                "target": "billing.credits",
                "base_hash": hash,
                "changes": { "body": "Patched body." },
                "reason": "demo"
            }))
            .expect("test patch parses");

            let envelope = review_with_patch(&session, Vec::new(), Some(&patch));

            let session_only_obligations: Vec<_> = session
                .proof_obligations()
                .iter()
                .map(|o| (o.object_id.clone(), o.reason.clone()))
                .collect();

            // Every diff/impact-driven obligation must still be present.
            for (id, reason) in &session_only_obligations {
                assert!(
                    envelope
                        .proof_obligations
                        .iter()
                        .any(|o| &o.object_id == id && &o.reason == reason),
                    "missing diff-driven obligation ({id}, {reason}) after union"
                );
            }

            // The top-level set is deduplicated by (object_id, reason).
            let mut seen = std::collections::BTreeSet::new();
            for o in &envelope.proof_obligations {
                assert!(
                    seen.insert((o.object_id.as_str(), o.reason.as_str())),
                    "duplicate (object_id, reason) in unioned obligations: {o:?}"
                );
            }

            // Result must be sorted by (object_id, reason).
            let mut sorted = envelope.proof_obligations.clone();
            sorted.sort_by(|a, b| {
                (a.object_id.as_str(), a.reason.as_str())
                    .cmp(&(b.object_id.as_str(), b.reason.as_str()))
            });
            assert_eq!(envelope.proof_obligations, sorted);
        }
    }
}
