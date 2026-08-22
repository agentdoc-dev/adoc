//! Managed state events over immutable content versions (E1.4;
//! ADR-0057 invariant 2; RED-TEAM-CLOSURE §RT-04; KNOWLEDGE-MODEL §K4).
//!
//! Content versions are immutable ([`ManagedVersionRecord`](super::managed);
//! E1.2); every managed state change is an append-only
//! [`ManagedStateEvent`] recorded over one exact content version. A
//! state-only transition (freshness `current` → `stale`, a verification
//! outcome, a sync observation, …) never alters the version's
//! `content_hash` and never mints a new content version — only a semantic
//! content change does, on import (RT-04).
//!
//! The six managed state dimensions of §K4 — governance, verification,
//! effectivity, freshness, integrity, per-connector synchronization — are
//! modeled as one closed enum per dimension, never a collapsed status
//! field (D07/D15): a value of one dimension is unrepresentable in
//! another at the type level. The vocabularies are CLOSED — a new value
//! is a registry edit plus a §K4 amendment, never an ad hoc string.
//!
//! No wall-clock exists anywhere in these records: "time T" is the
//! recorded event order ([`EventOrdinal`]), so replaying the same log
//! yields byte-identical state on every machine.

use serde::Serialize;

use super::diagnostic::DiagnosticCode;
use super::managed::{ManagedVersionId, WorkspaceCanonicalIdentity};
use super::reconciliation::PolicyVersion;

/// §K4 governance dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GovernanceState {
    Proposed,
    Approved,
    Rejected,
    Revoked,
}

/// §K4 verification dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum VerificationState {
    Unverified,
    PartiallyVerified,
    Verified,
    Failed,
}

/// §K4 effectivity dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EffectivityState {
    Pending,
    Scheduled,
    Effective,
    Suspended,
    Expired,
}

/// §K4 freshness dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FreshnessState {
    Current,
    NeedsReview,
    Stale,
}

/// §K4 integrity dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum IntegrityState {
    Clear,
    PotentiallyConflicting,
    Contradicted,
}

/// §K4 synchronization dimension — always per connector, carried with the
/// connector's `required_before_effective` flag on every sync event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SynchronizationState {
    InSync,
    PendingWriteback,
    PendingExternalApproval,
    WritebackFailed,
    SourceAhead,
    SourceDiverged,
    Paused,
    NotApplicable,
}

impl GovernanceState {
    pub(crate) const ALL: [Self; 4] = [
        Self::Proposed,
        Self::Approved,
        Self::Rejected,
        Self::Revoked,
    ];

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Revoked => "revoked",
        }
    }
}

impl VerificationState {
    pub(crate) const ALL: [Self; 4] = [
        Self::Unverified,
        Self::PartiallyVerified,
        Self::Verified,
        Self::Failed,
    ];

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Unverified => "unverified",
            Self::PartiallyVerified => "partially_verified",
            Self::Verified => "verified",
            Self::Failed => "failed",
        }
    }
}

impl EffectivityState {
    pub(crate) const ALL: [Self; 5] = [
        Self::Pending,
        Self::Scheduled,
        Self::Effective,
        Self::Suspended,
        Self::Expired,
    ];

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Scheduled => "scheduled",
            Self::Effective => "effective",
            Self::Suspended => "suspended",
            Self::Expired => "expired",
        }
    }
}

impl FreshnessState {
    pub(crate) const ALL: [Self; 3] = [Self::Current, Self::NeedsReview, Self::Stale];

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::NeedsReview => "needs_review",
            Self::Stale => "stale",
        }
    }
}

impl IntegrityState {
    pub(crate) const ALL: [Self; 3] = [
        Self::Clear,
        Self::PotentiallyConflicting,
        Self::Contradicted,
    ];

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Clear => "clear",
            Self::PotentiallyConflicting => "potentially_conflicting",
            Self::Contradicted => "contradicted",
        }
    }
}

