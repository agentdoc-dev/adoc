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

/// The append-only managed state event log. Appending is the ONLY
/// mutation this type exposes — there is no update or delete method at
/// all, so append-only holds at the store layer rather than by handler
/// discipline (E1.4.T2 hardens this with conflict and retention-floor
/// rejection codes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManagedStateEventStore {
    events: Vec<RecordedStateEvent>,
}

impl ManagedStateEventStore {
    pub(crate) fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// Append one state event, assigning the next ordinal. Never touches
    /// any managed version record: a state-only transition leaves every
    /// version ID and `content_hash` unchanged and creates no content
    /// version (RT-04) — this store has no reference to the workspace's
    /// version records at all, so the invariant holds by construction.
    pub(crate) fn append(&mut self, event: ManagedStateEvent) -> EventOrdinal {
        let ordinal = EventOrdinal(self.events.len() as u64);
        self.events.push(RecordedStateEvent { ordinal, event });
        ordinal
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

        let mut store = ManagedStateEventStore::new();
        store.append(freshness_event(subject.clone(), FreshnessState::Current));
        store.append(freshness_event(subject.clone(), FreshnessState::Stale));

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
        let mut store = ManagedStateEventStore::new();
        store.append(freshness_event(
            StateEventSubject {
                canonical: imported.canonical.clone(),
                version_id: imported.version_id.clone(),
            },
            FreshnessState::Stale,
        ));

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
                    "policy_version": "freshness-policy-1"
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
