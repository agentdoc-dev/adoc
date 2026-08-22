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

use std::collections::BTreeMap;

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

/// The recorded K9 replay posture (registered closed vocabulary,
/// E0.3.T4): can this derivation be reproduced later? A deletion/
/// tombstone event records the posture it leaves behind — deleting
/// retained evidence updates the posture by APPENDING, never by
/// rewriting governance history (K9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReplayPosture {
    FullyReplayable,
    SourceAccessRequired,
    IntentionallyNonReplayable,
    NoLongerReplayableAfterDeletion,
}

impl ReplayPosture {
    pub(crate) const ALL: [Self; 4] = [
        Self::FullyReplayable,
        Self::SourceAccessRequired,
        Self::IntentionallyNonReplayable,
        Self::NoLongerReplayableAfterDeletion,
    ];

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::FullyReplayable => "fully_replayable",
            Self::SourceAccessRequired => "source_access_required",
            Self::IntentionallyNonReplayable => "intentionally_non_replayable",
            Self::NoLongerReplayableAfterDeletion => "no_longer_replayable_after_deletion",
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

/// What one managed state event records, tagged by its event family —
/// all ten RT-04 families. One variant per §K4 dimension, each carrying
/// only that dimension's closed vocabulary — dimensions are never
/// conflated into a shared status field (D07/D15) — plus the four
/// families beyond the dimensions: an authorization-affecting source
/// change names the connector whose source ACL observation changed
/// (RT-04); a deletion/tombstone records the K9 replay posture it
/// leaves behind; declassification and migration are bare family
/// markers here — their policy mechanics are E6.2/E6.6, only the
/// append-only record shape is E1.4's.
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
    AuthorizationAffectingSourceChange {
        connector: ConnectorId,
    },
    Declassification,
    Migration,
    DeletionTombstone {
        replay_posture: ReplayPosture,
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

/// One event as recorded: the store-assigned ordinal, the event, and —
/// stored at write time, never re-derived (E1.4.T5) — the event's exact
/// canonical bytes plus the chained digest. Later export needs no extra
/// data: the `(ordinal, event_bytes, digest)` triples alone reproduce
/// every event and prove the log's order and integrity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct RecordedStateEvent {
    pub(crate) ordinal: EventOrdinal,
    pub(crate) event: ManagedStateEvent,
    /// The exact canonical JSON of `event` as serialized at write time.
    pub(crate) event_bytes: String,
    /// `sha256:`-prefixed digest over the previous record's digest (the
    /// empty string for ordinal 0) concatenated with `event_bytes` —
    /// each record binds the whole prefix.
    pub(crate) digest: String,
}

/// One dimension of reconstructed managed state. [`RecordedDimension::Gap`]
/// is the explicit marker for "no event recorded for this dimension at
/// this T" — every record predating the introduction of an emitter for a
/// dimension renders this gap, never an inferred, backfilled, or default
/// state (E1.4.T3; RT-04). On the wire the marker is the literal string
/// `"gap"`, unmistakable for any state value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RecordedDimension<S> {
    Gap,
    Recorded(S),
}

/// One connector's reconstructed synchronization state (§K4: always per
/// connector, with the connector's `required_before_effective`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct ConnectorSyncRecord {
    pub(crate) state: SynchronizationState,
    pub(crate) required_before_effective: bool,
}

/// The reconstructed managed state of one immutable content version: the
/// six §K4 dimensions, each independently recorded or an explicit gap.
/// Derived — reproducible from the event log at any T and never
/// authoritative (ADR-0057 invariant 2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ManagedVersionState {
    pub(crate) governance: RecordedDimension<GovernanceState>,
    pub(crate) verification: RecordedDimension<VerificationState>,
    pub(crate) effectivity: RecordedDimension<EffectivityState>,
    pub(crate) freshness: RecordedDimension<FreshnessState>,
    pub(crate) integrity: RecordedDimension<IntegrityState>,
    pub(crate) synchronization: BTreeMap<ConnectorId, ConnectorSyncRecord>,
}

impl ManagedVersionState {
    fn all_gaps() -> Self {
        Self {
            governance: RecordedDimension::Gap,
            verification: RecordedDimension::Gap,
            effectivity: RecordedDimension::Gap,
            freshness: RecordedDimension::Gap,
            integrity: RecordedDimension::Gap,
            synchronization: BTreeMap::new(),
        }
    }

    /// Apply one recorded change: set exactly the dimension it names.
    /// No change ever touches another dimension — conflation is
    /// unrepresentable (D07/D15).
    fn apply(&mut self, change: &ManagedStateChange) {
        match change {
            ManagedStateChange::Governance { state } => {
                self.governance = RecordedDimension::Recorded(*state);
            }
            ManagedStateChange::Verification { state } => {
                self.verification = RecordedDimension::Recorded(*state);
            }
            ManagedStateChange::Effectivity { state } => {
                self.effectivity = RecordedDimension::Recorded(*state);
            }
            ManagedStateChange::Freshness { state } => {
                self.freshness = RecordedDimension::Recorded(*state);
            }
            ManagedStateChange::Integrity { state } => {
                self.integrity = RecordedDimension::Recorded(*state);
            }
            ManagedStateChange::Synchronization {
                connector,
                state,
                required_before_effective,
            } => {
                self.synchronization.insert(
                    connector.clone(),
                    ConnectorSyncRecord {
                        state: *state,
                        required_before_effective: *required_before_effective,
                    },
                );
            }
            // The four RT-04 families beyond the §K4 dimensions are
            // recorded in the log but advance no dimension: what state
            // they imply is policy (declassification E6.2, deletion
            // execution E6.6, ACL re-evaluation E2.2) — inferring a
            // transition here would fabricate history (RT-04).
            ManagedStateChange::AuthorizationAffectingSourceChange { .. }
            | ManagedStateChange::Declassification
            | ManagedStateChange::Migration
            | ManagedStateChange::DeletionTombstone { .. } => {}
        }
    }
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
    #[error("the audit sink failed to persist the transition's audit record")]
    AuditPersistenceFailed,
    #[error("the state event could not be canonically serialized for its audit record")]
    EventSerializationUnavailable,
}