impl SynchronizationState {
    pub(crate) const ALL: [Self; 8] = [
        Self::InSync,
        Self::PendingWriteback,
        Self::PendingExternalApproval,
        Self::WritebackFailed,
        Self::SourceAhead,
        Self::SourceDiverged,
        Self::Paused,
        Self::NotApplicable,
    ];

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::InSync => "in_sync",
            Self::PendingWriteback => "pending_writeback",
            Self::PendingExternalApproval => "pending_external_approval",
            Self::WritebackFailed => "writeback_failed",
            Self::SourceAhead => "source_ahead",
            Self::SourceDiverged => "source_diverged",
            Self::Paused => "paused",
            Self::NotApplicable => "not_applicable",
        }
    }
}

/// One connector's identity within a workspace — opaque to `adoc-core`.
/// Non-blank without surrounding whitespace, never normalized (the
/// fail-closed posture of every identity value since E1.2: two spellings
/// must never silently unify, and a blank connector would make the
/// per-connector sync dimension unaddressable).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub(crate) struct ConnectorId(String);

impl ConnectorId {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, ManagedStateEventError> {
        let value = value.into();
        if value.is_empty() || value.trim() != value {
            Err(ManagedStateEventError::InvalidConnector)
        } else {
            Ok(Self(value))
        }
    }
}

/// The component that emitted a state event (a connector observer, the
/// governance service, a verification runner, …). Recorded on every event
/// so historical reconstruction can tell "no emitter existed yet" from
/// "the emitter observed nothing" (E1.4.T3 gap markers) and so the audit
/// coverage guard (E1.4.T4) has a wiring registry to diff against. Same
/// validation posture as [`ConnectorId`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub(crate) struct EventEmitter(String);

impl EventEmitter {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, ManagedStateEventError> {
        let value = value.into();
        if value.is_empty() || value.trim() != value {
            Err(ManagedStateEventError::InvalidEmitter)
        } else {
            Ok(Self(value))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ManagedStateEventError {
    #[error("connector id must be non-blank without surrounding whitespace")]
    InvalidConnector,
    #[error("event emitter must be non-blank without surrounding whitespace")]
    InvalidEmitter,
}

/// The immutable content version a state event attaches to (ADR-0057
/// invariant 2): the Managed Object's workspace canonical identity plus
/// the exact managed version ID — never a bare Object ID, which is
/// neither workspace-qualified nor version-exact.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(crate) struct StateEventSubject {
    pub(crate) canonical: WorkspaceCanonicalIdentity,
    pub(crate) version_id: ManagedVersionId,
}

/// What one managed state event records, tagged by its event family
/// (RT-04). One variant per §K4 dimension, each carrying only that
/// dimension's closed vocabulary — dimensions are never conflated into a
/// shared status field (D07/D15). The remaining RT-04 families
/// (authorization-affecting source changes, declassification, migration,
/// deletion/tombstone) land in E1.4.T5.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "family", rename_all = "snake_case")]
pub(crate) enum ManagedStateChange {
    Governance {
        state: GovernanceState,
    },
    Verification {
        state: VerificationState,
    },
    Effectivity {
        state: EffectivityState,
    },
    Freshness {
        state: FreshnessState,
    },
    Integrity {
        state: IntegrityState,
    },
    Synchronization {
        connector: ConnectorId,
        state: SynchronizationState,
        required_before_effective: bool,
    },
}

/// One managed state event: an append-only record of one state change
/// over one immutable content version, carrying the emitting component
/// and the exact policy version under which it was emitted (ADR-0057
/// invariant 2: reconstruction needs immutable versions + state events +
/// recorded policy/contract versions). Deliberately wall-clock-free.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ManagedStateEvent {
    pub(crate) subject: StateEventSubject,
    pub(crate) change: ManagedStateChange,
    pub(crate) emitter: EventEmitter,
    pub(crate) policy_version: PolicyVersion,
    /// A correction is a NEW record referencing the corrected one
    /// (V10.4.2): the referenced record stays byte-identical in the log
    /// forever. `None` for ordinary events.
    pub(crate) corrects: Option<EventOrdinal>,
}

/// The position of one recorded event in the append-only log — the only
/// notion of time in the managed state model. "Historical state at time
/// T" means "state after replaying events `0..=T`".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub(crate) struct EventOrdinal(pub(crate) u64);

/// One event as recorded: the store-assigned ordinal plus the event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct RecordedStateEvent {
    pub(crate) ordinal: EventOrdinal,
    pub(crate) event: ManagedStateEvent,
}

