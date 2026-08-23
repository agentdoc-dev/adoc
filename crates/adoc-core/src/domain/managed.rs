//! Managed Object identity contract (E1.2; ADR-0057 invariant 1).
//!
//! KNOWLEDGE-MODEL §K6 separates five identity layers, each its own type:
//!
//! 1. workspace canonical identity — [`WorkspaceCanonicalIdentity`];
//! 2. human-readable Object ID — the graph node `id` (authored as the
//!    `ObjectId` grammar, `domain::identity`);
//! 3. immutable managed version ID — [`ManagedVersionId`];
//! 4. Source Assertion identity — [`SourceAssertionIdentity`] (the Source
//!    Record/Assertion store itself is E4.1);
//! 5. Source Binding — `GraphSourceBinding` (E1.1, ADR-0058 §4).
//!
//! Managed import (RT-03, D36): matching Object IDs, titles, hashes, or
//! semantic similarity never auto-merge. A same-ID collision across
//! distinctly keyed repositories (see [`ManagedRepositoryRecord`] for what
//! the graph identity does and does not distinguish) retains every object
//! distinct and emits a typed [`ReconciliationCandidate`]
//! (`adoc.reconciliation_candidate.v0`, registered planned in
//! CONTRACT-REGISTRY.md); deciding a candidate is a governed E1.3 action,
//! never an import side effect.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::graph::{GraphArtifactDocument, GraphNode, GraphRepositoryIdentity, GraphSourceBinding};
use super::identity::ObjectId;

/// Cloud workspace identifier — opaque to `adoc-core`. Blank or padded
/// input is rejected: an empty workspace id would produce unqualified
/// canonical identities, defeating the qualification that keeps identity
/// unlinkable across workspaces (MILESTONES §E1.2 stop-ship), and a
/// whitespace-padded one would be a second spelling of the same workspace
/// that never compares equal. Never normalized — silently unifying two
/// spellings is a merge nobody decided.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub(crate) struct WorkspaceId(String);

impl WorkspaceId {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, ManagedIdentityError> {
        let value = value.into();
        if value.is_empty() || value.trim() != value {
            Err(ManagedIdentityError::InvalidWorkspaceId)
        } else {
            Ok(Self(value))
        }
    }
}

/// K6 layer 1: the workspace-qualified canonical identity of a Managed
/// Object, stored separately from the human-readable Object ID (RT-03).
/// Equality includes the workspace, so identities from different
/// workspaces never compare equal even when their opaque suffix matches.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(crate) struct WorkspaceCanonicalIdentity {
    pub(crate) workspace_id: WorkspaceId,
    pub(crate) canonical_id: String,
}

/// K6 layer 3: the unique immutable version ID every managed candidate or
/// active version receives. Opaque, never reused, never derived from
/// content — a semantic content change mints a new one (RT-04).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub(crate) struct ManagedVersionId(String);

/// K6 layer 4: the identity of one immutable Source Assertion (K7). Only
/// the identity layer lands in E1.2 — the Source Record/Assertion store is
/// E4.1. Blank or whitespace-padded identities fail closed (same posture
/// as [`WorkspaceId`]: identity values are never normalized).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub(crate) struct SourceAssertionIdentity(String);

impl SourceAssertionIdentity {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, ManagedIdentityError> {
        let value = value.into();
        if value.is_empty() || value.trim() != value {
            Err(ManagedIdentityError::InvalidSourceAssertionIdentity)
        } else {
            Ok(Self(value))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ManagedIdentityError {
    #[error("workspace id must be non-blank without surrounding whitespace")]
    InvalidWorkspaceId,
    #[error("source assertion identity must be non-blank without surrounding whitespace")]
    InvalidSourceAssertionIdentity,
    #[error("object id must not be blank")]
    BlankObjectId,
    #[error("object id violates the Object ID grammar")]
    InvalidObjectId,
    #[error("object id appears more than once in one artifact")]
    DuplicateObjectId,
    #[error("content hash must be `sha256:` followed by lowercase hex")]
    InvalidContentHash,
}

/// The published v6 wire grammar for `content_hash`
/// (`docs/agent/v0/schema/graph-artifact.v6.json`: `^sha256:[0-9a-f]+$`).
/// Stricter than the graph loader's pinned non-blank-suffix acceptance
/// (`graph::content_hash_matches_grammar`): at this trust boundary a
/// malformed or padded value would be enshrined in an immutable version
/// and compared verbatim for RT-04 unchanged-ness ever after.
fn content_hash_matches_published_grammar(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|suffix| {
        !suffix.is_empty()
            && suffix
                .bytes()
                .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    })
}

/// One managed repository inside a workspace, keyed on the graph
/// `repository_identity` (`{kind, config_path}` or explicit `null` —
/// required since v5, ADR-0049). The record is the binding slot (V10.3.2):
/// it can exist before the first artifact arrives, and its Object IDs stay
/// repository-local — the same ID in a distinctly keyed repository is a
/// different Managed Object.
///
/// The graph identity names the producing invocation family, not a
/// workspace-unique repository: every project-bound build emits the
/// constant `{local_project, "agentdoc.config.yaml"}`
/// (`FsSourceProvider::repository_identity`, ADR-0049 §7), so two
/// physical repositories importing under the producer identity are one
/// record to this aggregate. Identity-equality keying is therefore the
/// single-repository convenience only. E1.3 adds explicit routing for
/// the multi-repository workspace — a caller-supplied repository key
/// bound from the authenticated channel, never read from the artifact,
/// with the reserved binding slot (V10.3.2) as the disambiguator (PR
/// #150 adjudication); until then the artifact's declared identity *is*
/// the key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManagedRepositoryRecord {
    pub(crate) repository_identity: GraphRepositoryIdentity,
    /// Managed Objects keyed by their repository-local Object ID.
    pub(crate) objects: BTreeMap<String, ManagedObjectRecord>,
}

/// One Managed Object under one repository: a stable Object ID and
/// workspace canonical identity over an append-only list of immutable
/// versions. Never merged, rewritten, or re-homed by import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManagedObjectRecord {
    pub(crate) canonical: WorkspaceCanonicalIdentity,
    /// The human-readable Object ID — persists across revisions
    /// (invariant: non-empty — [`ManagedWorkspace::import_artifact`]
    /// rejects blank graph node ids before recording anything).
    pub(crate) object_id: String,
    /// Append-only: every import that changes governed meaning appends an
    /// immutable version; nothing is ever rewritten (RT-04).
    pub(crate) versions: Vec<ManagedVersionRecord>,
}