impl StateEventStoreError {
    /// The registered wire code for this rejection. Both conflict shapes
    /// — an occupied/future-ordinal write and a correction naming an
    /// unrecorded target — are appends contradicting recorded history,
    /// the `governance.record_conflict` family (V10.4.2).
    /// `audit.persistence_failed` is the operation-level code (E1.4);
    /// the gate-level `gate.audit_persistence_failed` (E5.3) is a
    /// distinct registered surface that consumes it — explicit mapping,
    /// never spelling drift (RT-21).
    pub(crate) fn diagnostic_code(&self) -> DiagnosticCode {
        match self {
            Self::RecordConflict { .. } | Self::CorrectionTargetMissing { .. } => {
                DiagnosticCode::GovernanceRecordConflict
            }
            Self::RetentionFloorViolation { .. } => DiagnosticCode::StoreRetentionFloorViolation,
            // Exact bytes at write time are part of the audit trail
            // (E1.4.T5): an event whose canonical bytes cannot be
            // produced fails exactly like a sink that cannot persist
            // them — the owning operation never succeeds unaudited.
            Self::AuditPersistenceFailed | Self::EventSerializationUnavailable => {
                DiagnosticCode::AuditPersistenceFailed
            }
        }
    }
}

/// One audit record, written by the store at append time — one per
/// recorded transition, carrying the exact event bytes and the chained
/// digest (E1.4.T5) so the full lifecycle reconstructs from audit rows
/// alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuditRecord {
    pub(crate) ordinal: EventOrdinal,
    /// Same exact bytes as [`RecordedStateEvent::event_bytes`].
    pub(crate) event_bytes: String,
    /// Same chained digest as [`RecordedStateEvent::digest`].
    pub(crate) digest: String,
}

/// The audit sink could not persist a record. Deliberately carries no
/// recovery hint: the owning operation must fail — under every V10.4.1
/// policy cell the failure is surfaced (blocking where policy blocks,
/// loudly annotated under the advisory default via the returned code),
/// and nothing ever silently succeeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("audit record persistence failed")]
pub(crate) struct AuditSinkError;

/// Internal port: where transition audit records are persisted. The
/// store writes the audit record BEFORE committing the event — a sink
/// failure aborts the append entirely, so an unaudited transition can
/// never exist (fail closed, V10.4.6).
pub(crate) trait AuditSink {
    fn record(&mut self, record: &AuditRecord) -> Result<(), AuditSinkError>;
}

/// One audited transition: a dimension, a target state, and the
/// emitting operation wired to audit it. Keyed by target state because
/// every recorded event lands on exactly one state — a dimension's
/// transition set is covered exactly when every reachable target state
/// is audited (edge-level narrowing is policy, E6.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuditedTransition {
    pub(crate) dimension: &'static str,
    pub(crate) state: &'static str,
    pub(crate) emitter: &'static str,
}

/// The audit emitter registry (E1.4.T4): the hand-maintained wiring
/// table the CI coverage guard diffs against the state machines. A new
/// vocabulary state fails the guard until a row lands here — auditing
/// is wired consciously, never assumed. Today every transition is
/// audited by the single append path, which writes the audit record
/// before committing the event.
const APPEND_EMITTER: &str = "managed_state_event_store.append";
pub(crate) const AUDIT_EMITTER_REGISTRY: &[AuditedTransition] = &{
    const fn wired(dimension: &'static str, state: &'static str) -> AuditedTransition {
        AuditedTransition {
            dimension,
            state,
            emitter: APPEND_EMITTER,
        }
    }
    [
        wired("governance", "proposed"),
        wired("governance", "approved"),
        wired("governance", "rejected"),
        wired("governance", "revoked"),
        wired("verification", "unverified"),
        wired("verification", "partially_verified"),
        wired("verification", "verified"),
        wired("verification", "failed"),
        wired("effectivity", "pending"),
        wired("effectivity", "scheduled"),
        wired("effectivity", "effective"),
        wired("effectivity", "suspended"),
        wired("effectivity", "expired"),
        wired("freshness", "current"),
        wired("freshness", "needs_review"),
        wired("freshness", "stale"),
        wired("integrity", "clear"),
        wired("integrity", "potentially_conflicting"),
        wired("integrity", "contradicted"),
        wired("synchronization", "in_sync"),
        wired("synchronization", "pending_writeback"),
        wired("synchronization", "pending_external_approval"),
        wired("synchronization", "writeback_failed"),
        wired("synchronization", "source_ahead"),
        wired("synchronization", "source_diverged"),
        wired("synchronization", "paused"),
        wired("synchronization", "not_applicable"),
    ]
};

/// The two directions the audit coverage guard checks: transitions the
/// registry does not audit, and registry entries naming no known
/// transition (stale or misspelled rows must not pass silently).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuditCoverageDiff {
    pub(crate) unaudited: Vec<(String, String)>,
    pub(crate) unknown: Vec<(String, String)>,
}

