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
//! repositories retains every object distinct and emits a typed
//! [`ReconciliationCandidate`] (`adoc.reconciliation_candidate.v0`,
//! registered planned in CONTRACT-REGISTRY.md); deciding a candidate is a
//! governed E1.3 action, never an import side effect.
//!
//! Routing (E1.3, adjudication of PR #150 claim 5): repositories live in
//! reserved binding slots keyed by [`RepositorySlotId`] handle, because the
//! CLI's `repository_identity` is degenerate across physical repositories
//! (see [`ManagedRepositoryRecord`] for what the graph identity does and
//! does not distinguish). Import routes explicitly by slot handle;
//! identity-equality lookup remains only as the single-repo convenience
//! and fails closed when ambiguous.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::graph::{GraphArtifactDocument, GraphNode, GraphRepositoryIdentity, GraphSourceBinding};
use super::identity::ObjectId;
use super::reconciliation::{
    DecisionParty, ReconciliationDecision, ReconciliationDecisionError, ReconciliationState,
    ReconciliationVerb,
};

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
    #[error("repository identity matches more than one reserved slot; route by slot handle")]
    AmbiguousRepositoryIdentity,
    #[error("repository slot handle is not reserved in this workspace")]
    UnknownRepositorySlot,
    #[error("artifact repository identity differs from the reserved slot's")]
    RepositoryIdentityMismatch,
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

/// The workspace-minted handle of one reserved binding slot (V10.3.2).
/// The slot handle — not the graph `repository_identity` — is what managed
/// import routes by: CLI builds emit a degenerate identity (every
/// project-bound build carries `{local_project, "agentdoc.config.yaml"}`),
/// so identity equality cannot distinguish two physical repositories in
/// one workspace (E1.3 adjudication of PR #150 claim 5). Opaque and never
/// reused, like the other minted ids.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct RepositorySlotId(String);

/// One managed repository inside a workspace — the reserved binding slot
/// (V10.3.2), keyed by its [`RepositorySlotId`] handle. It records the
/// graph `repository_identity` (`{kind, config_path}` or explicit `null` —
/// required since v5, ADR-0049) it was reserved for, can exist before the
/// first artifact arrives, and its Object IDs stay repository-local — the
/// same ID in another slot is a different Managed Object.
///
/// The graph identity names the producing invocation family, not a
/// workspace-unique repository: every project-bound build emits the
/// constant `{local_project, "agentdoc.config.yaml"}`
/// (`FsSourceProvider::repository_identity`, ADR-0049 §7), so
/// identity-equality keying distinguishes repositories only as far as the
/// caller's identities do — that is why the slot handle, not the
/// identity, is the key, and identity lookup is the single-repository
/// convenience only.
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
    repositories: BTreeMap<RepositorySlotId, ManagedRepositoryRecord>,
    // ponytail: workspace-local monotonic minting; all ids are opaque to
    // every consumer — Cloud storage mints its own scheme (the contract is
    // opacity and uniqueness, not the format).
    minted_canonical_ids: u64,
    minted_version_ids: u64,
    minted_slot_ids: u64,
    /// Append-only recorded reconciliation decision Governance Events
    /// (E1.3): the sole mutation path for reconciliation state.
    decisions: Vec<ReconciliationDecision>,
}

impl ManagedWorkspace {
    pub(crate) fn new(workspace_id: WorkspaceId) -> Self {
        Self {
            workspace_id,
            repositories: BTreeMap::new(),
            minted_canonical_ids: 0,
            minted_version_ids: 0,
            minted_slot_ids: 0,
            decisions: Vec::new(),
        }
    }

    /// The Managed Object a workspace canonical identity names, whichever
    /// slot holds it. Provenance stays queryable through every decision —
    /// no verb removes, rewrites, or re-homes a record (RT-03).
    pub(crate) fn managed_object(
        &self,
        canonical: &WorkspaceCanonicalIdentity,
    ) -> Option<&ManagedObjectRecord> {
        self.repositories.values().find_map(|record| {
            record
                .objects
                .values()
                .find(|object| &object.canonical == canonical)
        })
    }

    /// Record one reconciliation decision Governance Event. Fail closed:
    /// both parties must exist in this workspace, be bound to their exact
    /// latest managed version, and form a Reconciliation Candidate pair:
    /// candidates arise from exact Object-ID collision alone (RT-03), so
    /// two objects that never collided are not a pair to adjudicate. The
    /// latest-version binding is a RECORD-TIME admission gate — a
    /// decision adjudicated against content that had already changed
    /// never enters the log silently. It is not a durable property of a
    /// standing decision: a later import minting a new version for
    /// either party leaves the decision fully standing, and post-decision
    /// content change disturbs nothing — re-review/invalidation is out of
    /// scope for this slice (connector re-observation territory, E8.3).
    /// Recording appends to the log and touches no object or version
    /// record: RT-03 preservation holds by construction.
    pub(crate) fn record_decision(
        &mut self,
        decision: ReconciliationDecision,
    ) -> Result<(), ReconciliationDecisionError> {
        let subject = self.bound_party(decision.subject())?;
        let counterpart = self.bound_party(decision.counterpart())?;
        if subject.object_id != counterpart.object_id {
            return Err(ReconciliationDecisionError::NotACandidatePair);
        }
        // A standing merge freezes its parties against conflicting
        // decisions: a merged-away counterpart cannot be decided again
        // under another pair (it would be re-homed into two survivors at
        // once), and a surviving subject cannot itself be merged away (the
        // chain would silently orphan its antecedents from the new
        // survivor's single-level walk). Nested reconciliation policy is
        // post-V1 (MILESTONES §E1.3 out of scope) — out-of-scope input is
        // rejected, never half-modelled. Same-pair decisions stay exempt:
        // the last decision for a pair stands.
        // ponytail: full standing-state derive per record; index standing
        // merges by party if the log grows hot.
        let pair = decision.pair_key();
        let state = self.reconciliation_state();
        for standing in &state.standing {
            if standing.verb() != ReconciliationVerb::MergeRehome || standing.pair_key() == pair {
                continue;
            }
            let merged_away = &standing.counterpart().canonical;
            if merged_away == &decision.subject().canonical
                || merged_away == &decision.counterpart().canonical
            {
                return Err(ReconciliationDecisionError::PartyAlreadyMerged);
            }
            if decision.verb() == ReconciliationVerb::MergeRehome
                && standing.subject().canonical == decision.counterpart().canonical
            {
                return Err(ReconciliationDecisionError::NestedMergeUnsupported);
            }
        }
        self.decisions.push(decision);
        Ok(())
    }