/// The retention floor (V10.4.1/K9), in recorded-event-order terms: the
/// count of most-recent records every sweep must leave fully retained.
/// Records inside that span cannot be deleted by any API — and in E1.4
/// no delete path exists at all: sweeps only plan (execution is
/// V10.7.3/E6.6), so the floor is enforced at the earliest possible
/// surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RetentionFloor(pub(crate) u64);

/// A validated — never executed — retention sweep: the ordinals below
/// the floor-protected span. Deletion workflows above the floor arrive
/// in E6.6 (V10.7.3) and will consume this plan; nothing in E1.4
/// removes a record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RetentionSweepPlan {
    pub(crate) sweepable: Vec<EventOrdinal>,
}

/// Why the append-only store rejected a write-shaped request. Every
/// externally observable variant maps to a registered wire code via
/// [`StateEventStoreError::diagnostic_code`] — fail closed, never a
/// silent fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum StateEventStoreError {
    #[error(
        "ordinal {claimed:?} conflicts with the recorded log; records are append-only and only the next ordinal appends"
    )]
    RecordConflict { claimed: EventOrdinal },
    #[error("correction references ordinal {corrects:?}, which the log does not carry")]
    CorrectionTargetMissing { corrects: EventOrdinal },
    #[error(
        "sweep below {delete_below:?} reaches into the span protected by the retention floor (from {protected_from:?})"
    )]
    RetentionFloorViolation {
        delete_below: EventOrdinal,
        protected_from: EventOrdinal,
    },
}

impl StateEventStoreError {
    /// The registered wire code for this rejection. Both conflict shapes
    /// — an occupied/future-ordinal write and a correction naming an
    /// unrecorded target — are appends contradicting recorded history,
    /// the `governance.record_conflict` family (V10.4.2).
    pub(crate) fn diagnostic_code(&self) -> DiagnosticCode {
        match self {
            Self::RecordConflict { .. } | Self::CorrectionTargetMissing { .. } => {
                DiagnosticCode::GovernanceRecordConflict
            }
            Self::RetentionFloorViolation { .. } => DiagnosticCode::StoreRetentionFloorViolation,
        }
    }
}

/// The append-only managed state event log. Appending is the ONLY
/// mutation this type exposes — there is no update or delete method at
/// all, so append-only holds at the store layer rather than by handler
/// discipline. Write-shaped requests that contradict the recorded log
/// fail closed: an in-place update attempt is
/// `governance.record_conflict`, a sweep into the floor-protected span
/// is `store.retention_floor_violation` (E1.4.T2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManagedStateEventStore {
    events: Vec<RecordedStateEvent>,
    retention_floor: RetentionFloor,
}

impl ManagedStateEventStore {
    pub(crate) fn new(retention_floor: RetentionFloor) -> Self {
        Self {
            events: Vec::new(),
            retention_floor,
        }
    }

    /// Append one state event, assigning the next ordinal. Never touches
    /// any managed version record: a state-only transition leaves every
    /// version ID and `content_hash` unchanged and creates no content
    /// version (RT-04) — this store has no reference to the workspace's
    /// version records at all, so the invariant holds by construction.
    /// A correction must reference a recorded ordinal; a dangling one
    /// fails closed and appends nothing.
    pub(crate) fn append(
        &mut self,
        event: ManagedStateEvent,
    ) -> Result<EventOrdinal, StateEventStoreError> {
        let ordinal = EventOrdinal(self.events.len() as u64);
        if let Some(corrects) = event.corrects
            && corrects >= ordinal
        {
            return Err(StateEventStoreError::CorrectionTargetMissing { corrects });
        }
        self.events.push(RecordedStateEvent { ordinal, event });
        Ok(ordinal)
    }

    /// Append claiming an exact ordinal — the optimistic-concurrency
    /// entry point for replicating writers. A claimed ordinal the log
    /// already carries is an in-place update attempt; one past the end
    /// would leave a hole fabricating history. Both are the same
    /// conflict, and nothing changes on rejection.
    pub(crate) fn append_at(
        &mut self,
        claimed: EventOrdinal,
        event: ManagedStateEvent,
    ) -> Result<EventOrdinal, StateEventStoreError> {
        if claimed != EventOrdinal(self.events.len() as u64) {
            return Err(StateEventStoreError::RecordConflict { claimed });
        }
        self.append(event)
    }