/// One immutable managed version with its preserved provenance (RT-03:
/// reconciliation preserves all original Source Records, Assertions, and
/// Bindings).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManagedVersionRecord {
    pub(crate) version_id: ManagedVersionId,
    /// The producer-declared v6 governed-meaning hash, trusted as
    /// declared: grammar-checked at import, never re-derived from node
    /// content (see [`ManagedWorkspace::import_artifact`]).
    pub(crate) content_hash: String,
    /// The exact Source Binding observed when this version was minted,
    /// when the artifact carried one. A same-hash re-observation with a
    /// changed binding (document moved, rename-only source revision) mints
    /// nothing and does **not** refresh this record: versions are
    /// immutable, and a placement change is not a content change (RT-04,
    /// ADR-0058 — placement is excluded from the v6 hash). Re-observation
    /// freshness is the connector/store's concern (E4.1).
    pub(crate) source_binding: Option<GraphSourceBinding>,
    /// Contributing Source Assertions. Empty until the Source
    /// Record/Assertion store lands (E4.1).
    pub(crate) source_assertions: Vec<SourceAssertionIdentity>,
}

/// The typed reconciliation candidate (`adoc.reconciliation_candidate.v0`):
/// emitted when an imported Object ID collides with a Managed Object in
/// another repository of the same workspace. Both objects stay distinct —
/// keep-distinct / link / supersede / merge decisions are governed E1.3
/// actions with exact authority, policy, and version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ReconciliationCandidate {
    /// The colliding human-readable Object ID (identical on both parties
    /// by construction).
    pub(crate) object_id: String,
    pub(crate) reason: ReconciliationReason,
    pub(crate) existing: ReconciliationParty,
    pub(crate) incoming: ReconciliationParty,
}

/// Closed reason vocabulary. Only an exact Object-ID collision produces a
/// candidate — hash equality, title equality, or semantic similarity never
/// do (RT-03/D36), and no such detector exists anywhere in this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReconciliationReason {
    ObjectIdCollision,
}

/// One side of a reconciliation candidate, identified by its workspace
/// canonical identity, repository, and latest immutable version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ReconciliationParty {
    pub(crate) canonical: WorkspaceCanonicalIdentity,
    pub(crate) repository_identity: GraphRepositoryIdentity,
    pub(crate) version_id: ManagedVersionId,
    pub(crate) content_hash: String,
}

/// What one [`ManagedWorkspace::import_artifact`] call did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManagedImportOutcome {
    /// One entry per version minted by this import. Unchanged governed
    /// meaning mints nothing — a re-observation is not a new content
    /// version (RT-04).
    pub(crate) imported: Vec<ImportedVersion>,
    /// A collision is announced exactly once — on the colliding Object
    /// ID's first appearance in a repository — and is not re-derivable
    /// from the aggregate afterwards. The caller owns persisting every
    /// candidate until it is decided (durability is the Cloud cut,
    /// E1.2.T3; decisions are E1.3).
    pub(crate) reconciliation_candidates: Vec<ReconciliationCandidate>,
}

/// One minted version: the identity layers assigned to an imported object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImportedVersion {
    pub(crate) object_id: String,
    pub(crate) canonical: WorkspaceCanonicalIdentity,
    pub(crate) version_id: ManagedVersionId,
}

/// The managed-import domain service: one Cloud workspace's managed
/// repositories, objects, and versions, in memory. Durable storage is the
/// Cloud cut (E1.2.T3); this aggregate is the executable identity
/// contract the stacked E1 slices build on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManagedWorkspace {
    workspace_id: WorkspaceId,
    repositories: BTreeMap<GraphRepositoryIdentity, ManagedRepositoryRecord>,
    // ponytail: workspace-local monotonic minting; both ids are opaque to
    // every consumer — Cloud storage mints its own scheme (the contract is
    // opacity and uniqueness, not the format).
    minted_canonical_ids: u64,
    minted_version_ids: u64,
}

impl ManagedWorkspace {
    pub(crate) fn new(workspace_id: WorkspaceId) -> Self {
        Self {
            workspace_id,
            repositories: BTreeMap::new(),
            minted_canonical_ids: 0,
            minted_version_ids: 0,
        }
    }

    pub(crate) fn repository(
        &self,
        repository_identity: &GraphRepositoryIdentity,
    ) -> Option<&ManagedRepositoryRecord> {
        self.repositories.get(repository_identity)
    }

    /// V10.3.2: reserve the repository record — the binding slot — before
    /// the first artifact arrives. Idempotent: a later reservation or
    /// import binds to the existing record and never clears it.
    pub(crate) fn reserve_repository(
        &mut self,
        repository_identity: GraphRepositoryIdentity,
    ) -> &mut ManagedRepositoryRecord {
        self.repositories
            .entry(repository_identity.clone())
            .or_insert_with(|| ManagedRepositoryRecord {
                repository_identity,
                objects: BTreeMap::new(),
            })
    }