    /// Resolve one decision party fail-closed: the object must exist in
    /// this workspace and the binding must be its exact latest version.
    fn bound_party(
        &self,
        bound: &DecisionParty,
    ) -> Result<&ManagedObjectRecord, ReconciliationDecisionError> {
        let object = self
            .managed_object(&bound.canonical)
            .ok_or(ReconciliationDecisionError::UnknownParty)?;
        let latest = object
            .versions
            .last()
            // Unreachable: records only ever gain versions, starting
            // with one at creation; kept typed, never unwrapped.
            .ok_or(ReconciliationDecisionError::UnknownParty)?;
        if latest.version_id != bound.version_id {
            return Err(ReconciliationDecisionError::StaleVersionBinding);
        }
        Ok(object)
    }

    /// The recorded decision history, in order — the replay input.
    pub(crate) fn decisions(&self) -> &[ReconciliationDecision] {
        &self.decisions
    }

    /// Replay one decision from a trusted recorded log. The admission
    /// checks ([`ManagedWorkspace::record_decision`]: candidate pair,
    /// latest-version binding, standing-merge conflicts) ran when the
    /// decision entered the log; they are record-time gates, not
    /// log-validity invariants — a decision recorded between two imports
    /// must replay after those imports even though its binding is no
    /// longer the latest. Party EXISTENCE is the exception: RT-03
    /// promises every decision party stays queryable through the
    /// decision, a property of the log *plus* the import history it
    /// replays over — so replaying a valid log against a mismatched
    /// history (truncated rebuild, decisions restored ahead of their
    /// imports, the wrong workspace) fails closed with
    /// [`ReconciliationDecisionError::UnknownParty`] and appends
    /// nothing, instead of deriving standing state whose canonicals
    /// resolve to no managed object. Never feed unvalidated input here:
    /// new decisions go through [`ManagedWorkspace::record_decision`].
    pub(crate) fn replay_decision(
        &mut self,
        decision: ReconciliationDecision,
    ) -> Result<(), ReconciliationDecisionError> {
        for bound in [decision.subject(), decision.counterpart()] {
            if self.managed_object(&bound.canonical).is_none() {
                return Err(ReconciliationDecisionError::UnknownParty);
            }
        }
        self.decisions.push(decision);
        Ok(())
    }

    /// The derived reconciliation state: deterministic, wall-clock-free —
    /// replaying [`ManagedWorkspace::decisions`] over the same import
    /// history yields byte-identical state.
    pub(crate) fn reconciliation_state(&self) -> ReconciliationState {
        ReconciliationState::derive(&self.decisions)
    }

    /// The canonical identities merged into `survivor` by standing
    /// merge/re-home decisions (E1.3.T3). Directional — the survivor is
    /// the decision's subject — and derived from the decision log alone:
    /// no import, hash, or similarity ever creates a merged relation.
    /// Each antecedent's full provenance stays queryable via
    /// [`ManagedWorkspace::managed_object`] (RT-03).
    pub(crate) fn merged_antecedents(
        &self,
        survivor: &WorkspaceCanonicalIdentity,
    ) -> Vec<WorkspaceCanonicalIdentity> {
        self.reconciliation_state()
            .standing
            .iter()
            .filter(|decision| decision.verb() == ReconciliationVerb::MergeRehome)
            .filter(|decision| &decision.subject().canonical == survivor)
            .map(|decision| decision.counterpart().canonical.clone())
            .collect()
    }

    /// Single-repo convenience: the record whose recorded identity equals
    /// `repository_identity`. Absence (`Ok(None)`) and ambiguity are
    /// distinct outcomes: once two slots share a (degenerate) identity the
    /// lookup fails closed with
    /// [`ManagedIdentityError::AmbiguousRepositoryIdentity`] instead of
    /// answering `None` — "not imported yet" and "imported twice, route by
    /// slot handle" must never read the same.
    /// [`ManagedWorkspace::repository_slot`] is the authoritative accessor.
    pub(crate) fn repository(
        &self,
        repository_identity: &GraphRepositoryIdentity,
    ) -> Result<Option<&ManagedRepositoryRecord>, ManagedIdentityError> {
        let mut matches = self
            .repositories
            .values()
            .filter(|record| &record.repository_identity == repository_identity);
        match (matches.next(), matches.next()) {
            (Some(record), None) => Ok(Some(record)),
            (None, _) => Ok(None),
            (Some(_), Some(_)) => Err(ManagedIdentityError::AmbiguousRepositoryIdentity),
        }
    }

    pub(crate) fn repository_slot(
        &self,
        slot: &RepositorySlotId,
    ) -> Option<&ManagedRepositoryRecord> {
        self.repositories.get(slot)
    }

    /// V10.3.2 single-repo convenience: reserve the binding slot for this
    /// identity before the first artifact arrives. Idempotent by identity —
    /// a later reservation or import binds to the existing slot and never
    /// clears it. Once two slots share the identity the lookup is
    /// ambiguous and fails closed: reserve each physical repository
    /// explicitly via [`ManagedWorkspace::reserve_repository_slot`].
    pub(crate) fn reserve_repository(
        &mut self,
        repository_identity: GraphRepositoryIdentity,
    ) -> Result<RepositorySlotId, ManagedIdentityError> {
        let mut matches = self
            .repositories
            .iter()
            .filter(|(_, record)| record.repository_identity == repository_identity)
            .map(|(slot, _)| slot.clone());
        match (matches.next(), matches.next()) {
            (Some(slot), None) => Ok(slot),
            (Some(_), Some(_)) => Err(ManagedIdentityError::AmbiguousRepositoryIdentity),
            (None, _) => Ok(self.reserve_repository_slot(repository_identity)),
        }
    }

    /// E1.3 (adjudication of PR #150 claim 5): explicitly reserve a fresh
    /// binding slot for one physical repository. Always mints — this is
    /// how two physical repositories with the same degenerate CLI identity
    /// become two slots with distinct object registries.
    pub(crate) fn reserve_repository_slot(
        &mut self,
        repository_identity: GraphRepositoryIdentity,
    ) -> RepositorySlotId {
        self.minted_slot_ids += 1;
        let slot = RepositorySlotId(format!("slot-{}", self.minted_slot_ids));
        self.repositories.insert(
            slot.clone(),
            ManagedRepositoryRecord {
                repository_identity,
                objects: BTreeMap::new(),
            },
        );
        slot
    }