    /// Validate a retention sweep request: delete every record with
    /// ordinal strictly below `delete_below`. The floor-protected span —
    /// the most recent [`RetentionFloor`] records, the whole log when it
    /// is shorter — is never sweepable; a request reaching into it fails
    /// closed. Planning never deletes: no delete path exists in E1.4 at
    /// all (execution with sealed export is V10.7.3/E6.6).
    pub(crate) fn plan_retention_sweep(
        &self,
        delete_below: EventOrdinal,
    ) -> Result<RetentionSweepPlan, StateEventStoreError> {
        let protected_from =
            EventOrdinal((self.events.len() as u64).saturating_sub(self.retention_floor.0));
        if delete_below > protected_from {
            return Err(StateEventStoreError::RetentionFloorViolation {
                delete_below,
                protected_from,
            });
        }
        Ok(RetentionSweepPlan {
            sweepable: (0..delete_below.0).map(EventOrdinal).collect(),
        })
    }

    /// The recorded events, in append order — the replay input.
    pub(crate) fn events(&self) -> &[RecordedStateEvent] {
        &self.events
    }
}

#[cfg(test)]
mod tests {
    //! E1.4.T1 exit tests (MILESTONES §E1.4; ADR-0057 invariant 2;
    //! RT-04): a state-only transition leaves version ID and
    //! `content_hash` unchanged and creates no new content version; the
    //! six §K4 dimensions are separate closed vocabularies.

    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;
    use crate::domain::graph::{
        GraphArtifactDocument, GraphKnowledgeObjectNode, GraphNode, GraphRelations,
        GraphRepositoryIdentity, GraphSourceSpan,
    };
    use crate::domain::managed::{ManagedWorkspace, WorkspaceId};