    /// Import every Knowledge Object of a Graph Artifact into the
    /// artifact's repository record. Retains every object; a same-ID
    /// collision with another repository emits a [`ReconciliationCandidate`]
    /// and never merges, links, or re-homes anything.
    ///
    /// Fail closed on crafted input (`GraphArtifactDocument` derives
    /// `Deserialize`, so callers cannot assume compiler-built documents): a
    /// blank, grammar-violating, or repeated Object ID — or a
    /// `content_hash` outside the published v6 wire grammar — anywhere in
    /// the artifact rejects the whole import before any state changes.
    /// The graph loader enforces the same ID invariants (`id.invalid`,
    /// `DiagnosticCode::IdDuplicateInArtifact`); for `content_hash` it is
    /// deliberately more lenient (see
    /// `content_hash_matches_published_grammar`). A document handed to
    /// import directly must not bypass this boundary — a
    /// grammar-evading ID would also evade the exact-ID collision
    /// detector, a repeated ID would silently unify into one record (the
    /// intra-artifact shape of the merge RT-03/D36 forbids), and a
    /// malformed hash would be enshrined in an immutable version and
    /// compared for RT-04 unchanged-ness ever after.
    ///
    /// Two deliberate boundaries: (1) unlike `GraphIndex::from_document`,
    /// which degrades per node into diagnostics, one bad node rejects the
    /// whole document — at this trust boundary a partial import would be
    /// a silent drop; (2) the artifact's self-declared
    /// `repository_identity` is taken as the record key, which
    /// distinguishes repositories only as far as the caller's identities
    /// do ([`ManagedRepositoryRecord`]) — E1.3 adds a caller-supplied
    /// repository key so the record is bound from the authenticated
    /// channel rather than the payload; until then the artifact's
    /// declared identity *is* the key.
    ///
    /// Two trust assumptions stay with the producer, deliberately —
    /// the fail-closed list above is NOT exhaustive verification:
    /// (1) schema admission — the exact-match v6 loader
    /// (`infrastructure/artifact/graph_json.rs`) is the only
    /// deserialization path for external artifact JSON, and import never
    /// re-checks `schema_version`, so a document deserialized around
    /// that boundary could enshrine placement-bearing v5 hashes as
    /// governed-meaning version keys; (2) the declared `content_hash`
    /// is trusted as the authority on governed meaning —
    /// grammar-checked, never re-derived from node content (the
    /// crate-wide posture; `domain/review/object_diff.rs` makes the
    /// same assumption for change detection) — so unchanged-ness under
    /// RT-04 is exactly as trustworthy as the producer's hash.
    pub(crate) fn import_artifact(
        &mut self,
        artifact: &GraphArtifactDocument,
    ) -> Result<ManagedImportOutcome, ManagedIdentityError> {
        let mut seen_ids = BTreeSet::new();
        for node in artifact
            .nodes
            .iter()
            .filter_map(GraphNode::as_knowledge_object)
        {
            // Blank is a strict subset of the grammar failure below —
            // checked first only for the more precise error.
            if node.id.trim().is_empty() {
                return Err(ManagedIdentityError::BlankObjectId);
            }
            if ObjectId::new(node.id.as_str()).is_err() {
                return Err(ManagedIdentityError::InvalidObjectId);
            }
            if !seen_ids.insert(node.id.as_str()) {
                return Err(ManagedIdentityError::DuplicateObjectId);
            }
            if !content_hash_matches_published_grammar(&node.content_hash) {
                return Err(ManagedIdentityError::InvalidContentHash);
            }
        }
        // Arrival binds the slot even when the artifact carries no
        // Knowledge Objects (E1.2.T4).
        self.reserve_repository(artifact.repository_identity.clone());
        let mut imported = Vec::new();
        let mut reconciliation_candidates = Vec::new();
        for node in artifact
            .nodes
            .iter()
            .filter_map(GraphNode::as_knowledge_object)
        {
            let known = self
                .repositories
                .get(&artifact.repository_identity)
                .and_then(|record| record.objects.get(&node.id))
                .map(|object| {
                    let unchanged = object
                        .versions
                        .last()
                        .is_some_and(|latest| latest.content_hash == node.content_hash);
                    (object.canonical.clone(), unchanged)
                });
            let (canonical, colliding_parties) = match known {
                // RT-04: unchanged governed meaning re-observed — no new
                // content version, nothing minted.
                Some((_, true)) => continue,
                Some((canonical, false)) => (canonical, Vec::new()),
                // First appearance of this Object ID in this repository:
                // record same-ID parties from every other repository, then
                // mint a fresh canonical identity — a colliding object's
                // identity is never adopted.
                None => {
                    let parties = self.colliding_parties(&artifact.repository_identity, &node.id);
                    (self.mint_canonical_identity(), parties)
                }
            };
            let version_id = self.mint_version_id();
            // Reserved before the loop; re-reserving binds to the existing
            // record — this is the single construction path.
            let record = self.reserve_repository(artifact.repository_identity.clone());
            let object =
                record
                    .objects
                    .entry(node.id.clone())
                    .or_insert_with(|| ManagedObjectRecord {
                        canonical: canonical.clone(),
                        object_id: node.id.clone(),
                        versions: Vec::new(),
                    });
            object.versions.push(ManagedVersionRecord {
                version_id: version_id.clone(),
                content_hash: node.content_hash.clone(),
                source_binding: node.source_binding.clone(),
                source_assertions: Vec::new(),
            });
            for existing in colliding_parties {
                reconciliation_candidates.push(ReconciliationCandidate {
                    object_id: node.id.clone(),
                    reason: ReconciliationReason::ObjectIdCollision,
                    existing,
                    incoming: ReconciliationParty {
                        canonical: canonical.clone(),
                        repository_identity: artifact.repository_identity.clone(),
                        version_id: version_id.clone(),
                        content_hash: node.content_hash.clone(),
                    },
                });
            }
            imported.push(ImportedVersion {
                object_id: node.id.clone(),
                canonical,
                version_id,
            });
        }
        Ok(ManagedImportOutcome {
            imported,
            reconciliation_candidates,
        })
    }

    /// Same-ID parties in every other repository of this workspace. The
    /// scan is exact-ID only: no hash, title, or similarity matching
    /// exists (RT-03/D36).
    fn colliding_parties(
        &self,
        repository_identity: &GraphRepositoryIdentity,
        object_id: &str,
    ) -> Vec<ReconciliationParty> {
        self.repositories
            .iter()
            .filter(|(key, _)| *key != repository_identity)
            .filter_map(|(key, record)| {
                let object = record.objects.get(object_id)?;
                // Non-empty by construction: records only ever gain
                // versions, starting with one at creation.
                let latest = object.versions.last()?;
                Some(ReconciliationParty {
                    canonical: object.canonical.clone(),
                    repository_identity: key.clone(),
                    version_id: latest.version_id.clone(),
                    content_hash: latest.content_hash.clone(),
                })
            })
            .collect()
    }