    /// Single-repo convenience: import into the slot whose recorded
    /// identity equals the artifact's, reserving it on first arrival.
    /// Fails closed with [`ManagedIdentityError::AmbiguousRepositoryIdentity`]
    /// once two slots share the identity — degenerate CLI identities make
    /// identity lookup unable to distinguish physical repositories, so
    /// multi-repo workspaces route explicitly via
    /// [`ManagedWorkspace::import_artifact_into`] (E1.3 adjudication of
    /// PR #150 claim 5).
    pub(crate) fn import_artifact(
        &mut self,
        artifact: &GraphArtifactDocument,
    ) -> Result<ManagedImportOutcome, ManagedIdentityError> {
        Self::validate_artifact_object_ids(artifact)?;
        // Arrival binds the slot even when the artifact carries no
        // Knowledge Objects (E1.2.T4) — but only after validation, so a
        // rejected import binds nothing.
        let slot = self.reserve_repository(artifact.repository_identity.clone())?;
        self.import_validated(&slot, artifact)
    }

    /// E1.3 explicit routing: import into a previously reserved slot,
    /// named by handle. The artifact's own `repository_identity` never
    /// routes — but routing an artifact whose identity differs from the
    /// slot's recorded one is rejected fail-closed rather than silently
    /// re-labeled.
    pub(crate) fn import_artifact_into(
        &mut self,
        slot: &RepositorySlotId,
        artifact: &GraphArtifactDocument,
    ) -> Result<ManagedImportOutcome, ManagedIdentityError> {
        let record = self
            .repositories
            .get(slot)
            .ok_or(ManagedIdentityError::UnknownRepositorySlot)?;
        if record.repository_identity != artifact.repository_identity {
            return Err(ManagedIdentityError::RepositoryIdentityMismatch);
        }
        Self::validate_artifact_object_ids(artifact)?;
        self.import_validated(slot, artifact)
    }

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
    /// Deliberate boundary: unlike `GraphIndex::from_document`,
    /// which degrades per node into diagnostics, one bad node rejects the
    /// whole document — at this trust boundary a partial import would be a
    /// silent drop.
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
    fn validate_artifact_object_ids(
        artifact: &GraphArtifactDocument,
    ) -> Result<(), ManagedIdentityError> {
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
        Ok(())
    }