    fn workspace() -> ManagedWorkspace {
        ManagedWorkspace::new(WorkspaceId::new("ws-acme").expect("workspace id is non-blank"))
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

    fn artifact(nodes: Vec<GraphKnowledgeObjectNode>) -> GraphArtifactDocument {
        GraphArtifactDocument {
            schema_version: "adoc.graph.v6".to_string(),
            repository_identity: GraphRepositoryIdentity::local_project(
                "agentdoc.config.yaml".to_string(),
            ),
            nodes: nodes.into_iter().map(GraphNode::KnowledgeObject).collect(),
            edges: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn freshness_event(subject: StateEventSubject, state: FreshnessState) -> ManagedStateEvent {
        ManagedStateEvent {
            subject,
            change: ManagedStateChange::Freshness { state },
            emitter: EventEmitter::new("cloud.freshness_evaluator").expect("non-blank"),
            policy_version: PolicyVersion::new("freshness-policy-1").expect("non-blank"),
            corrects: None,
        }
    }

    /// Exit test (ADR-0057 invariant 2, the slice's headline acceptance):
    /// a state-only freshness `current` → `stale` transition on a managed
    /// version leaves the version ID and `content_hash` unchanged and
    /// creates no new content version — the whole workspace aggregate is
    /// byte-identical before and after the event.
    #[test]
    fn state_only_freshness_transition_leaves_versions_untouched() {
        let mut workspace = workspace();
        let outcome = workspace
            .import_artifact(&artifact(vec![knowledge_object(
                "billing.credits",
                "sha256:aaa",
            )]))
            .expect("import accepted");
        let imported = &outcome.imported[0];
        let subject = StateEventSubject {
            canonical: imported.canonical.clone(),
            version_id: imported.version_id.clone(),
        };
        let before = workspace.clone();

        let mut store = ManagedStateEventStore::new(RetentionFloor(1));
        store
            .append(freshness_event(subject.clone(), FreshnessState::Current))
            .expect("append accepted");
        store
            .append(freshness_event(subject.clone(), FreshnessState::Stale))
            .expect("append accepted");

        assert_eq!(
            workspace, before,
            "a state-only transition must not touch any version record"
        );
        let object = workspace
            .managed_object(&subject.canonical)
            .expect("object recorded");
        assert_eq!(object.versions.len(), 1, "no new content version");
        assert_eq!(object.versions[0].version_id, subject.version_id);
        assert_eq!(object.versions[0].content_hash, "sha256:aaa");
        assert_eq!(store.events().len(), 2);
        assert_eq!(store.events()[1].ordinal, EventOrdinal(1));
    }

    /// The serialized event record the Cloud cut consumes: ordinal,
    /// version-exact subject, family-tagged change, emitter, and policy
    /// version — and nothing else (no wall-clock anywhere).
    #[test]
    fn recorded_event_serialized_shape_is_pinned() {
        let mut workspace = workspace();
        let outcome = workspace
            .import_artifact(&artifact(vec![knowledge_object(
                "billing.credits",
                "sha256:aaa",
            )]))
            .expect("import accepted");
        let imported = &outcome.imported[0];
        let mut store = ManagedStateEventStore::new(RetentionFloor(1));
        store
            .append(freshness_event(
                StateEventSubject {
                    canonical: imported.canonical.clone(),
                    version_id: imported.version_id.clone(),
                },
                FreshnessState::Stale,
            ))
            .expect("append accepted");

        let value = serde_json::to_value(&store.events()[0]).expect("record serializes");
        assert_eq!(
            value,
            json!({
                "ordinal": 0,
                "event": {
                    "subject": {
                        "canonical": {
                            "workspace_id": "ws-acme",
                            "canonical_id": "mo-1"
                        },
                        "version_id": "mv-1"
                    },
                    "change": { "family": "freshness", "state": "stale" },
                    "emitter": "cloud.freshness_evaluator",
                    "policy_version": "freshness-policy-1",
                    "corrects": null
                }
            }),
            "recorded state event shape drifted"
        );
    }

    /// The serialized change shape of every dimension family, pinned:
    /// one `family` tag per §K4 dimension, each carrying only its own
    /// closed vocabulary; synchronization is per connector and carries
    /// `required_before_effective` on every event (§K4).
    #[test]
    fn dimension_family_serialized_shapes_are_pinned() {
        let cases = [
            (
                ManagedStateChange::Governance {
                    state: GovernanceState::Approved,
                },
                json!({ "family": "governance", "state": "approved" }),
            ),
            (
                ManagedStateChange::Verification {
                    state: VerificationState::PartiallyVerified,
                },
                json!({ "family": "verification", "state": "partially_verified" }),
            ),
            (
                ManagedStateChange::Effectivity {
                    state: EffectivityState::Suspended,
                },
                json!({ "family": "effectivity", "state": "suspended" }),
            ),
            (
                ManagedStateChange::Freshness {
                    state: FreshnessState::NeedsReview,
                },
                json!({ "family": "freshness", "state": "needs_review" }),
            ),
            (
                ManagedStateChange::Integrity {
                    state: IntegrityState::PotentiallyConflicting,
                },
                json!({ "family": "integrity", "state": "potentially_conflicting" }),
            ),
            (
                ManagedStateChange::Synchronization {
                    connector: ConnectorId::new("confluence").expect("non-blank"),
                    state: SynchronizationState::PendingWriteback,
                    required_before_effective: true,
                },
                json!({
                    "family": "synchronization",
                    "connector": "confluence",
                    "state": "pending_writeback",
                    "required_before_effective": true
                }),
            ),
        ];
        for (change, expected) in cases {
            assert_eq!(
                serde_json::to_value(&change).expect("serializes"),
                expected,
                "family shape drifted"
            );
        }
    }

    /// The six closed §K4 vocabularies, pinned value-for-value in order.
    /// `as_str` and the serde rendering must agree — the wire string is
    /// the single spelling everywhere.
    #[test]
    fn dimension_vocabularies_are_closed_and_wire_stable() {
        fn wire<S: Serialize>(state: &S) -> String {
            serde_json::to_value(state)
                .expect("serializes")
                .as_str()
                .expect("state serializes as a bare string")
                .to_string()
        }

        let governance: Vec<String> = GovernanceState::ALL.iter().map(wire).collect();
        assert_eq!(governance, ["proposed", "approved", "rejected", "revoked"]);
        let verification: Vec<String> = VerificationState::ALL.iter().map(wire).collect();
        assert_eq!(
            verification,
            ["unverified", "partially_verified", "verified", "failed"]
        );
        let effectivity: Vec<String> = EffectivityState::ALL.iter().map(wire).collect();
        assert_eq!(
            effectivity,
            ["pending", "scheduled", "effective", "suspended", "expired"]
        );
        let freshness: Vec<String> = FreshnessState::ALL.iter().map(wire).collect();
        assert_eq!(freshness, ["current", "needs_review", "stale"]);
        let integrity: Vec<String> = IntegrityState::ALL.iter().map(wire).collect();
        assert_eq!(
            integrity,
            ["clear", "potentially_conflicting", "contradicted"]
        );
        let synchronization: Vec<String> = SynchronizationState::ALL.iter().map(wire).collect();
        assert_eq!(
            synchronization,
            [
                "in_sync",
                "pending_writeback",
                "pending_external_approval",
                "writeback_failed",
                "source_ahead",
                "source_diverged",
                "paused",
                "not_applicable"
            ]
        );

        for state in GovernanceState::ALL {
            assert_eq!(wire(&state), state.as_str());
        }
        for state in VerificationState::ALL {
            assert_eq!(wire(&state), state.as_str());
        }
        for state in EffectivityState::ALL {
            assert_eq!(wire(&state), state.as_str());
        }
        for state in FreshnessState::ALL {
            assert_eq!(wire(&state), state.as_str());
        }
        for state in IntegrityState::ALL {
            assert_eq!(wire(&state), state.as_str());
        }
        for state in SynchronizationState::ALL {
            assert_eq!(wire(&state), state.as_str());
        }
    }

    // E1.4.T2 (MILESTONES §E1.4; V10.4.2 provenance): append-only is
    // enforced at the store layer, not by handler discipline. The store
    // type exposes no update or delete method at all; the adversarial
    // paths below are the only write-shaped requests besides append, and
    // both fail closed with registered wire codes.

    fn subject_for(workspace: &mut ManagedWorkspace) -> StateEventSubject {
        let outcome = workspace
            .import_artifact(&artifact(vec![knowledge_object(
                "billing.credits",
                "sha256:aaa",
            )]))
            .expect("import accepted");
        let imported = &outcome.imported[0];
        StateEventSubject {
            canonical: imported.canonical.clone(),
            version_id: imported.version_id.clone(),
        }
    }

    /// Acceptance (MILESTONES §E1.4): an in-place update attempt — a
    /// write claiming an ordinal the log already carries — yields
    /// `governance.record_conflict` and changes nothing. A claimed
    /// ordinal past the end (a hole that would fabricate history) is the
    /// same conflict.
    #[test]
    fn write_at_an_occupied_or_future_ordinal_is_a_record_conflict() {
        let mut workspace = workspace();
        let subject = subject_for(&mut workspace);
        let mut store = ManagedStateEventStore::new(RetentionFloor(1));
        store
            .append(freshness_event(subject.clone(), FreshnessState::Current))
            .expect("append accepted");
        let before = store.clone();

        for claimed in [EventOrdinal(0), EventOrdinal(2)] {
            let outcome = store.append_at(
                claimed,
                freshness_event(subject.clone(), FreshnessState::Stale),
            );
            let error = outcome.expect_err("occupied or future ordinal must be rejected");
            assert_eq!(error, StateEventStoreError::RecordConflict { claimed });
            assert_eq!(
                error.diagnostic_code().as_str(),
                "governance.record_conflict"
            );
        }
        assert_eq!(store, before, "a rejected write must change nothing");

        store
            .append_at(
                EventOrdinal(1),
                freshness_event(subject, FreshnessState::Stale),
            )
            .expect("the exact next ordinal appends");
        assert_eq!(store.events().len(), 2);
    }

    /// Corrections are new records referencing the corrected one — the
    /// corrected record itself stays byte-identical in the log.
    #[test]
    fn a_correction_appends_a_new_record_and_rewrites_nothing() {
        let mut workspace = workspace();
        let subject = subject_for(&mut workspace);
        let mut store = ManagedStateEventStore::new(RetentionFloor(1));
        store
            .append(freshness_event(subject.clone(), FreshnessState::Stale))
            .expect("append accepted");
        let original = store.events()[0].clone();

        let mut correction = freshness_event(subject, FreshnessState::Current);
        correction.corrects = Some(EventOrdinal(0));
        store.append(correction).expect("correction appends");

        assert_eq!(store.events().len(), 2);
        assert_eq!(
            store.events()[0],
            original,
            "the corrected record is never rewritten"
        );
        assert_eq!(store.events()[1].event.corrects, Some(EventOrdinal(0)));
    }

    /// A correction referencing a record the log does not carry
    /// contradicts recorded history and fails closed under the same
    /// registered conflict code.
    #[test]
    fn a_correction_referencing_an_unrecorded_ordinal_is_rejected() {
        let mut workspace = workspace();
        let subject = subject_for(&mut workspace);
        let mut store = ManagedStateEventStore::new(RetentionFloor(1));
        let mut correction = freshness_event(subject, FreshnessState::Current);
        correction.corrects = Some(EventOrdinal(3));

        let error = store
            .append(correction)
            .expect_err("a dangling correction must be rejected");
        assert_eq!(
            error,
            StateEventStoreError::CorrectionTargetMissing {
                corrects: EventOrdinal(3)
            }
        );
        assert_eq!(
            error.diagnostic_code().as_str(),
            "governance.record_conflict"
        );
        assert!(store.events().is_empty(), "nothing may be appended");
    }

    /// Adversarial acceptance (MILESTONES §E1.4): a retention sweep
    /// reaching one record into the floor-protected span — the
    /// "floor-minus-one-day" case in recorded-event-order terms — is
    /// rejected with `store.retention_floor_violation`. Sweeping is
    /// plan-only in E1.4 either way: deletion workflows above the floor
    /// are V10.7.3/E6.6, so no delete path exists at all yet.
    #[test]
    fn a_retention_sweep_into_the_protected_span_is_rejected() {
        let mut workspace = workspace();
        let subject = subject_for(&mut workspace);
        let mut store = ManagedStateEventStore::new(RetentionFloor(3));
        for _ in 0..5 {
            store
                .append(freshness_event(subject.clone(), FreshnessState::Stale))
                .expect("append accepted");
        }
        let before = store.clone();

        // Five records, floor 3: ordinals 2..=4 are protected. Asking to
        // delete everything below ordinal 3 reaches one record inside.
        let error = store
            .plan_retention_sweep(EventOrdinal(3))
            .expect_err("a sweep into the protected span must be rejected");
        assert_eq!(
            error,
            StateEventStoreError::RetentionFloorViolation {
                delete_below: EventOrdinal(3),
                protected_from: EventOrdinal(2),
            }
        );
        assert_eq!(
            error.diagnostic_code().as_str(),
            "store.retention_floor_violation"
        );

        let plan = store
            .plan_retention_sweep(EventOrdinal(2))
            .expect("a sweep below the protected span plans");
        assert_eq!(plan.sweepable, vec![EventOrdinal(0), EventOrdinal(1)]);
        assert_eq!(
            store, before,
            "planning deletes nothing — execution is a later slice (E6.6)"
        );
    }

    /// With fewer records than the floor requires retained, every sweep
    /// request is a violation — the floor never underflows into
    /// permitting deletion of a short log.
    #[test]
    fn a_short_log_is_entirely_floor_protected() {
        let mut workspace = workspace();
        let subject = subject_for(&mut workspace);
        let mut store = ManagedStateEventStore::new(RetentionFloor(3));
        store
            .append(freshness_event(subject, FreshnessState::Stale))
            .expect("append accepted");

        let error = store
            .plan_retention_sweep(EventOrdinal(1))
            .expect_err("the whole short log is protected");
        assert_eq!(
            error,
            StateEventStoreError::RetentionFloorViolation {
                delete_below: EventOrdinal(1),
                protected_from: EventOrdinal(0),
            }
        );
    }

    /// Blank or padded connector and emitter values fail closed — an
    /// unaddressable sync connector or an anonymous emitter must not be
    /// recordable (same posture as every E1.2 identity value).
    #[test]
    fn blank_or_padded_connector_and_emitter_are_rejected() {
        for invalid in ["", "  ", " confluence", "confluence "] {
            assert_eq!(
                ConnectorId::new(invalid),
                Err(ManagedStateEventError::InvalidConnector),
                "{invalid:?} must be rejected"
            );
            assert_eq!(
                EventEmitter::new(invalid),
                Err(ManagedStateEventError::InvalidEmitter),
                "{invalid:?} must be rejected"
            );
        }
    }
}