    fn mint_canonical_identity(&mut self) -> WorkspaceCanonicalIdentity {
        self.minted_canonical_ids += 1;
        WorkspaceCanonicalIdentity {
            workspace_id: self.workspace_id.clone(),
            canonical_id: format!("mo-{}", self.minted_canonical_ids),
        }
    }

    fn mint_version_id(&mut self) -> ManagedVersionId {
        self.minted_version_ids += 1;
        ManagedVersionId(format!("mv-{}", self.minted_version_ids))
    }
}

#[cfg(test)]
mod tests {
    //! E1.2.T1 exit tests (MILESTONES §E1.2; RED-TEAM-CLOSURE §RT-03;
    //! DECISION-REGISTER §D36; ADR-0057 invariant 1): importing two Graph
    //! Artifacts that carry the same Object ID retains both objects
    //! distinct and emits a typed reconciliation candidate — never a merge.
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;
    use crate::domain::graph::{
        GraphKnowledgeObjectNode, GraphNode, GraphRelations, GraphSourceBinding, GraphSourceSpan,
    };

    /// Registered (planned) contract id governing the serialized candidate
    /// record shape pinned below.
    const RECONCILIATION_CANDIDATE_CONTRACT: &str = "adoc.reconciliation_candidate.v0";

    fn workspace() -> ManagedWorkspace {
        ManagedWorkspace::new(WorkspaceId::new("ws-acme").expect("workspace id is non-blank"))
    }

    fn local_repo(config_path: &str) -> GraphRepositoryIdentity {
        GraphRepositoryIdentity::local_project(config_path.to_string())
    }

    fn knowledge_object(id: &str, content_hash: &str) -> GraphKnowledgeObjectNode {
        GraphKnowledgeObjectNode {
            id: id.to_string(),
            kind: "claim".to_string(),
            content_hash: content_hash.to_string(),
            status: None,
            severity: None,
            trust: None,
            body: "Credits apply after payment.".to_string(),
            page_id: "team.page".to_string(),
            source_span: GraphSourceSpan {
                path: "docs/team.adoc".to_string(),
                line: 1,
                column: 1,
            },
            source_binding: None,
            visibility: None,
            field_visibility: None,
            fields: BTreeMap::new(),
            relations: GraphRelations::default(),
            impacts: Vec::new(),
            approved_by: Vec::new(),
            allowed_actions: Vec::new(),
            forbidden_actions: Vec::new(),
            contradiction_claims: Vec::new(),
            evidence: Vec::new(),
            effective_status: None,
            effective_reason: None,
            evidence_quality: None,
        }
    }