/// Diff the six state machines' transition sets — every (dimension,
/// target state) pair derivable from the closed vocabularies — against
/// an audit emitter registry. Pure so the guard can also prove that a
/// deliberately unwired fixture fails.
pub(crate) fn audit_coverage_diff(registry: &[AuditedTransition]) -> AuditCoverageDiff {
    let mut complete: Vec<(String, String)> = Vec::new();
    complete.extend(
        GovernanceState::ALL
            .iter()
            .map(|s| ("governance".to_string(), s.as_str().to_string())),
    );
    complete.extend(
        VerificationState::ALL
            .iter()
            .map(|s| ("verification".to_string(), s.as_str().to_string())),
    );
    complete.extend(
        EffectivityState::ALL
            .iter()
            .map(|s| ("effectivity".to_string(), s.as_str().to_string())),
    );
    complete.extend(
        FreshnessState::ALL
            .iter()
            .map(|s| ("freshness".to_string(), s.as_str().to_string())),
    );
    complete.extend(
        IntegrityState::ALL
            .iter()
            .map(|s| ("integrity".to_string(), s.as_str().to_string())),
    );
    complete.extend(
        SynchronizationState::ALL
            .iter()
            .map(|s| ("synchronization".to_string(), s.as_str().to_string())),
    );

    let registered: Vec<(String, String)> = registry
        .iter()
        .map(|entry| (entry.dimension.to_string(), entry.state.to_string()))
        .collect();
    AuditCoverageDiff {
        unaudited: complete
            .iter()
            .filter(|pair| !registered.contains(pair))
            .cloned()
            .collect(),
        unknown: registered
            .iter()
            .filter(|pair| !complete.contains(pair))
            .cloned()
            .collect(),
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
    /// The derived current-state cache, maintained incrementally on
    /// append. Allowed but NEVER authoritative (ADR-0057 invariant 2):
    /// [`ManagedStateEventStore::reconstruct_through`] reproduces it
    /// from the recorded events alone, and the domain tests pin the two
    /// equal after every replay.
    current: BTreeMap<StateEventSubject, ManagedVersionState>,
}

impl ManagedStateEventStore {
    pub(crate) fn new(retention_floor: RetentionFloor) -> Self {
        Self {
            events: Vec::new(),
            retention_floor,
            current: BTreeMap::new(),
        }
    }

    /// Append one state event, assigning the next ordinal. Never touches
    /// any managed version record: a state-only transition leaves every
    /// version ID and `content_hash` unchanged and creates no content
    /// version (RT-04) — this store has no reference to the workspace's
    /// version records at all, so the invariant holds by construction.
    /// A correction must reference a recorded ordinal; a dangling one
    /// fails closed and appends nothing.
    ///
    /// The transition's audit record is written to `sink` BEFORE the
    /// event commits: a sink failure aborts the whole append with
    /// `audit.persistence_failed` — the owning operation surfaces the
    /// failure under every V10.4.1 policy cell, and an unaudited
    /// committed transition can never exist (never fail-open, V10.4.6).
    pub(crate) fn append(
        &mut self,
        sink: &mut dyn AuditSink,
        event: ManagedStateEvent,
    ) -> Result<EventOrdinal, StateEventStoreError> {
        let ordinal = EventOrdinal(self.events.len() as u64);
        if let Some(corrects) = event.corrects
            && corrects >= ordinal
        {
            return Err(StateEventStoreError::CorrectionTargetMissing { corrects });
        }
        // Exact bytes + chained digest, produced at write time (E1.4.T5)
        // — never re-derived later, so export needs no extra data.
        let event_bytes = serde_json::to_string(&event)
            .map_err(|_| StateEventStoreError::EventSerializationUnavailable)?;
        let previous_digest = self.events.last().map(|r| r.digest.as_str()).unwrap_or("");
        let digest =
            super::hashing::sha256_prefixed(format!("{previous_digest}{event_bytes}").as_bytes());
        if sink
            .record(&AuditRecord {
                ordinal,
                event_bytes: event_bytes.clone(),
                digest: digest.clone(),
            })
            .is_err()
        {
            return Err(StateEventStoreError::AuditPersistenceFailed);
        }
        self.current
            .entry(event.subject.clone())
            .or_insert_with(ManagedVersionState::all_gaps)
            .apply(&event.change);
        self.events.push(RecordedStateEvent {
            ordinal,
            event,
            event_bytes,
            digest,
        });
        Ok(ordinal)
    }

    /// Append claiming an exact ordinal — the optimistic-concurrency
    /// entry point for replicating writers. A claimed ordinal the log
    /// already carries is an in-place update attempt; one past the end
    /// would leave a hole fabricating history. Both are the same
    /// conflict, and nothing changes on rejection.
    pub(crate) fn append_at(
        &mut self,
        sink: &mut dyn AuditSink,
        claimed: EventOrdinal,
        event: ManagedStateEvent,
    ) -> Result<EventOrdinal, StateEventStoreError> {
        if claimed != EventOrdinal(self.events.len() as u64) {
            return Err(StateEventStoreError::RecordConflict { claimed });
        }
        self.append(sink, event)
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

    /// The derived current-state cache. Never authoritative — always
    /// reproducible via [`ManagedStateEventStore::reconstruct_through`].
    pub(crate) fn current_state(&self) -> &BTreeMap<StateEventSubject, ManagedVersionState> {
        &self.current
    }

    /// Historical current-state at time T, where T is an event ordinal:
    /// replay the recorded events `0..=through`, in order, from the
    /// immutable records alone. A correction applies at its own position
    /// — history before it stands unrewritten, and no transition is ever
    /// inferred or backfilled (RT-04). A T past the last recorded event
    /// replays everything: nothing after the log's end exists to change
    /// the answer.
    pub(crate) fn reconstruct_through(
        &self,
        through: EventOrdinal,
    ) -> BTreeMap<StateEventSubject, ManagedVersionState> {
        let mut state: BTreeMap<StateEventSubject, ManagedVersionState> = BTreeMap::new();
        for recorded in self.events.iter().filter(|r| r.ordinal <= through) {
            state
                .entry(recorded.event.subject.clone())
                .or_insert_with(ManagedVersionState::all_gaps)
                .apply(&recorded.event.change);
        }
        state
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

    /// The reference audit sink for domain tests: persists every record
    /// in memory, in write order. Production sinks are adapter concerns
    /// (the Cloud store, E1.4 cloud cut).
    #[derive(Debug, Default)]
    struct InMemoryAuditSink {
        records: Vec<AuditRecord>,
    }

    impl AuditSink for InMemoryAuditSink {
        fn record(&mut self, record: &AuditRecord) -> Result<(), AuditSinkError> {
            self.records.push(record.clone());
            Ok(())
        }
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

        let mut sink = InMemoryAuditSink::default();
        let mut store = ManagedStateEventStore::new(RetentionFloor(1));
        store
            .append(
                &mut sink,
                freshness_event(subject.clone(), FreshnessState::Current),
            )
            .expect("append accepted");
        store
            .append(
                &mut sink,
                freshness_event(subject.clone(), FreshnessState::Stale),
            )
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
    /// version-exact subject, family-tagged change, emitter, policy
    /// version, exact bytes, and chained digest — and nothing else (no
    /// wall-clock anywhere).
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
        let mut sink = InMemoryAuditSink::default();
        let mut store = ManagedStateEventStore::new(RetentionFloor(1));
        store
            .append(
                &mut sink,
                freshness_event(
                    StateEventSubject {
                        canonical: imported.canonical.clone(),
                        version_id: imported.version_id.clone(),
                    },
                    FreshnessState::Stale,
                ),
            )
            .expect("append accepted");

        let record = &store.events()[0];
        let value = serde_json::to_value(record).expect("record serializes");
        let expected_event = json!({
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
        });
        assert_eq!(
            value,
            json!({
                "ordinal": 0,
                "event": expected_event,
                "event_bytes": record.event_bytes,
                "digest": record.digest,
            }),
            "recorded state event shape drifted"
        );
        // The stored bytes are the event's canonical serialization; the
        // first digest chains from the empty string.
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&record.event_bytes).expect("bytes parse"),
            expected_event
        );
        assert_eq!(
            record.digest,
            crate::domain::hashing::sha256_prefixed(record.event_bytes.as_bytes())
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
        let mut sink = InMemoryAuditSink::default();
        let mut store = ManagedStateEventStore::new(RetentionFloor(1));
        store
            .append(
                &mut sink,
                freshness_event(subject.clone(), FreshnessState::Current),
            )
            .expect("append accepted");
        let before = store.clone();

        for claimed in [EventOrdinal(0), EventOrdinal(2)] {
            let outcome = store.append_at(
                &mut sink,
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
                &mut sink,
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
        let mut sink = InMemoryAuditSink::default();
        let mut store = ManagedStateEventStore::new(RetentionFloor(1));
        store
            .append(
                &mut sink,
                freshness_event(subject.clone(), FreshnessState::Stale),
            )
            .expect("append accepted");
        let original = store.events()[0].clone();

        let mut correction = freshness_event(subject, FreshnessState::Current);
        correction.corrects = Some(EventOrdinal(0));
        store
            .append(&mut sink, correction)
            .expect("correction appends");

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
        let mut sink = InMemoryAuditSink::default();
        let mut store = ManagedStateEventStore::new(RetentionFloor(1));
        let mut correction = freshness_event(subject, FreshnessState::Current);
        correction.corrects = Some(EventOrdinal(3));

        let error = store
            .append(&mut sink, correction)
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
        let mut sink = InMemoryAuditSink::default();
        let mut store = ManagedStateEventStore::new(RetentionFloor(3));
        for _ in 0..5 {
            store
                .append(
                    &mut sink,
                    freshness_event(subject.clone(), FreshnessState::Stale),
                )
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
        let mut sink = InMemoryAuditSink::default();
        let mut store = ManagedStateEventStore::new(RetentionFloor(3));
        store
            .append(&mut sink, freshness_event(subject, FreshnessState::Stale))
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

    // E1.4.T3 (MILESTONES §E1.4; ADR-0057 invariant 2): derived caches
    // are allowed but never authoritative — historical current-state at
    // time T reconstructs from the immutable versions, the recorded
    // state events, and their recorded policy versions alone, where T is
    // an event ordinal (no wall-clock exists in domain records).

    fn event(
        subject: &StateEventSubject,
        change: ManagedStateChange,
        emitter: &str,
        policy: &str,
    ) -> ManagedStateEvent {
        ManagedStateEvent {
            subject: subject.clone(),
            change,
            emitter: EventEmitter::new(emitter).expect("non-blank"),
            policy_version: PolicyVersion::new(policy).expect("non-blank"),
            corrects: None,
        }
    }

    /// Exit test: replaying the full event log yields exactly the
    /// derived current-state cache — the cache is reproducible, so it
    /// can never drift into being the authority.
    #[test]
    fn full_replay_matches_the_derived_cache() {
        let mut workspace = workspace();
        let subject = subject_for(&mut workspace);
        let mut sink = InMemoryAuditSink::default();
        let mut store = ManagedStateEventStore::new(RetentionFloor(1));
        let connector = ConnectorId::new("confluence").expect("non-blank");
        for change in [
            ManagedStateChange::Governance {
                state: GovernanceState::Proposed,
            },
            ManagedStateChange::Verification {
                state: VerificationState::PartiallyVerified,
            },
            ManagedStateChange::Governance {
                state: GovernanceState::Approved,
            },
            ManagedStateChange::Effectivity {
                state: EffectivityState::Effective,
            },
            ManagedStateChange::Synchronization {
                connector: connector.clone(),
                state: SynchronizationState::PendingWriteback,
                required_before_effective: false,
            },
            ManagedStateChange::Freshness {
                state: FreshnessState::Stale,
            },
        ] {
            store
                .append(
                    &mut sink,
                    event(&subject, change, "cloud.governance", "policy-1"),
                )
                .expect("append accepted");
        }

        assert_eq!(
            store.reconstruct_through(EventOrdinal(5)),
            *store.current_state(),
            "full replay must reproduce the derived cache exactly"
        );
    }

    /// Historical current-state at time T: replay through a mid-log
    /// ordinal renders the state exactly as it was recorded then —
    /// later events are invisible, earlier ones stand.
    #[test]
    fn historical_state_at_an_event_ordinal_reconstructs_exactly() {
        let mut workspace = workspace();
        let subject = subject_for(&mut workspace);
        let mut sink = InMemoryAuditSink::default();
        let mut store = ManagedStateEventStore::new(RetentionFloor(1));
        store
            .append(
                &mut sink,
                event(
                    &subject,
                    ManagedStateChange::Governance {
                        state: GovernanceState::Proposed,
                    },
                    "cloud.governance",
                    "policy-1",
                ),
            )
            .expect("append accepted");
        store
            .append(
                &mut sink,
                event(
                    &subject,
                    ManagedStateChange::Freshness {
                        state: FreshnessState::Stale,
                    },
                    "cloud.freshness_evaluator",
                    "policy-1",
                ),
            )
            .expect("append accepted");
        store
            .append(
                &mut sink,
                event(
                    &subject,
                    ManagedStateChange::Governance {
                        state: GovernanceState::Approved,
                    },
                    "cloud.governance",
                    "policy-2",
                ),
            )
            .expect("append accepted");

        let at_one = store.reconstruct_through(EventOrdinal(1));
        let state = &at_one[&subject];
        assert_eq!(
            state.governance,
            RecordedDimension::Recorded(GovernanceState::Proposed),
            "the ordinal-2 approval must be invisible at T=1"
        );
        assert_eq!(
            state.freshness,
            RecordedDimension::Recorded(FreshnessState::Stale)
        );

        let now = store.reconstruct_through(EventOrdinal(2));
        assert_eq!(
            now[&subject].governance,
            RecordedDimension::Recorded(GovernanceState::Approved)
        );
    }

    /// Gap markers: a dimension with no recorded event — for example
    /// every record predating the introduction of an emitter for that
    /// dimension — renders an explicit gap, never an inferred or
    /// backfilled transition and never a default state.
    #[test]
    fn unrecorded_dimensions_render_explicit_gap_markers() {
        let mut workspace = workspace();
        let subject = subject_for(&mut workspace);
        let mut sink = InMemoryAuditSink::default();
        let mut store = ManagedStateEventStore::new(RetentionFloor(1));
        store
            .append(
                &mut sink,
                event(
                    &subject,
                    ManagedStateChange::Freshness {
                        state: FreshnessState::Current,
                    },
                    "cloud.freshness_evaluator",
                    "policy-1",
                ),
            )
            .expect("append accepted");

        let state = &store.current_state()[&subject];
        assert_eq!(state.governance, RecordedDimension::Gap);
        assert_eq!(state.verification, RecordedDimension::Gap);
        assert_eq!(state.effectivity, RecordedDimension::Gap);
        assert_eq!(state.integrity, RecordedDimension::Gap);
        assert!(state.synchronization.is_empty());
        assert_eq!(
            state.freshness,
            RecordedDimension::Recorded(FreshnessState::Current)
        );

        // The gap marker is explicit on the wire too — a consumer can
        // never mistake "no event recorded" for a state value.
        assert_eq!(
            serde_json::to_value(state.governance).expect("serializes"),
            json!("gap")
        );
        assert_eq!(
            serde_json::to_value(state.freshness).expect("serializes"),
            json!({ "recorded": "current" })
        );
    }

    /// A correction applies at its own position in the recorded order —
    /// reconstruction at a T before the correction still shows the
    /// corrected record's state, because history is never rewritten and
    /// transitions are never backfilled (RT-04).
    #[test]
    fn a_correction_applies_at_its_own_ordinal_never_retroactively() {
        let mut workspace = workspace();
        let subject = subject_for(&mut workspace);
        let mut sink = InMemoryAuditSink::default();
        let mut store = ManagedStateEventStore::new(RetentionFloor(1));
        store
            .append(
                &mut sink,
                event(
                    &subject,
                    ManagedStateChange::Freshness {
                        state: FreshnessState::Stale,
                    },
                    "cloud.freshness_evaluator",
                    "policy-1",
                ),
            )
            .expect("append accepted");
        let mut correction = event(
            &subject,
            ManagedStateChange::Freshness {
                state: FreshnessState::Current,
            },
            "cloud.freshness_evaluator",
            "policy-1",
        );
        correction.corrects = Some(EventOrdinal(0));
        store
            .append(&mut sink, correction)
            .expect("correction appends");

        assert_eq!(
            store.reconstruct_through(EventOrdinal(0))[&subject].freshness,
            RecordedDimension::Recorded(FreshnessState::Stale),
            "history at T=0 keeps the uncorrected record — never rewritten"
        );
        assert_eq!(
            store.current_state()[&subject].freshness,
            RecordedDimension::Recorded(FreshnessState::Current)
        );
    }

    /// Per-connector synchronization reconstructs per connector: two
    /// connectors' states never collapse into one, and each carries its
    /// own `required_before_effective`.
    #[test]
    fn synchronization_reconstructs_per_connector() {
        let mut workspace = workspace();
        let subject = subject_for(&mut workspace);
        let mut sink = InMemoryAuditSink::default();
        let mut store = ManagedStateEventStore::new(RetentionFloor(1));
        for (connector, state, required) in [
            ("confluence", SynchronizationState::InSync, true),
            ("github", SynchronizationState::SourceDiverged, false),
        ] {
            store
                .append(
                    &mut sink,
                    event(
                        &subject,
                        ManagedStateChange::Synchronization {
                            connector: ConnectorId::new(connector).expect("non-blank"),
                            state,
                            required_before_effective: required,
                        },
                        "cloud.sync_observer",
                        "policy-1",
                    ),
                )
                .expect("append accepted");
        }

        let sync = &store.current_state()[&subject].synchronization;
        assert_eq!(sync.len(), 2);
        let confluence = &sync[&ConnectorId::new("confluence").expect("non-blank")];
        assert_eq!(confluence.state, SynchronizationState::InSync);
        assert!(confluence.required_before_effective);
        let github = &sync[&ConnectorId::new("github").expect("non-blank")];
        assert_eq!(github.state, SynchronizationState::SourceDiverged);
        assert!(!github.required_before_effective);
    }

    // E1.4.T4 (MILESTONES §E1.4; V10.4.6 provenance): every state
    // transition is audited, the coverage guard fails the build on any
    // unaudited one, and a failing audit sink can never fail open.

    /// An audit sink whose persistence always fails — the forced-failure
    /// fixture of the milestone's acceptance.
    struct FailingAuditSink;

    impl AuditSink for FailingAuditSink {
        fn record(&mut self, _record: &AuditRecord) -> Result<(), AuditSinkError> {
            Err(AuditSinkError)
        }
    }

    /// Acceptance: forced audit-sink failure surfaces
    /// `audit.persistence_failed` from the owning operation and nothing
    /// silently succeeds — the event is not appended, the derived cache
    /// is untouched, and no partial record exists anywhere.
    #[test]
    fn forced_audit_sink_failure_fails_the_append_closed() {
        let mut workspace = workspace();
        let subject = subject_for(&mut workspace);
        let mut sink = InMemoryAuditSink::default();
        let mut store = ManagedStateEventStore::new(RetentionFloor(1));

        let error = store
            .append(
                &mut FailingAuditSink,
                freshness_event(subject.clone(), FreshnessState::Stale),
            )
            .expect_err("a failed audit write must fail the owning operation");
        assert_eq!(error, StateEventStoreError::AuditPersistenceFailed);
        assert_eq!(error.diagnostic_code().as_str(), "audit.persistence_failed");
        assert!(store.events().is_empty(), "nothing may append");
        assert!(store.current_state().is_empty(), "no cache entry may exist");

        // The same event persists once the sink recovers — the failure
        // was the sink's, not the event's.
        store
            .append(&mut sink, freshness_event(subject, FreshnessState::Stale))
            .expect("append accepted");
        assert_eq!(store.events().len(), 1);
        assert_eq!(sink.records.len(), 1);
        assert_eq!(sink.records[0].ordinal, EventOrdinal(0));
    }

    /// The audit coverage guard (CI): diff the six state machines'
    /// transition sets — every reachable target state per dimension,
    /// since each recorded event lands on exactly one — against the
    /// audit emitter registry. The complete set passes.
    #[test]
    fn audit_emitter_registry_covers_every_transition() {
        let diff = audit_coverage_diff(AUDIT_EMITTER_REGISTRY);
        assert_eq!(
            diff.unaudited,
            Vec::<(String, String)>::new(),
            "unaudited transitions — extend AUDIT_EMITTER_REGISTRY and wire the emitter"
        );
        assert_eq!(
            diff.unknown,
            Vec::<(String, String)>::new(),
            "registry entries naming no known transition — stale or misspelled"
        );
        for entry in AUDIT_EMITTER_REGISTRY {
            assert!(
                !entry.emitter.trim().is_empty(),
                "every audited transition names its emitter"
            );
        }
    }

    /// Acceptance: a deliberately unwired fixture transition fails the
    /// coverage guard — dropping one entry surfaces exactly that
    /// transition as unaudited, and a fixture entry naming a state the
    /// vocabularies do not carry is flagged as unknown.
    #[test]
    fn an_unwired_or_unknown_fixture_transition_fails_the_coverage_guard() {
        let unwired: Vec<AuditedTransition> = AUDIT_EMITTER_REGISTRY
            .iter()
            .filter(|entry| !(entry.dimension == "freshness" && entry.state == "stale"))
            .cloned()
            .collect();
        let diff = audit_coverage_diff(&unwired);
        assert_eq!(
            diff.unaudited,
            vec![("freshness".to_string(), "stale".to_string())],
            "the dropped transition must surface as unaudited"
        );

        let mut with_unknown: Vec<AuditedTransition> = AUDIT_EMITTER_REGISTRY.to_vec();
        with_unknown.push(AuditedTransition {
            dimension: "freshness",
            state: "fresh",
            emitter: "managed_state_event_store.append",
        });
        let diff = audit_coverage_diff(&with_unknown);
        assert_eq!(
            diff.unknown,
            vec![("freshness".to_string(), "fresh".to_string())],
            "an entry outside the closed vocabularies must be flagged"
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

    // E1.4.T5 (MILESTONES §E1.4; RT-04; K9): the remaining event
    // families, plus exact bytes and the digest chain stored at write
    // time — later export needs no extra data.

    /// The serialized shapes of the four RT-04 families beyond the six
    /// §K4 dimensions, pinned for the Cloud cut: authorization-affecting
    /// source changes name their connector, deletion/tombstone records
    /// the K9 replay posture it leaves behind, declassification and
    /// migration are bare family markers (their policy mechanics are
    /// E6.2/E6.6 — the append-only record shape is fixed here).
    #[test]
    fn remaining_event_family_serialized_shapes_are_pinned() {
        let cases = [
            (
                ManagedStateChange::AuthorizationAffectingSourceChange {
                    connector: ConnectorId::new("confluence").expect("non-blank"),
                },
                json!({
                    "family": "authorization_affecting_source_change",
                    "connector": "confluence"
                }),
            ),
            (
                ManagedStateChange::Declassification,
                json!({ "family": "declassification" }),
            ),
            (
                ManagedStateChange::Migration,
                json!({ "family": "migration" }),
            ),
            (
                ManagedStateChange::DeletionTombstone {
                    replay_posture: ReplayPosture::NoLongerReplayableAfterDeletion,
                },
                json!({
                    "family": "deletion_tombstone",
                    "replay_posture": "no_longer_replayable_after_deletion"
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

    /// The registered K9 replay-posture vocabulary (E0.3.T4), pinned
    /// value-for-value against the domain enum — the wire string is the
    /// single spelling everywhere.
    #[test]
    fn replay_posture_vocabulary_is_closed_and_wire_stable() {
        let wire: Vec<String> = ReplayPosture::ALL
            .iter()
            .map(|posture| {
                serde_json::to_value(posture)
                    .expect("serializes")
                    .as_str()
                    .expect("posture serializes as a bare string")
                    .to_string()
            })
            .collect();
        assert_eq!(
            wire,
            [
                "fully_replayable",
                "source_access_required",
                "intentionally_non_replayable",
                "no_longer_replayable_after_deletion"
            ]
        );
        for posture in ReplayPosture::ALL {
            assert_eq!(
                serde_json::to_value(posture).expect("serializes"),
                json!(posture.as_str())
            );
        }
    }

    /// The four non-dimension families are recorded in the log but
    /// advance no §K4 dimension — a deletion/tombstone APPENDS (K9:
    /// governance history is never rewritten), and none of them touches
    /// a version record.
    #[test]
    fn non_dimension_families_advance_no_state_dimension() {
        let mut workspace = workspace();
        let subject = subject_for(&mut workspace);
        let before = workspace.clone();
        let mut sink = InMemoryAuditSink::default();
        let mut store = ManagedStateEventStore::new(RetentionFloor(1));
        let changes = [
            ManagedStateChange::AuthorizationAffectingSourceChange {
                connector: ConnectorId::new("confluence").expect("non-blank"),
            },
            ManagedStateChange::Declassification,
            ManagedStateChange::Migration,
            ManagedStateChange::DeletionTombstone {
                replay_posture: ReplayPosture::NoLongerReplayableAfterDeletion,
            },
        ];
        for change in changes {
            store
                .append(
                    &mut sink,
                    ManagedStateEvent {
                        subject: subject.clone(),
                        change,
                        emitter: EventEmitter::new("cloud.governance").expect("non-blank"),
                        policy_version: PolicyVersion::new("policy-1").expect("non-blank"),
                        corrects: None,
                    },
                )
                .expect("append accepted");
        }

        assert_eq!(store.events().len(), 4, "every family appends a record");
        let state = &store.current_state()[&subject];
        assert_eq!(state.governance, RecordedDimension::Gap);
        assert_eq!(state.verification, RecordedDimension::Gap);
        assert_eq!(state.effectivity, RecordedDimension::Gap);
        assert_eq!(state.freshness, RecordedDimension::Gap);
        assert_eq!(state.integrity, RecordedDimension::Gap);
        assert!(state.synchronization.is_empty());
        assert_eq!(
            store.current_state(),
            &store.reconstruct_through(EventOrdinal(3)),
            "replay must match the derived cache for every family"
        );
        assert_eq!(
            workspace, before,
            "no family may touch any managed version record"
        );
    }

    /// Store-level round trip (MILESTONES §E1.4.T5): the exact canonical
    /// event bytes and the chained digest are stored at write time, on
    /// the recorded row AND on its audit record. Walking the stored
    /// `(ordinal, event_bytes, digest)` triples alone — no live structs,
    /// no recomputation from external data — verifies the whole chain
    /// and reproduces every event, so a later export needs no extra
    /// data.
    #[test]
    fn exact_bytes_and_digest_chain_round_trip_from_stored_records_alone() {
        let mut workspace = workspace();
        let subject = subject_for(&mut workspace);
        let mut sink = InMemoryAuditSink::default();
        let mut store = ManagedStateEventStore::new(RetentionFloor(1));
        let all_ten_families = [
            ManagedStateChange::Governance {
                state: GovernanceState::Approved,
            },
            ManagedStateChange::Verification {
                state: VerificationState::Verified,
            },
            ManagedStateChange::Effectivity {
                state: EffectivityState::Effective,
            },
            ManagedStateChange::Freshness {
                state: FreshnessState::Current,
            },
            ManagedStateChange::Integrity {
                state: IntegrityState::Clear,
            },
            ManagedStateChange::Synchronization {
                connector: ConnectorId::new("confluence").expect("non-blank"),
                state: SynchronizationState::InSync,
                required_before_effective: false,
            },
            ManagedStateChange::AuthorizationAffectingSourceChange {
                connector: ConnectorId::new("confluence").expect("non-blank"),
            },
            ManagedStateChange::Declassification,
            ManagedStateChange::Migration,
            ManagedStateChange::DeletionTombstone {
                replay_posture: ReplayPosture::NoLongerReplayableAfterDeletion,
            },
        ];
        for change in all_ten_families {
            store
                .append(
                    &mut sink,
                    ManagedStateEvent {
                        subject: subject.clone(),
                        change,
                        emitter: EventEmitter::new("cloud.governance").expect("non-blank"),
                        policy_version: PolicyVersion::new("policy-1").expect("non-blank"),
                        corrects: None,
                    },
                )
                .expect("append accepted");
        }

        // The export input: only what the store persisted at write time.
        let stored: Vec<(EventOrdinal, String, String)> = store
            .events()
            .iter()
            .map(|record| {
                (
                    record.ordinal,
                    record.event_bytes.clone(),
                    record.digest.clone(),
                )
            })
            .collect();
        assert_eq!(stored.len(), 10);

        let mut previous_digest = String::new();
        for (index, (ordinal, event_bytes, digest)) in stored.iter().enumerate() {
            assert_eq!(*ordinal, EventOrdinal(index as u64));
            // Exact bytes: the stored text IS the canonical serialization
            // of the recorded event — byte-identical, not re-derived.
            assert_eq!(
                event_bytes,
                &serde_json::to_string(&store.events()[index].event).expect("event serializes"),
                "stored bytes drifted from the recorded event"
            );
            // Chain: each digest binds the exact bytes to the whole
            // prefix; verification needs only the stored triples.
            let chained = format!("{previous_digest}{event_bytes}");
            assert_eq!(
                digest,
                &crate::domain::hashing::sha256_prefixed(chained.as_bytes()),
                "digest chain broken at ordinal {index}"
            );
            previous_digest = digest.clone();
            // The bytes alone reproduce the event content.
            let replayed: serde_json::Value =
                serde_json::from_str(event_bytes).expect("stored bytes parse");
            assert_eq!(
                replayed,
                serde_json::to_value(&store.events()[index].event).expect("event serializes")
            );
        }

        // The audit trail carries the same bytes and digests: the full
        // lifecycle reconstructs from audit rows alone (acceptance).
        assert_eq!(sink.records.len(), 10);
        for (record, (ordinal, event_bytes, digest)) in sink.records.iter().zip(&stored) {
            assert_eq!(record.ordinal, *ordinal);
            assert_eq!(&record.event_bytes, event_bytes);
            assert_eq!(&record.digest, digest);
        }
    }

    /// Acceptance (MILESTONES §E1.4): the full lifecycle — record →
    /// deliver → approve → edit → invalidate → re-approve — reconstructs
    /// exactly from the audit rows alone. The edit is a content change:
    /// it mints a second immutable version on the same canonical
    /// identity (RT-04 — never a mutation of the recorded one), every
    /// subsequent event lands on the new subject, and the first
    /// version's recorded history stands unrewritten.
    #[test]
    fn full_lifecycle_with_an_edit_reconstructs_from_audit_rows_alone() {
        let mut workspace = workspace();
        let subject_v1 = subject_for(&mut workspace);
        let mut sink = InMemoryAuditSink::default();
        let mut store = ManagedStateEventStore::new(RetentionFloor(1));
        // record → deliver → approve, on the first managed version.
        for change in [
            ManagedStateChange::Governance {
                state: GovernanceState::Proposed,
            },
            ManagedStateChange::Effectivity {
                state: EffectivityState::Effective,
            },
            ManagedStateChange::Governance {
                state: GovernanceState::Approved,
            },
        ] {
            store
                .append(
                    &mut sink,
                    event(&subject_v1, change, "cloud.governance", "policy-1"),
                )
                .expect("append accepted");
        }

        // The edit: re-importing the object with changed governed meaning
        // mints a second immutable version on the same canonical identity.
        let edited = workspace
            .import_artifact(&artifact(vec![knowledge_object(
                "billing.credits",
                "sha256:bbb",
            )]))
            .expect("edit import accepted");
        let minted = &edited.imported[0];
        assert_eq!(minted.canonical, subject_v1.canonical);
        assert_ne!(minted.version_id, subject_v1.version_id);
        let subject_v2 = StateEventSubject {
            canonical: minted.canonical.clone(),
            version_id: minted.version_id.clone(),
        };
        assert_eq!(
            workspace
                .managed_object(&subject_v1.canonical)
                .expect("object recorded")
                .versions
                .len(),
            2,
            "the edit appends a second immutable version"
        );

        // invalidate → re-approve, landing on the new subject.
        for change in [
            ManagedStateChange::Governance {
                state: GovernanceState::Revoked,
            },
            ManagedStateChange::Governance {
                state: GovernanceState::Approved,
            },
        ] {
            store
                .append(
                    &mut sink,
                    event(&subject_v2, change, "cloud.governance", "policy-2"),
                )
                .expect("append accepted");
        }

        // Reconstruction input: the audit rows ALONE — each row's exact
        // bytes parsed back, keyed by the subject the bytes carry, folded
        // per dimension. No live store structures are consulted.
        let fold = |records: &[AuditRecord]| -> BTreeMap<(String, String), serde_json::Value> {
            let mut replayed = BTreeMap::new();
            for record in records {
                let bytes: serde_json::Value =
                    serde_json::from_str(&record.event_bytes).expect("audit bytes parse");
                let key = (
                    bytes["subject"]["canonical"]["canonical_id"]
                        .as_str()
                        .expect("canonical id")
                        .to_string(),
                    bytes["subject"]["version_id"]
                        .as_str()
                        .expect("version id")
                        .to_string(),
                );
                let state = replayed.entry(key).or_insert_with(|| {
                    json!({
                        "governance": "gap",
                        "verification": "gap",
                        "effectivity": "gap",
                        "freshness": "gap",
                        "integrity": "gap",
                        "synchronization": {}
                    })
                });
                let family = bytes["change"]["family"].as_str().expect("family");
                assert!(
                    matches!(
                        family,
                        "governance" | "verification" | "effectivity" | "freshness" | "integrity"
                    ),
                    "unexpected family in the lifecycle fixture: {family}"
                );
                state[family] = json!({ "recorded": bytes["change"]["state"].clone() });
            }
            replayed
        };
        let key_of = |subject: &StateEventSubject| {
            let value = serde_json::to_value(subject).expect("subject serializes");
            (
                value["canonical"]["canonical_id"]
                    .as_str()
                    .expect("canonical id")
                    .to_string(),
                value["version_id"]
                    .as_str()
                    .expect("version id")
                    .to_string(),
            )
        };

        // The full replay from audit rows matches the derived cache.
        let replayed = fold(&sink.records);
        let expected: BTreeMap<(String, String), serde_json::Value> = store
            .current_state()
            .iter()
            .map(|(subject, state)| {
                (
                    key_of(subject),
                    serde_json::to_value(state).expect("state serializes"),
                )
            })
            .collect();
        assert_eq!(
            replayed, expected,
            "audit rows alone must reproduce the derived cache"
        );
        assert_eq!(
            replayed[&key_of(&subject_v1)],
            json!({
                "governance": { "recorded": "approved" },
                "verification": "gap",
                "effectivity": { "recorded": "effective" },
                "freshness": "gap",
                "integrity": "gap",
                "synchronization": {}
            }),
            "the first version's history stands unrewritten after the edit"
        );
        assert_eq!(
            replayed[&key_of(&subject_v2)],
            json!({
                "governance": { "recorded": "approved" },
                "verification": "gap",
                "effectivity": "gap",
                "freshness": "gap",
                "integrity": "gap",
                "synchronization": {}
            }),
            "the re-approval lands on the edited version"
        );
        // Mid-log: right after the invalidation (ordinal 3), the edited
        // version is revoked — the re-approval is invisible.
        let at_invalidation = fold(&sink.records[..4]);
        assert_eq!(
            at_invalidation[&key_of(&subject_v2)]["governance"],
            json!({ "recorded": "revoked" })
        );
    }

    /// An event whose bytes cannot be produced must fail the owning
    /// operation with `audit.persistence_failed` — exact bytes at write
    /// time are part of the audit trail, and nothing succeeds without
    /// them.
    #[test]
    fn event_serialization_failure_maps_to_audit_persistence_failed() {
        assert_eq!(
            StateEventStoreError::EventSerializationUnavailable
                .diagnostic_code()
                .as_str(),
            "audit.persistence_failed"
        );
    }
}