    /// Import every Knowledge Object of a validated Graph Artifact into
    /// the reserved slot. Retains every object; a same-ID collision with
    /// another slot emits a [`ReconciliationCandidate`] and never merges,
    /// links, or re-homes anything.
    fn import_validated(
        &mut self,
        slot: &RepositorySlotId,
        artifact: &GraphArtifactDocument,
    ) -> Result<ManagedImportOutcome, ManagedIdentityError> {
        let mut imported = Vec::new();
        let mut reconciliation_candidates = Vec::new();
        for node in artifact
            .nodes
            .iter()
            .filter_map(GraphNode::as_knowledge_object)
        {
            let known = self
                .repositories
                .get(slot)
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
                // First appearance of this Object ID in this slot: record
                // same-ID parties from every other slot, then mint a fresh
                // canonical identity — a colliding object's identity is
                // never adopted.
                None => {
                    let parties = self.colliding_parties(slot, &node.id);
                    (self.mint_canonical_identity(), parties)
                }
            };
            let version_id = self.mint_version_id();
            let record = self
                .repositories
                .get_mut(slot)
                // Unreachable: both entry points resolve or reserve the
                // slot before delegating here; kept typed, never unwrapped.
                .ok_or(ManagedIdentityError::UnknownRepositorySlot)?;
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

    /// Same-ID parties in every other slot of this workspace — two slots
    /// sharing a degenerate identity still collide with each other. The
    /// scan is exact-ID only: no hash, title, or similarity matching
    /// exists (RT-03/D36).
    fn colliding_parties(
        &self,
        slot: &RepositorySlotId,
        object_id: &str,
    ) -> Vec<ReconciliationParty> {
        self.repositories
            .iter()
            .filter(|(key, _)| *key != slot)
            .filter_map(|(_, record)| {
                let object = record.objects.get(object_id)?;
                // Non-empty by construction: records only ever gain
                // versions, starting with one at creation.
                let latest = object.versions.last()?;
                Some(ReconciliationParty {
                    canonical: object.canonical.clone(),
                    repository_identity: record.repository_identity.clone(),
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
    use crate::domain::reconciliation::{
        DecisionParty, PolicyVersion, Principal, ReconciliationDecision, ReconciliationVerb,
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
            .expect("identity unambiguous")
            .expect("repo a recorded")
            .objects["billing.credits"];
        let object_b = &workspace
            .repository(&repo_b)
            .expect("identity unambiguous")
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

        let object = &workspace
            .repository(&repo)
            .expect("identity unambiguous")
            .expect("repo recorded")
            .objects["billing.credits"];
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
        let object = &workspace
            .repository(&repo)
            .expect("identity unambiguous")
            .expect("repo recorded")
            .objects["billing.credits"];
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
        let object = &workspace
            .repository(&repo)
            .expect("identity unambiguous")
            .expect("repo recorded")
            .objects["billing.credits"];
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
        assert_eq!(
            workspace.repository(&repo),
            Ok(None),
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
        assert_eq!(
            workspace.repository(&repo),
            Ok(None),
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
        assert_eq!(
            workspace.repository(&repo),
            Ok(None),
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

        let object = &workspace
            .repository(&repo)
            .expect("identity unambiguous")
            .expect("repo recorded")
            .objects["billing.credits"];
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
        let objects = &workspace
            .repository(&repo)
            .expect("identity unambiguous")
            .expect("repo recorded")
            .objects;
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
            workspace
                .repository(&repo_a)
                .expect("identity unambiguous")
                .expect("repo a")
                .objects
                .len(),
            1
        );
        assert_eq!(
            workspace
                .repository(&repo_b)
                .expect("identity unambiguous")
                .expect("repo b")
                .objects
                .len(),
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
        let objects = &workspace
            .repository(&repo)
            .expect("identity unambiguous")
            .expect("repo recorded")
            .objects;
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
        let object_a = &workspace
            .repository(&repo_a)
            .expect("identity unambiguous")
            .expect("repo a")
            .objects["billing.credits"];
        let object_b = &workspace
            .repository(&repo_b)
            .expect("identity unambiguous")
            .expect("repo b")
            .objects["billing.credits"];
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

        workspace
            .reserve_repository(repo.clone())
            .expect("identity unambiguous");

        let record = workspace
            .repository(&repo)
            .expect("identity unambiguous")
            .expect("slot reserved");
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
        let first = workspace
            .reserve_repository(repo.clone())
            .expect("identity unambiguous");
        let again = workspace
            .reserve_repository(repo.clone())
            .expect("identity unambiguous");
        assert_eq!(first, again, "identity-idempotent reserve never forks");

        workspace
            .import_artifact(&artifact(
                repo.clone(),
                vec![knowledge_object("billing.credits", "sha256:aaa")],
            ))
            .expect("import accepted");
        workspace
            .reserve_repository(repo.clone())
            .expect("identity unambiguous");

        let record = workspace
            .repository(&repo)
            .expect("identity unambiguous")
            .expect("repo recorded");
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
            .expect("identity unambiguous")
            .expect("repo recorded on arrival");
        assert!(record.objects.is_empty());
    }

    // E1.3.T1 (MILESTONES §E1.3; RT-03): a keep-distinct decision is
    // recorded as a Governance Event bound to exact version ID + policy
    // version + principal, and replays from history to the identical
    // resulting state. The record is `adoc.reconciliation_decision.v0`
    // (registered planned); E4.2's `adoc.governance_event.v0` (cloud)
    // will enclose it as the event payload.

    /// Registered (planned) contract id governing the serialized decision
    /// record shape pinned below.
    const RECONCILIATION_DECISION_CONTRACT: &str = "adoc.reconciliation_decision.v0";

    fn party(of: &ReconciliationParty) -> DecisionParty {
        DecisionParty {
            canonical: of.canonical.clone(),
            version_id: of.version_id.clone(),
        }
    }

    fn principal() -> Principal {
        Principal::new("user:alex").expect("principal is non-blank")
    }

    fn policy() -> PolicyVersion {
        PolicyVersion::new("reconciliation-policy/1").expect("policy version is non-blank")
    }

    /// Two repositories colliding on `billing.credits` — the import
    /// history every replay test rebuilds identically.
    fn collided_workspace() -> (ManagedWorkspace, ReconciliationCandidate) {
        let mut workspace = workspace();
        workspace
            .import_artifact(&artifact(
                local_repo("a/agentdoc.config.yaml"),
                vec![knowledge_object("billing.credits", "sha256:aaa")],
            ))
            .expect("import accepted");
        let outcome = workspace
            .import_artifact(&artifact(
                local_repo("b/agentdoc.config.yaml"),
                vec![knowledge_object("billing.credits", "sha256:bbb")],
            ))
            .expect("import accepted");
        let candidate = outcome.reconciliation_candidates[0].clone();
        (workspace, candidate)
    }

    /// Exit test (E1.3): a keep-distinct decision bound to exact version
    /// IDs, policy version, and principal is recorded as an event, and
    /// replaying the recorded history over the same import history yields
    /// a byte-identical reconciliation state — with both objects still
    /// distinct.
    #[test]
    fn keep_distinct_decision_is_principal_bound_and_replays_to_identical_state() {
        let (mut first_run, candidate) = collided_workspace();
        let decision = ReconciliationDecision::new(
            ReconciliationVerb::KeepDistinct,
            party(&candidate.existing),
            party(&candidate.incoming),
            principal(),
            policy(),
        )
        .expect("two distinct parties");
        first_run
            .record_decision(decision)
            .expect("decision recorded");

        let (mut replayed, _) = collided_workspace();
        for event in first_run.decisions().to_vec() {
            replayed.replay_decision(event).expect("replay accepted");
        }

        assert_eq!(first_run, replayed, "replay must rebuild the workspace");
        assert_eq!(
            serde_json::to_vec(&first_run.reconciliation_state()).expect("state serializes"),
            serde_json::to_vec(&replayed.reconciliation_state()).expect("state serializes"),
            "replayed reconciliation state must be byte-identical"
        );
        assert_ne!(
            candidate.existing.canonical, candidate.incoming.canonical,
            "keep-distinct leaves both objects distinct"
        );
        assert_eq!(first_run.reconciliation_state().standing.len(), 1);
    }

    /// A decision recorded between two imports of the same object stays
    /// replayable: rebuilding the workspace as "all imports, then the
    /// recorded log" must reach the identical state. Latest-version
    /// binding is a record-time admission check, not a log-validity
    /// invariant — replay trusts the log instead of re-running admission,
    /// which would reject a decision that legitimately entered it.
    #[test]
    fn decision_recorded_between_imports_replays_from_the_trusted_log() {
        let (mut first_run, candidate) = collided_workspace();
        let decision = ReconciliationDecision::new(
            ReconciliationVerb::KeepDistinct,
            party(&candidate.existing),
            party(&candidate.incoming),
            principal(),
            policy(),
        )
        .expect("two distinct parties");
        first_run
            .record_decision(decision)
            .expect("decision recorded");
        // A later import mints a newer version for the subject: the
        // recorded decision's binding is no longer the latest.
        first_run
            .import_artifact(&artifact(
                local_repo("a/agentdoc.config.yaml"),
                vec![knowledge_object("billing.credits", "sha256:aa2")],
            ))
            .expect("import accepted");

        let (mut replayed, _) = collided_workspace();
        replayed
            .import_artifact(&artifact(
                local_repo("a/agentdoc.config.yaml"),
                vec![knowledge_object("billing.credits", "sha256:aa2")],
            ))
            .expect("import accepted");
        for event in first_run.decisions().to_vec() {
            replayed.replay_decision(event).expect("replay accepted");
        }

        assert_eq!(first_run, replayed, "replay must rebuild the workspace");
        assert_eq!(
            serde_json::to_vec(&first_run.reconciliation_state()).expect("state serializes"),
            serde_json::to_vec(&replayed.reconciliation_state()).expect("state serializes"),
            "replayed reconciliation state must be byte-identical"
        );
    }

    /// Replaying a legitimately recorded log against a MISMATCHED import
    /// history — a truncated rebuild, decisions restored ahead of the
    /// imports that produced them, the wrong workspace — must fail
    /// closed: standing state naming canonicals that resolve to no
    /// managed object would break RT-03's provenance-stays-queryable
    /// promise silently. Party existence is a log-plus-history
    /// invariant; only the latest-version comparison is record-time.
    #[test]
    fn replaying_against_a_mismatched_import_history_fails_closed() {
        let (mut first_run, candidate) = collided_workspace();
        let decision = ReconciliationDecision::new(
            ReconciliationVerb::KeepDistinct,
            party(&candidate.existing),
            party(&candidate.incoming),
            principal(),
            policy(),
        )
        .expect("two distinct parties");
        first_run
            .record_decision(decision)
            .expect("decision recorded");

        // No imports replayed: neither party exists in this workspace.
        let mut fresh =
            ManagedWorkspace::new(WorkspaceId::new("ws-acme").expect("workspace id is non-blank"));
        assert_eq!(
            fresh.replay_decision(first_run.decisions()[0].clone()),
            Err(ReconciliationDecisionError::UnknownParty),
            "a decision whose parties are absent from the import history must not replay"
        );
        assert!(
            fresh.decisions().is_empty(),
            "nothing may be appended on rejection"
        );
    }

    /// The serialized decision record the Cloud cut consumes under the
    /// planned `adoc.reconciliation_decision.v0` registry row: closed verb
    /// set, both parties bound by workspace canonical identity + exact
    /// managed version id, non-optional principal and policy version, and
    /// no wall-clock field anywhere (Cloud's enclosing governance event
    /// owns occurrence time).
    #[test]
    fn reconciliation_decision_serialized_record_shape_is_pinned() {
        let (_, candidate) = collided_workspace();
        let decision = ReconciliationDecision::new(
            ReconciliationVerb::KeepDistinct,
            party(&candidate.existing),
            party(&candidate.incoming),
            principal(),
            policy(),
        )
        .expect("two distinct parties");

        let value = serde_json::to_value(&decision).expect("decision serializes");
        assert_eq!(
            value,
            json!({
                "verb": "keep_distinct",
                "subject": {
                    "canonical": {
                        "workspace_id": "ws-acme",
                        "canonical_id": "mo-1"
                    },
                    "version_id": "mv-1"
                },
                "counterpart": {
                    "canonical": {
                        "workspace_id": "ws-acme",
                        "canonical_id": "mo-2"
                    },
                    "version_id": "mv-2"
                },
                "principal": "user:alex",
                "policy_version": "reconciliation-policy/1"
            }),
            "the {RECONCILIATION_DECISION_CONTRACT} record shape drifted"
        );
    }

    /// A later decision for the same pair supersedes the earlier one in
    /// the derived state (append-only log, last decision stands), and the
    /// full history stays recorded.
    #[test]
    fn later_decision_for_the_same_pair_stands_while_history_is_kept() {
        let (mut workspace, candidate) = collided_workspace();
        for verb in [
            ReconciliationVerb::KeepDistinct,
            ReconciliationVerb::LinkAlias,
        ] {
            let decision = ReconciliationDecision::new(
                verb,
                party(&candidate.existing),
                party(&candidate.incoming),
                principal(),
                policy(),
            )
            .expect("two distinct parties");
            workspace
                .record_decision(decision)
                .expect("decision recorded");
        }

        assert_eq!(workspace.decisions().len(), 2, "the log is append-only");
        let state = workspace.reconciliation_state();
        assert_eq!(state.standing.len(), 1);
        assert_eq!(state.standing[0].verb(), ReconciliationVerb::LinkAlias);
    }

    // E1.3.T2 (MILESTONES §E1.3; RT-03): link/alias and supersede
    // decisions preserve every original Source Record/Assertion/Binding —
    // at the E1.2 representation level: every ManagedVersionRecord with
    // its Source Binding and Source Assertion list — with no observation
    // rewritten or dropped.

    fn bound_object(id: &str, hash: &str, path: &str, digest: &str) -> GraphKnowledgeObjectNode {
        let mut node = knowledge_object(id, hash);
        node.source_binding = Some(GraphSourceBinding {
            connector: "local_fs".to_string(),
            source: "team.page".to_string(),
            revision: None,
            path: path.to_string(),
            anchor: id.to_string(),
            source_revision_digest: digest.to_string(),
        });
        node
    }

    /// Two repositories colliding on `billing.credits`, every version
    /// carrying its own Source Binding and two versions on the existing
    /// side — the provenance a decision must preserve.
    fn collided_workspace_with_bindings() -> (ManagedWorkspace, DecisionParty, DecisionParty) {
        let mut workspace = workspace();
        let repo_a = local_repo("a/agentdoc.config.yaml");
        let repo_b = local_repo("b/agentdoc.config.yaml");
        workspace
            .import_artifact(&artifact(
                repo_a.clone(),
                vec![bound_object(
                    "billing.credits",
                    "sha256:aaa",
                    "docs/team.adoc",
                    "sha256:feed",
                )],
            ))
            .expect("import accepted");
        workspace
            .import_artifact(&artifact(
                repo_a,
                vec![bound_object(
                    "billing.credits",
                    "sha256:aa2",
                    "docs/team.adoc",
                    "sha256:f00d",
                )],
            ))
            .expect("import accepted");
        let outcome = workspace
            .import_artifact(&artifact(
                repo_b,
                vec![bound_object(
                    "billing.credits",
                    "sha256:bbb",
                    "docs/other.adoc",
                    "sha256:beef",
                )],
            ))
            .expect("import accepted");
        let candidate = &outcome.reconciliation_candidates[0];
        let (subject, counterpart) = (party(&candidate.existing), party(&candidate.incoming));
        (workspace, subject, counterpart)
    }

    /// RT-03 fixture shared by the link/alias and supersede tests: the
    /// decision appends one Governance Event and changes nothing else —
    /// every original version record, Source Binding, and (empty until
    /// E4.1) Source Assertion list survives byte-for-byte, both parties
    /// stay individually queryable, and the standing state records the
    /// verb with its direction.
    fn decision_preserves_every_original_record(verb: ReconciliationVerb) {
        let (mut workspace, subject, counterpart) = collided_workspace_with_bindings();
        let before = workspace.clone();
        let decision = ReconciliationDecision::new(
            verb,
            subject.clone(),
            counterpart.clone(),
            principal(),
            policy(),
        )
        .expect("two distinct parties");

        workspace
            .record_decision(decision)
            .expect("decision recorded");

        for repo in [
            local_repo("a/agentdoc.config.yaml"),
            local_repo("b/agentdoc.config.yaml"),
        ] {
            assert_eq!(
                workspace.repository(&repo),
                before.repository(&repo),
                "a decision must not rewrite or drop any repository record"
            );
        }
        let subject_object = workspace
            .managed_object(&subject.canonical)
            .expect("subject stays queryable");
        assert_eq!(subject_object.versions.len(), 2);
        assert!(
            subject_object
                .versions
                .iter()
                .all(|version| version.source_binding.is_some()),
            "every original Source Binding survives the decision"
        );
        let counterpart_object = workspace
            .managed_object(&counterpart.canonical)
            .expect("counterpart stays queryable");
        assert_eq!(counterpart_object.versions.len(), 1);
        assert!(counterpart_object.versions[0].source_binding.is_some());

        let state = workspace.reconciliation_state();
        assert_eq!(state.standing.len(), 1);
        assert_eq!(state.standing[0].verb(), verb);
        assert_eq!(state.standing[0].subject(), &subject);
        assert_eq!(state.standing[0].counterpart(), &counterpart);
    }

    /// E1.3.T2: linking an alias preserves every original record; the
    /// counterpart becomes an alias of the subject in the standing state
    /// only — no observation is rewritten or dropped.
    #[test]
    fn link_alias_decision_preserves_every_original_record() {
        decision_preserves_every_original_record(ReconciliationVerb::LinkAlias);
    }

    /// E1.3.T2: superseding preserves every original record; the
    /// superseded counterpart stays fully queryable with its provenance —
    /// no observation is rewritten or dropped.
    #[test]
    fn supersede_decision_preserves_every_original_record() {
        decision_preserves_every_original_record(ReconciliationVerb::Supersede);
    }

    // E1.3.T3 (MILESTONES §E1.3; RT-03): merge/re-home is explicit. The
    // ONLY path to merged state is a recorded principal-bound decision —
    // matching ID, hash, title, or similarity yields candidates at most —
    // and the provenance of both antecedents stays retained and
    // queryable afterwards.

    /// Exit + adversarial test: identical Object ID AND identical
    /// `content_hash` across two repositories — the most merge-tempting
    /// collision — yields a candidate only. No merged relation exists
    /// until a recorded merge/re-home decision transitions the pair, and
    /// afterwards both antecedents' full provenance remains queryable.
    #[test]
    fn merged_state_is_reachable_only_through_a_recorded_decision() {
        let mut workspace = workspace();
        let repo_a = local_repo("a/agentdoc.config.yaml");
        let repo_b = local_repo("b/agentdoc.config.yaml");
        workspace
            .import_artifact(&artifact(
                repo_a.clone(),
                vec![bound_object(
                    "billing.credits",
                    "sha256:same",
                    "docs/team.adoc",
                    "sha256:feed",
                )],
            ))
            .expect("import accepted");
        let outcome = workspace
            .import_artifact(&artifact(
                repo_b.clone(),
                vec![bound_object(
                    "billing.credits",
                    "sha256:same",
                    "docs/other.adoc",
                    "sha256:beef",
                )],
            ))
            .expect("import accepted");

        // Identical ID + hash: a candidate, never a merge — and no
        // standing reconciliation relation of any kind.
        assert_eq!(outcome.reconciliation_candidates.len(), 1);
        let candidate = &outcome.reconciliation_candidates[0];
        let survivor = party(&candidate.existing);
        let antecedent = party(&candidate.incoming);
        assert_ne!(survivor.canonical, antecedent.canonical);
        assert!(workspace.reconciliation_state().standing.is_empty());
        assert!(workspace.merged_antecedents(&survivor.canonical).is_empty());
        assert!(
            workspace
                .merged_antecedents(&antecedent.canonical)
                .is_empty()
        );

        let decision = ReconciliationDecision::new(
            ReconciliationVerb::MergeRehome,
            survivor.clone(),
            antecedent.clone(),
            principal(),
            policy(),
        )
        .expect("two distinct parties");
        workspace
            .record_decision(decision)
            .expect("decision recorded");

        // The recorded decision is the merged state — directional, and
        // derived only from the log.
        let state = workspace.reconciliation_state();
        assert_eq!(state.standing.len(), 1);
        assert_eq!(state.standing[0].verb(), ReconciliationVerb::MergeRehome);
        assert_eq!(
            workspace.merged_antecedents(&survivor.canonical),
            vec![antecedent.canonical.clone()]
        );
        assert!(
            workspace
                .merged_antecedents(&antecedent.canonical)
                .is_empty(),
            "merge direction must not invert"
        );

        // Provenance of both antecedents retained and queryable (RT-03).
        for bound in [&survivor, &antecedent] {
            let object = workspace
                .managed_object(&bound.canonical)
                .expect("antecedent stays queryable");
            assert_eq!(object.versions.len(), 1);
            assert!(object.versions[0].source_binding.is_some());
        }
        assert_eq!(
            workspace
                .repository(&repo_a)
                .expect("identity unambiguous")
                .expect("repo a retained")
                .objects
                .len(),
            1
        );
        assert_eq!(
            workspace
                .repository(&repo_b)
                .expect("identity unambiguous")
                .expect("repo b retained")
                .objects
                .len(),
            1
        );
    }

    /// Stop-ship (MILESTONES §E1.3): a decision without principal, policy,
    /// or a pair to adjudicate is unconstructible — the only constructor
    /// takes typed non-optional bindings, and blank or padded authority
    /// context fails closed at construction.
    #[test]
    fn unbound_authority_context_is_unconstructible() {
        for invalid in ["", " \t", " user:alex", "user:alex "] {
            assert_eq!(
                Principal::new(invalid),
                Err(ReconciliationDecisionError::InvalidPrincipal)
            );
            assert_eq!(
                PolicyVersion::new(invalid),
                Err(ReconciliationDecisionError::InvalidPolicyVersion)
            );
        }

        let (_, subject, _) = collided_workspace_with_bindings();
        assert_eq!(
            ReconciliationDecision::new(
                ReconciliationVerb::MergeRehome,
                subject.clone(),
                subject,
                principal(),
                policy(),
            ),
            Err(ReconciliationDecisionError::SelfDecision),
            "one object is not a pair to adjudicate"
        );
    }

    /// Recording fails closed on a party this workspace does not know —
    /// including a canonical identity from another workspace — and on a
    /// version binding that is no longer the party's latest: a decision
    /// adjudicated against content that has since changed never applies
    /// silently.
    #[test]
    fn recording_rejects_unknown_parties_and_stale_version_bindings() {
        let (mut workspace, subject, counterpart) = collided_workspace_with_bindings();

        let foreign = DecisionParty {
            canonical: WorkspaceCanonicalIdentity {
                workspace_id: WorkspaceId::new("ws-other").expect("non-blank"),
                canonical_id: subject.canonical.canonical_id.clone(),
            },
            version_id: subject.version_id.clone(),
        };
        let decision = ReconciliationDecision::new(
            ReconciliationVerb::MergeRehome,
            subject.clone(),
            foreign,
            principal(),
            policy(),
        )
        .expect("two distinct parties");
        assert_eq!(
            workspace.record_decision(decision),
            Err(ReconciliationDecisionError::UnknownParty),
            "an equal opaque suffix in another workspace is not this party"
        );

        // A new version arrives for the subject: the old exact-version
        // binding is stale and the decision must not apply.
        workspace
            .import_artifact(&artifact(
                local_repo("a/agentdoc.config.yaml"),
                vec![bound_object(
                    "billing.credits",
                    "sha256:aa3",
                    "docs/team.adoc",
                    "sha256:cafe",
                )],
            ))
            .expect("import accepted");
        let stale = ReconciliationDecision::new(
            ReconciliationVerb::MergeRehome,
            subject,
            counterpart,
            principal(),
            policy(),
        )
        .expect("two distinct parties");
        assert_eq!(
            workspace.record_decision(stale),
            Err(ReconciliationDecisionError::StaleVersionBinding)
        );
        assert!(
            workspace.decisions().is_empty(),
            "a rejected decision must not enter the log"
        );
    }

    /// Three repositories colliding on `billing.credits` — parties A, B,
    /// C at their latest versions, every pair a candidate.
    fn three_way_collision() -> (
        ManagedWorkspace,
        DecisionParty,
        DecisionParty,
        DecisionParty,
    ) {
        let mut workspace = workspace();
        let mut parties = Vec::new();
        for (repo, hash) in [
            ("a/agentdoc.config.yaml", "sha256:aaa"),
            ("b/agentdoc.config.yaml", "sha256:bbb"),
            ("c/agentdoc.config.yaml", "sha256:ccc"),
        ] {
            let outcome = workspace
                .import_artifact(&artifact(
                    local_repo(repo),
                    vec![knowledge_object("billing.credits", hash)],
                ))
                .expect("import accepted");
            parties.push(DecisionParty {
                canonical: outcome.imported[0].canonical.clone(),
                version_id: outcome.imported[0].version_id.clone(),
            });
        }
        let c = parties.pop().expect("three imports");
        let b = parties.pop().expect("three imports");
        let a = parties.pop().expect("three imports");
        (workspace, a, b, c)
    }

    fn decide(
        verb: ReconciliationVerb,
        subject: &DecisionParty,
        counterpart: &DecisionParty,
    ) -> ReconciliationDecision {
        ReconciliationDecision::new(
            verb,
            subject.clone(),
            counterpart.clone(),
            principal(),
            policy(),
        )
        .expect("two distinct parties")
    }

    /// A standing merge freezes its merged-away counterpart: once C is
    /// merged into A, any decision naming C under a different pair —
    /// merging it into B too, or re-deciding it under any verb — is
    /// rejected fail-closed rather than deriving a state where C is
    /// re-homed into two survivors at once. A later decision on the SAME
    /// pair still supersedes (last decision stands), after which the
    /// un-merged party is decidable again.
    #[test]
    fn recording_rejects_decisions_naming_a_merged_away_party() {
        let (mut workspace, a, b, c) = three_way_collision();
        workspace
            .record_decision(decide(ReconciliationVerb::MergeRehome, &a, &c))
            .expect("first merge stands");

        assert_eq!(
            workspace.record_decision(decide(ReconciliationVerb::MergeRehome, &b, &c)),
            Err(ReconciliationDecisionError::PartyAlreadyMerged),
            "one counterpart must not be re-homed into two survivors"
        );
        assert_eq!(
            workspace.record_decision(decide(ReconciliationVerb::KeepDistinct, &c, &b)),
            Err(ReconciliationDecisionError::PartyAlreadyMerged),
            "a merged-away party is frozen for every verb"
        );
        assert_eq!(
            workspace.merged_antecedents(&a.canonical),
            vec![c.canonical.clone()]
        );
        assert!(workspace.merged_antecedents(&b.canonical).is_empty());

        workspace
            .record_decision(decide(ReconciliationVerb::KeepDistinct, &a, &c))
            .expect("a same-pair decision supersedes the standing merge");
        workspace
            .record_decision(decide(ReconciliationVerb::MergeRehome, &b, &c))
            .expect("an un-merged party is decidable again");
        assert!(workspace.merged_antecedents(&a.canonical).is_empty());
        assert_eq!(
            workspace.merged_antecedents(&b.canonical),
            vec![c.canonical.clone()]
        );
    }

    /// Merging away a survivor that holds merged antecedents would chain
    /// merges — nested reconciliation, out of scope for this slice
    /// (MILESTONES §E1.3) — and the single-level antecedent walk would
    /// silently lose the survivor's own antecedents. Rejected fail-closed;
    /// a non-merge decision on the survivor stays legal.
    #[test]
    fn recording_rejects_a_merge_chain_into_a_standing_survivor() {
        let (mut workspace, a, b, c) = three_way_collision();
        workspace
            .record_decision(decide(ReconciliationVerb::MergeRehome, &b, &c))
            .expect("first merge stands");

        assert_eq!(
            workspace.record_decision(decide(ReconciliationVerb::MergeRehome, &a, &b)),
            Err(ReconciliationDecisionError::NestedMergeUnsupported)
        );
        assert_eq!(
            workspace.merged_antecedents(&b.canonical),
            vec![c.canonical.clone()]
        );
        assert!(workspace.merged_antecedents(&a.canonical).is_empty());

        workspace
            .record_decision(decide(ReconciliationVerb::KeepDistinct, &a, &b))
            .expect("a non-merge decision on the survivor stays legal");
    }

    /// One survivor absorbing several antecedents through separate
    /// decisions is not a chain: merge C into A, then B into A — both
    /// stand, deterministically ordered by pair key.
    #[test]
    fn one_survivor_may_absorb_multiple_antecedents() {
        let (mut workspace, a, b, c) = three_way_collision();
        workspace
            .record_decision(decide(ReconciliationVerb::MergeRehome, &a, &c))
            .expect("first merge stands");
        workspace
            .record_decision(decide(ReconciliationVerb::MergeRehome, &a, &b))
            .expect("a survivor may absorb another antecedent");
        assert_eq!(
            workspace.merged_antecedents(&a.canonical),
            vec![b.canonical.clone(), c.canonical.clone()]
        );
    }

    /// A Reconciliation Decision decides a Reconciliation Candidate pair
    /// (CONTEXT.md), and candidates arise from exact Object-ID collision
    /// alone (RT-03): two objects that never collided are not a pair to
    /// adjudicate, whatever the verb — recording fails closed instead of
    /// letting any two known objects be linked, superseded, or merged.
    #[test]
    fn recording_rejects_parties_that_never_formed_a_candidate_pair() {
        let mut workspace = workspace();
        let first = workspace
            .import_artifact(&artifact(
                local_repo("a/agentdoc.config.yaml"),
                vec![knowledge_object("billing.credits", "sha256:aaa")],
            ))
            .expect("import accepted");
        let second = workspace
            .import_artifact(&artifact(
                local_repo("b/agentdoc.config.yaml"),
                vec![knowledge_object("billing.refunds", "sha256:bbb")],
            ))
            .expect("import accepted");

        let uncollided = |of: &ImportedVersion| DecisionParty {
            canonical: of.canonical.clone(),
            version_id: of.version_id.clone(),
        };
        for verb in [
            ReconciliationVerb::KeepDistinct,
            ReconciliationVerb::LinkAlias,
            ReconciliationVerb::Supersede,
            ReconciliationVerb::MergeRehome,
        ] {
            let decision = ReconciliationDecision::new(
                verb,
                uncollided(&first.imported[0]),
                uncollided(&second.imported[0]),
                principal(),
                policy(),
            )
            .expect("two distinct parties");
            assert_eq!(
                workspace.record_decision(decision),
                Err(ReconciliationDecisionError::NotACandidatePair)
            );
        }
        assert!(
            workspace.decisions().is_empty(),
            "a rejected decision must not enter the log"
        );
    }

    /// The closed decision verb vocabulary on the wire — exactly the four
    /// verbs MILESTONES §E1.3 names, snake_case, nothing else.
    #[test]
    fn decision_verbs_are_a_closed_wire_vocabulary() {
        for (verb, wire) in [
            (ReconciliationVerb::KeepDistinct, "keep_distinct"),
            (ReconciliationVerb::LinkAlias, "link_alias"),
            (ReconciliationVerb::Supersede, "supersede"),
            (ReconciliationVerb::MergeRehome, "merge_rehome"),
        ] {
            assert_eq!(
                serde_json::to_value(verb).expect("verb serializes"),
                json!(wire)
            );
        }
    }

    // E1.3 routing adjudication (PR #150 claim 5): CLI builds emit a
    // degenerate `repository_identity` — every project-bound build carries
    // `{local_project, "agentdoc.config.yaml"}` — so identity-equality
    // lookup cannot distinguish two physical repositories in one
    // workspace. The reserved binding slot (V10.3.2) is the disambiguator:
    // import routes by slot handle; identity lookup stays only as the
    // single-repo convenience.

    /// Two physical repositories with the SAME degenerate identity, routed
    /// to two reserved slots: distinct object registries, and a same-ID
    /// import across them still produces a reconciliation candidate.
    #[test]
    fn two_slots_sharing_a_degenerate_identity_stay_distinct_and_reconcile() {
        let mut workspace = workspace();
        let degenerate = local_repo("agentdoc.config.yaml");
        let slot_a = workspace.reserve_repository_slot(degenerate.clone());
        let slot_b = workspace.reserve_repository_slot(degenerate.clone());
        assert_ne!(slot_a, slot_b);

        let first = workspace
            .import_artifact_into(
                &slot_a,
                &artifact(
                    degenerate.clone(),
                    vec![knowledge_object("billing.credits", "sha256:aaa")],
                ),
            )
            .expect("routed import accepted");
        assert!(first.reconciliation_candidates.is_empty());

        let second = workspace
            .import_artifact_into(
                &slot_b,
                &artifact(
                    degenerate.clone(),
                    vec![knowledge_object("billing.credits", "sha256:bbb")],
                ),
            )
            .expect("routed import accepted");

        let object_a =
            &workspace.repository_slot(&slot_a).expect("slot a").objects["billing.credits"];
        let object_b =
            &workspace.repository_slot(&slot_b).expect("slot b").objects["billing.credits"];
        assert_ne!(
            object_a.canonical, object_b.canonical,
            "slots sharing an identity must keep distinct object registries"
        );
        assert_eq!(object_a.versions[0].content_hash, "sha256:aaa");
        assert_eq!(object_b.versions[0].content_hash, "sha256:bbb");

        assert_eq!(second.reconciliation_candidates.len(), 1);
        let candidate = &second.reconciliation_candidates[0];
        assert_eq!(candidate.reason, ReconciliationReason::ObjectIdCollision);
        assert_eq!(candidate.existing.canonical, object_a.canonical);
        assert_eq!(candidate.incoming.canonical, object_b.canonical);
    }

    /// Identity-equality lookup remains only for the single-repo case:
    /// once two slots share the identity, identity-routed import is
    /// ambiguous and fails closed instead of guessing a slot.
    #[test]
    fn identity_routed_import_fails_closed_when_two_slots_share_the_identity() {
        let mut workspace = workspace();
        let degenerate = local_repo("agentdoc.config.yaml");
        let slot_a = workspace.reserve_repository_slot(degenerate.clone());
        workspace.reserve_repository_slot(degenerate.clone());

        let outcome = workspace.import_artifact(&artifact(
            degenerate.clone(),
            vec![knowledge_object("billing.credits", "sha256:aaa")],
        ));

        assert_eq!(
            outcome,
            Err(ManagedIdentityError::AmbiguousRepositoryIdentity)
        );
        assert_eq!(
            workspace.repository(&degenerate),
            Err(ManagedIdentityError::AmbiguousRepositoryIdentity),
            "the read lookup distinguishes ambiguity from absence"
        );
        assert!(
            workspace
                .repository_slot(&slot_a)
                .expect("slot reserved")
                .objects
                .is_empty(),
            "an ambiguous import must not touch any slot"
        );
        assert!(
            workspace.reserve_repository(degenerate).is_err(),
            "identity-idempotent reservation is equally ambiguous"
        );
    }

    /// Explicit routing fails closed on an unknown slot handle and on an
    /// artifact whose identity differs from the slot's, with no state
    /// change either way.
    #[test]
    fn routed_import_rejects_unknown_slot_and_identity_mismatch_without_state() {
        let mut workspace = workspace();
        let repo = local_repo("a/agentdoc.config.yaml");
        let slot = workspace.reserve_repository_slot(repo.clone());
        let document = artifact(
            repo.clone(),
            vec![knowledge_object("billing.credits", "sha256:aaa")],
        );

        let unreserved = RepositorySlotId("slot-99".to_string());
        assert_eq!(
            workspace.import_artifact_into(&unreserved, &document),
            Err(ManagedIdentityError::UnknownRepositorySlot)
        );

        let mismatched = artifact(
            local_repo("b/agentdoc.config.yaml"),
            vec![knowledge_object("billing.credits", "sha256:aaa")],
        );
        assert_eq!(
            workspace.import_artifact_into(&slot, &mismatched),
            Err(ManagedIdentityError::RepositoryIdentityMismatch)
        );
        assert!(
            workspace
                .repository_slot(&slot)
                .expect("slot reserved")
                .objects
                .is_empty(),
            "a rejected routed import must not mint anything"
        );
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
            .expect("identity unambiguous")
            .expect("project repo")
            .objects["billing.credits"];
        let in_standalone = &workspace
            .repository(&standalone)
            .expect("identity unambiguous")
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