    fn artifact(
        repository_identity: GraphRepositoryIdentity,
        nodes: Vec<GraphKnowledgeObjectNode>,
    ) -> GraphArtifactDocument {
        GraphArtifactDocument {
            schema_version: "adoc.graph.v6".to_string(),
            repository_identity,
            nodes: nodes.into_iter().map(GraphNode::KnowledgeObject).collect(),
            edges: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    /// Exit test: same Object ID in two repositories — both retained
    /// distinct under fresh workspace canonical identities, one typed
    /// reconciliation candidate emitted, never a merge (RT-03, D36).
    #[test]
    fn same_object_id_in_two_repositories_retains_both_and_emits_candidate() {
        let mut workspace = workspace();
        let repo_a = local_repo("a/agentdoc.config.yaml");
        let repo_b = local_repo("b/agentdoc.config.yaml");

        let first = workspace
            .import_artifact(&artifact(
                repo_a.clone(),
                vec![knowledge_object("billing.credits", "sha256:aaa")],
            ))
            .expect("import accepted");
        assert!(first.reconciliation_candidates.is_empty());

        let second = workspace
            .import_artifact(&artifact(
                repo_b.clone(),
                vec![knowledge_object("billing.credits", "sha256:bbb")],
            ))
            .expect("import accepted");

        let object_a = &workspace
            .repository(&repo_a)
            .expect("repo a recorded")
            .objects["billing.credits"];
        let object_b = &workspace
            .repository(&repo_b)
            .expect("repo b recorded")
            .objects["billing.credits"];
        assert_eq!(object_a.object_id, "billing.credits");
        assert_eq!(object_b.object_id, "billing.credits");
        assert_ne!(
            object_a.canonical, object_b.canonical,
            "colliding Object IDs must never share a canonical identity"
        );

        assert_eq!(second.reconciliation_candidates.len(), 1);
        let candidate = &second.reconciliation_candidates[0];
        assert_eq!(candidate.object_id, "billing.credits");
        assert_eq!(candidate.reason, ReconciliationReason::ObjectIdCollision);
        assert_eq!(candidate.existing.canonical, object_a.canonical);
        assert_eq!(candidate.existing.repository_identity, repo_a);
        assert_eq!(candidate.incoming.canonical, object_b.canonical);
        assert_eq!(candidate.incoming.repository_identity, repo_b);

        // Single-shot: the collision was announced on the ID's first
        // appearance in repo b — a later revision of the still-colliding
        // object mints a version but never re-announces the candidate
        // (the caller persists it until decided, E1.2.T3/E1.3).
        let third = workspace
            .import_artifact(&artifact(
                repo_b.clone(),
                vec![knowledge_object("billing.credits", "sha256:ccc")],
            ))
            .expect("import accepted");
        assert_eq!(third.imported.len(), 1);
        assert!(
            third.reconciliation_candidates.is_empty(),
            "a candidate is emitted exactly once, not per revision"
        );
        let object_b = &workspace
            .repository(&repo_b)
            .expect("repo b recorded")
            .objects["billing.credits"];
        assert_eq!(object_b.versions.len(), 2);
    }

    /// The serialized record the Cloud cut consumes under the planned
    /// `adoc.reconciliation_candidate.v0` registry row. Covers both
    /// repository identity spellings: `{kind, config_path}` and explicit
    /// `null` (standalone).
    #[test]
    fn reconciliation_candidate_serialized_record_shape_is_pinned() {
        let mut workspace = workspace();
        workspace
            .import_artifact(&artifact(
                local_repo("a/agentdoc.config.yaml"),
                vec![knowledge_object("billing.credits", "sha256:aaa")],
            ))
            .expect("import accepted");
        let outcome = workspace
            .import_artifact(&artifact(
                GraphRepositoryIdentity::standalone(),
                vec![knowledge_object("billing.credits", "sha256:bbb")],
            ))
            .expect("import accepted");

        let candidate = &outcome.reconciliation_candidates[0];
        let value = serde_json::to_value(candidate).expect("candidate serializes");
        assert_eq!(
            value,
            json!({
                "object_id": "billing.credits",
                "reason": "object_id_collision",
                "existing": {
                    "canonical": {
                        "workspace_id": "ws-acme",
                        "canonical_id": "mo-1"
                    },
                    "repository_identity": {
                        "kind": "local_project",
                        "config_path": "a/agentdoc.config.yaml"
                    },
                    "version_id": "mv-1",
                    "content_hash": "sha256:aaa"
                },
                "incoming": {
                    "canonical": {
                        "workspace_id": "ws-acme",
                        "canonical_id": "mo-2"
                    },
                    "repository_identity": null,
                    "version_id": "mv-2",
                    "content_hash": "sha256:bbb"
                }
            }),
            "the {RECONCILIATION_CANDIDATE_CONTRACT} record shape drifted"
        );
    }

    /// Acceptance: every managed candidate version receives a unique
    /// immutable version ID while the Object ID and workspace canonical
    /// identity stay stable across revisions.
    #[test]
    fn revisions_mint_unique_version_ids_under_a_stable_object_identity() {
        let mut workspace = workspace();
        let repo = local_repo("a/agentdoc.config.yaml");

        let first = workspace
            .import_artifact(&artifact(
                repo.clone(),
                vec![knowledge_object("billing.credits", "sha256:aaa")],
            ))
            .expect("import accepted");
        let second = workspace
            .import_artifact(&artifact(
                repo.clone(),
                vec![knowledge_object("billing.credits", "sha256:ccc")],
            ))
            .expect("import accepted");

        let object =
            &workspace.repository(&repo).expect("repo recorded").objects["billing.credits"];
        assert_eq!(object.versions.len(), 2);
        assert_ne!(object.versions[0].version_id, object.versions[1].version_id);
        assert_eq!(first.imported[0].canonical, second.imported[0].canonical);
        assert_eq!(second.imported[0].object_id, "billing.credits");
        assert!(second.reconciliation_candidates.is_empty());
    }

    /// Pins the binding-retention contract: a same-hash re-observation
    /// carrying a *changed* Source Binding mints nothing and keeps the
    /// original binding — versions are immutable and placement is not
    /// content (RT-04, ADR-0058). Re-observation freshness is E4.1's
    /// concern, and the Cloud cut copies this behavior.
    #[test]
    fn reimporting_unchanged_content_with_a_new_binding_keeps_the_original() {
        let mut workspace = workspace();
        let repo = local_repo("a/agentdoc.config.yaml");
        let binding = |path: &str, digest: &str| GraphSourceBinding {
            connector: "local_fs".to_string(),
            source: "team.page".to_string(),
            revision: None,
            path: path.to_string(),
            anchor: "billing.credits".to_string(),
            source_revision_digest: digest.to_string(),
        };
        let mut node = knowledge_object("billing.credits", "sha256:aaa");
        node.source_binding = Some(binding("docs/team.adoc", "sha256:feed"));
        let original = node.source_binding.clone();
        workspace
            .import_artifact(&artifact(repo.clone(), vec![node]))
            .expect("import accepted");

        let mut moved = knowledge_object("billing.credits", "sha256:aaa");
        moved.source_binding = Some(binding("docs/moved.adoc", "sha256:beef"));
        let again = workspace
            .import_artifact(&artifact(repo.clone(), vec![moved]))
            .expect("import accepted");

        assert!(again.imported.is_empty());
        let object =
            &workspace.repository(&repo).expect("repo recorded").objects["billing.credits"];
        assert_eq!(object.versions.len(), 1);
        assert_eq!(
            object.versions[0].source_binding, original,
            "an immutable version keeps the binding observed when it was minted"
        );
    }

    /// RT-04: re-observing unchanged governed meaning is not a new content
    /// version — nothing is minted.
    #[test]
    fn reimporting_unchanged_content_mints_no_new_version() {
        let mut workspace = workspace();
        let repo = local_repo("a/agentdoc.config.yaml");
        let document = artifact(
            repo.clone(),
            vec![knowledge_object("billing.credits", "sha256:aaa")],
        );

        workspace
            .import_artifact(&document)
            .expect("import accepted");
        let again = workspace
            .import_artifact(&document)
            .expect("import accepted");

        assert!(again.imported.is_empty());
        assert!(again.reconciliation_candidates.is_empty());
        let object =
            &workspace.repository(&repo).expect("repo recorded").objects["billing.credits"];
        assert_eq!(object.versions.len(), 1);
    }

    /// Stop-ship guard (MILESTONES §E1.2 acceptance): identity is
    /// workspace-qualified — equal opaque suffixes in two workspaces never
    /// compare equal, so an unqualified Object ID cannot link identities
    /// across workspaces.
    #[test]
    fn canonical_identity_is_workspace_qualified() {
        let mut workspace_a = ManagedWorkspace::new(WorkspaceId::new("ws-a").expect("non-blank"));
        let mut workspace_b = ManagedWorkspace::new(WorkspaceId::new("ws-b").expect("non-blank"));
        let repo = local_repo("a/agentdoc.config.yaml");
        let document = artifact(
            repo.clone(),
            vec![knowledge_object("billing.credits", "sha256:aaa")],
        );

        let in_a = workspace_a
            .import_artifact(&document)
            .expect("import accepted");
        let in_b = workspace_b
            .import_artifact(&document)
            .expect("import accepted");

        assert_eq!(in_a.imported[0].canonical.canonical_id, "mo-1");
        assert_eq!(in_b.imported[0].canonical.canonical_id, "mo-1");
        assert_ne!(
            in_a.imported[0].canonical, in_b.imported[0].canonical,
            "workspace qualification must separate identical opaque suffixes"
        );
    }

    /// A blank workspace id would produce unqualified canonical identities,
    /// and a padded one would make `"ws-acme"` and `" ws-acme"` two
    /// workspaces whose identities never compare equal — with the
    /// whitespace shipping on the wire through the transparent payload.
    /// Construction fails closed on both; it never normalizes (silently
    /// unifying two spellings is a merge nobody decided).
    #[test]
    fn blank_or_padded_workspace_id_is_rejected() {
        assert!(WorkspaceId::new("").is_err());
        assert!(WorkspaceId::new(" \t").is_err());
        assert!(WorkspaceId::new(" ws-acme").is_err());
        assert!(WorkspaceId::new("ws-acme ").is_err());
    }

    /// A crafted artifact carrying a blank Object ID (empty or
    /// whitespace-only — `GraphArtifactDocument` derives `Deserialize`, so
    /// import callers cannot assume compiler-built input) is rejected
    /// before any state changes: no slot bound, nothing minted (fail
    /// closed, matching the Cloud store's non-empty check).
    #[test]
    fn blank_object_id_on_import_is_rejected_without_partial_state() {
        let mut workspace = workspace();
        let repo = local_repo("a/agentdoc.config.yaml");

        for blank in ["", "  "] {
            let outcome = workspace.import_artifact(&artifact(
                repo.clone(),
                vec![
                    knowledge_object("billing.credits", "sha256:aaa"),
                    knowledge_object(blank, "sha256:bbb"),
                ],
            ));
            assert_eq!(outcome, Err(ManagedIdentityError::BlankObjectId));
        }
        assert!(
            workspace.repository(&repo).is_none(),
            "a rejected import must not bind the repository slot or mint anything"
        );
    }

    /// A crafted artifact repeating an Object ID would otherwise mint two
    /// versions in one import (differing hashes) or silently drop the
    /// second occurrence (equal hashes) — the intra-artifact shape of the
    /// auto-merge RT-03/D36 forbids. The graph loader already rejects the
    /// document (`DiagnosticCode::IdDuplicateInArtifact`); import fails
    /// closed the same way, before any state changes.
    #[test]
    fn duplicate_object_ids_in_one_artifact_are_rejected_without_partial_state() {
        let mut workspace = workspace();
        let repo = local_repo("a/agentdoc.config.yaml");

        for hashes in [["sha256:aaa", "sha256:bbb"], ["sha256:5a3e", "sha256:5a3e"]] {
            let outcome = workspace.import_artifact(&artifact(
                repo.clone(),
                vec![
                    knowledge_object("billing.credits", hashes[0]),
                    knowledge_object("billing.credits", hashes[1]),
                ],
            ));
            assert_eq!(outcome, Err(ManagedIdentityError::DuplicateObjectId));
        }
        assert!(
            workspace.repository(&repo).is_none(),
            "a rejected import must not bind the repository slot or mint anything"
        );
    }

    /// Exact Object-ID equality is the only collision detector this module
    /// has, so an ID evading the grammar evades reconciliation too:
    /// `"billing.credits "` never collides with `"billing.credits"` yet
    /// reads identically. Import enforces the same `ObjectId` grammar as
    /// the graph loader (`id.invalid`) and fails closed.
    #[test]
    fn object_ids_violating_the_grammar_are_rejected_without_partial_state() {
        let mut workspace = workspace();
        let repo = local_repo("a/agentdoc.config.yaml");

        for invalid in [
            "billing.credits ",
            " billing.credits",
            "Billing.Credits",
            "billing",
            "billing.-credits",
        ] {
            let outcome = workspace.import_artifact(&artifact(
                repo.clone(),
                vec![knowledge_object(invalid, "sha256:aaa")],
            ));
            assert_eq!(
                outcome,
                Err(ManagedIdentityError::InvalidObjectId),
                "{invalid:?} must be rejected"
            );
        }
        assert!(
            workspace.repository(&repo).is_none(),
            "a rejected import must not bind the repository slot or mint anything"
        );
    }

    /// A crafted artifact carrying a blank or malformed `content_hash`
    /// would otherwise mint an immutable version around it, surface it in
    /// reconciliation candidates, and treat every later observation of the
    /// same malformed value as unchanged. Import enforces the published v6
    /// wire grammar `^sha256:[0-9a-f]+$` (graph-artifact.v6.json): non-hex,
    /// uppercase, and whitespace-padded suffixes all fail closed before any
    /// state changes — a padded spelling of an existing hash would
    /// otherwise mint a fresh version of unchanged content and ship the
    /// padded value in the candidate payload.
    #[test]
    fn malformed_content_hashes_on_import_are_rejected_without_partial_state() {
        let mut workspace = workspace();
        let repo = local_repo("a/agentdoc.config.yaml");

        for invalid in [
            "",
            "  ",
            "sha256:",
            "sha256:  ",
            "md5:abc",
            "sha256:not-a-digest",
            "sha256:abc ",
            " sha256:abc",
            "sha256:ABC",
        ] {
            let outcome = workspace.import_artifact(&artifact(
                repo.clone(),
                vec![knowledge_object("billing.credits", invalid)],
            ));
            assert_eq!(
                outcome,
                Err(ManagedIdentityError::InvalidContentHash),
                "{invalid:?} must be rejected"
            );
        }
        assert!(
            workspace.repository(&repo).is_none(),
            "a rejected import must not bind the repository slot or mint anything"
        );
    }

    /// The Source Assertion identity layer (K7) exists as its own type;
    /// blank and padded identities fail closed and the value serializes
    /// transparently.
    #[test]
    fn source_assertion_identity_is_typed_and_rejects_blank_values() {
        assert!(SourceAssertionIdentity::new("  ").is_err());
        assert!(SourceAssertionIdentity::new(" a:b").is_err());
        assert!(SourceAssertionIdentity::new("a:b ").is_err());
        let identity =
            SourceAssertionIdentity::new("confluence:page-9:rev-4:assertion-2").expect("non-blank");
        assert_eq!(
            serde_json::to_value(&identity).expect("serializes"),
            json!("confluence:page-9:rev-4:assertion-2")
        );
    }

    /// RT-03: imported versions preserve their exact Source Binding; the
    /// Source Assertion slot exists per version and stays empty until the
    /// Source Record/Assertion store lands (E4.1).
    #[test]
    fn imported_versions_preserve_the_source_binding() {
        let mut workspace = workspace();
        let repo = local_repo("a/agentdoc.config.yaml");
        let mut node = knowledge_object("billing.credits", "sha256:aaa");
        node.source_binding = Some(GraphSourceBinding {
            connector: "local_fs".to_string(),
            source: "team.page".to_string(),
            revision: None,
            path: "docs/team.adoc".to_string(),
            anchor: "billing.credits".to_string(),
            source_revision_digest: "sha256:feed".to_string(),
        });
        let expected = node.source_binding.clone();

        workspace
            .import_artifact(&artifact(repo.clone(), vec![node]))
            .expect("import accepted");

        let object =
            &workspace.repository(&repo).expect("repo recorded").objects["billing.credits"];
        assert_eq!(object.versions[0].source_binding, expected);
        assert!(object.versions[0].source_assertions.is_empty());
    }

    // E1.2.T2 negative fixtures (RT-03/D36): no auto-merge on ID, title,
    // hash, or similarity. No similarity machinery exists in this crate —
    // these fixtures prove the absence of any unification path: candidates
    // arise from exact Object-ID collision alone, and even those never
    // merge.

    /// Exit test: the same semantic `content_hash` under two different
    /// Object IDs stays two objects — no candidate, no merge. Two objects
    /// can hash identically at all because ADR-0058 §2 keeps identity out
    /// of the v6 hash payload (identity and semantic hash are separate K6
    /// layers).
    #[test]
    fn same_content_hash_under_two_object_ids_stays_two_objects() {
        let mut workspace = workspace();
        let repo = local_repo("a/agentdoc.config.yaml");

        let outcome = workspace
            .import_artifact(&artifact(
                repo.clone(),
                vec![
                    knowledge_object("billing.credits", "sha256:5a3e"),
                    knowledge_object("billing.refunds", "sha256:5a3e"),
                ],
            ))
            .expect("import accepted");

        assert!(outcome.reconciliation_candidates.is_empty());
        let objects = &workspace.repository(&repo).expect("repo recorded").objects;
        assert_eq!(objects.len(), 2);
        assert_ne!(
            objects["billing.credits"].canonical, objects["billing.refunds"].canonical,
            "equal hashes must never collapse two Object IDs into one identity"
        );
        assert_ne!(
            objects["billing.credits"].versions[0].version_id,
            objects["billing.refunds"].versions[0].version_id
        );
    }

    /// Equal hashes across repositories without an ID collision produce
    /// nothing: no candidate, no link, both objects retained.
    #[test]
    fn equal_hashes_across_repositories_without_id_collision_produce_nothing() {
        let mut workspace = workspace();
        let repo_a = local_repo("a/agentdoc.config.yaml");
        let repo_b = local_repo("b/agentdoc.config.yaml");

        workspace
            .import_artifact(&artifact(
                repo_a.clone(),
                vec![knowledge_object("billing.credits", "sha256:5a3e")],
            ))
            .expect("import accepted");
        let outcome = workspace
            .import_artifact(&artifact(
                repo_b.clone(),
                vec![knowledge_object("billing.refunds", "sha256:5a3e")],
            ))
            .expect("import accepted");

        assert!(outcome.reconciliation_candidates.is_empty());
        assert_eq!(
            workspace.repository(&repo_a).expect("repo a").objects.len(),
            1
        );
        assert_eq!(
            workspace.repository(&repo_b).expect("repo b").objects.len(),
            1
        );
    }

    /// Same title, same body, same hash — maximum surface similarity under
    /// two different Object IDs — never unifies and emits no candidate.
    #[test]
    fn identical_titles_and_bodies_never_unify() {
        let mut workspace = workspace();
        let repo = local_repo("a/agentdoc.config.yaml");
        let mut first = knowledge_object("billing.credits", "sha256:5a3e");
        first
            .fields
            .insert("title".to_string(), "Credit policy".to_string());
        let mut second = knowledge_object("billing.credit-rules", "sha256:5a3e");
        second
            .fields
            .insert("title".to_string(), "Credit policy".to_string());
        assert_eq!(first.body, second.body);

        let outcome = workspace
            .import_artifact(&artifact(repo.clone(), vec![first, second]))
            .expect("import accepted");

        assert!(outcome.reconciliation_candidates.is_empty());
        let objects = &workspace.repository(&repo).expect("repo recorded").objects;
        assert_eq!(objects.len(), 2);
        assert_ne!(
            objects["billing.credits"].canonical,
            objects["billing.credit-rules"].canonical
        );
    }

    /// Adversarial (MILESTONES §E1.2 acceptance): a crafted import
    /// replaying an existing object's exact Object ID AND `content_hash`
    /// from another repository — the most merge-tempting collision — is
    /// retained distinct with a candidate only; hash equality confers no
    /// unification.
    #[test]
    fn crafted_replay_of_id_and_hash_is_retained_distinct_candidate_only() {
        let mut workspace = workspace();
        let repo_a = local_repo("a/agentdoc.config.yaml");
        let repo_b = local_repo("b/agentdoc.config.yaml");

        workspace
            .import_artifact(&artifact(
                repo_a.clone(),
                vec![knowledge_object("billing.credits", "sha256:5a3e")],
            ))
            .expect("import accepted");
        let outcome = workspace
            .import_artifact(&artifact(
                repo_b.clone(),
                vec![knowledge_object("billing.credits", "sha256:5a3e")],
            ))
            .expect("import accepted");

        assert_eq!(outcome.reconciliation_candidates.len(), 1);
        let candidate = &outcome.reconciliation_candidates[0];
        assert_eq!(candidate.reason, ReconciliationReason::ObjectIdCollision);
        assert_eq!(
            candidate.existing.content_hash,
            candidate.incoming.content_hash
        );
        assert_ne!(
            candidate.existing.canonical, candidate.incoming.canonical,
            "an identical hash must not let the replay adopt the existing identity"
        );
        let object_a = &workspace.repository(&repo_a).expect("repo a").objects["billing.credits"];
        let object_b = &workspace.repository(&repo_b).expect("repo b").objects["billing.credits"];
        assert_ne!(object_a.canonical, object_b.canonical);
        assert_eq!(object_a.versions.len(), 1);
        assert_eq!(object_b.versions.len(), 1);
    }

    // E1.2.T4: the managed repository record keys on the graph
    // `repository_identity` ({kind, config_path} or explicit null,
    // ADR-0049); the binding slot is reserved before the first artifact
    // arrives (provenance V10.3.2); imported Object IDs stay
    // repository-local.

    /// V10.3.2: the binding slot — the keyed repository record — exists
    /// before the first artifact arrives.
    #[test]
    fn reserved_repository_slot_exists_before_the_first_artifact() {
        let mut workspace = workspace();
        let repo = local_repo("a/agentdoc.config.yaml");

        workspace.reserve_repository(repo.clone());

        let record = workspace.repository(&repo).expect("slot reserved");
        assert_eq!(record.repository_identity, repo);
        assert!(record.objects.is_empty());
    }

    /// The first import binds into the reserved record — reservation and
    /// import key on the same repository identity, and re-reserving never
    /// clears what an import recorded.
    #[test]
    fn import_binds_to_the_reserved_record_and_reserve_is_idempotent() {
        let mut workspace = workspace();
        let repo = local_repo("a/agentdoc.config.yaml");
        workspace.reserve_repository(repo.clone());
        workspace.reserve_repository(repo.clone());

        workspace
            .import_artifact(&artifact(
                repo.clone(),
                vec![knowledge_object("billing.credits", "sha256:aaa")],
            ))
            .expect("import accepted");
        workspace.reserve_repository(repo.clone());

        let record = workspace.repository(&repo).expect("repo recorded");
        assert!(record.objects.contains_key("billing.credits"));
    }

    /// Importing an artifact with no Knowledge Objects still records the
    /// repository: arrival binds the slot even when nothing is imported.
    #[test]
    fn import_without_knowledge_objects_still_records_the_repository() {
        let mut workspace = workspace();
        let repo = local_repo("a/agentdoc.config.yaml");

        let outcome = workspace
            .import_artifact(&artifact(repo.clone(), Vec::new()))
            .expect("import accepted");

        assert!(outcome.imported.is_empty());
        let record = workspace
            .repository(&repo)
            .expect("repo recorded on arrival");
        assert!(record.objects.is_empty());
    }

    /// Exit test: two imported repositories collide — `{kind, config_path}`
    /// and explicit `null` are distinct keys, each retains its own object
    /// under the shared Object ID, and the collision is a candidate, never
    /// a silent same-ID merge across repositories.
    #[test]
    fn object_ids_stay_repository_local_across_null_and_project_keys() {
        let mut workspace = workspace();
        let project = local_repo("a/agentdoc.config.yaml");
        let standalone = GraphRepositoryIdentity::standalone();

        workspace
            .import_artifact(&artifact(
                project.clone(),
                vec![knowledge_object("billing.credits", "sha256:aaa")],
            ))
            .expect("import accepted");
        let outcome = workspace
            .import_artifact(&artifact(
                standalone.clone(),
                vec![knowledge_object("billing.credits", "sha256:bbb")],
            ))
            .expect("import accepted");

        assert_eq!(outcome.reconciliation_candidates.len(), 1);
        let in_project = &workspace
            .repository(&project)
            .expect("project repo")
            .objects["billing.credits"];
        let in_standalone = &workspace
            .repository(&standalone)
            .expect("standalone repo")
            .objects["billing.credits"];
        assert_ne!(in_project.canonical, in_standalone.canonical);
        assert_eq!(in_project.versions[0].content_hash, "sha256:aaa");
        assert_eq!(in_standalone.versions[0].content_hash, "sha256:bbb");
    }

    /// Pins what the wire actually exhibits today: every project-bound
    /// artifact the compiler emits carries the constant
    /// `{local_project, "agentdoc.config.yaml"}` identity
    /// (`FsSourceProvider::repository_identity`, ADR-0049 §7), so two
    /// physical repositories importing under the producer identity are
    /// one record here — the shared Object ID appends as a revision and
    /// no candidate is emitted. Identity-equality keying distinguishes
    /// repositories only as far as the caller's identities do
    /// ([`ManagedRepositoryRecord`]); explicit routing to a reserved
    /// binding slot (V10.3.2) is E1.3 scope, and the Cloud cut keys
    /// uploads from its authenticated channel, never from the artifact.
    #[test]
    fn identical_producer_identities_collapse_into_one_repository_record() {
        let mut workspace = workspace();
        let producer = local_repo("agentdoc.config.yaml");

        workspace
            .import_artifact(&artifact(
                producer.clone(),
                vec![knowledge_object("billing.credits", "sha256:aaa")],
            ))
            .expect("import accepted");
        let second = workspace
            .import_artifact(&artifact(
                producer.clone(),
                vec![knowledge_object("billing.credits", "sha256:bbb")],
            ))
            .expect("import accepted");

        assert!(
            second.reconciliation_candidates.is_empty(),
            "equal keys are one repository: no cross-repository collision exists to announce"
        );
        let object = &workspace
            .repository(&producer)
            .expect("one record under the producer identity")
            .objects["billing.credits"];
        assert_eq!(
            object.versions.len(),
            2,
            "the second import appends a revision of the first repository's object"
        );
    }
}
