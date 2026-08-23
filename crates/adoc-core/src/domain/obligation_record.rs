//! Stage-bound stateful Proof Obligation records (E1.6; KNOWLEDGE-MODEL
//! §K8; DECISION-REGISTER D16; MILESTONES §E1.6).
//!
//! The shipped stateless [`ProofObligation`] wire shape is embedded in
//! `adoc.review.v0` / `adoc.patch.check.v0` and stays untouched (public
//! API); this module adds the NEW registered contract
//! `adoc.proof_obligation.v0`: typed state, a `required_at` stage, and an
//! exact managed-version subject binding — never a bare Object ID.
//! [`ProofObligationRecord::from_legacy`] relates the two shapes; nothing
//! mutates the legacy one.
//!
//! Both §K8 vocabularies are CLOSED — obligation states
//! (`open|satisfied|waived|failed|expired`) and `required_at` stages
//! (`proposal_validation|approval|verification|effectivity|`
//! `connector_synchronization|agent_action`) are registered in
//! CONTRACT-REGISTRY.md and guard-pinned against §K8; a new value is a
//! registry edit plus an annex amendment, never an ad hoc string.
//!
//! Whether an obligation is informational or blocking at a stage/risk/
//! action is DATA — a typed [`ObligationPolicy`] input — never a
//! hard-coded role or stage check: the same obligation flips
//! informational ↔ blocking by policy data alone (§K8).
//!
//! Obligation state changes are obligation-record appends in
//! [`ObligationLedger`], NOT new E1.4 `ManagedStateChange` families: the
//! E1.4 event families are the closed §K4 vocabulary, pinned three ways
//! (registry table, §K4 fenced block, audit emitter registry), and §K8
//! defines obligations as their own record kind — not a seventh managed
//! dimension. The ledger mirrors the E1.4 append-only posture (append is
//! the only mutation it exposes) and holds no reference to the E1.4
//! store or any §K4 state, so obligation activity — waivers included —
//! cannot touch a managed dimension by construction.
//!
//! No wall-clock exists anywhere in these records: time-bounds evaluate
//! against an explicit event-ordinal input (E1.4's only notion of time).

use std::collections::BTreeMap;

use serde::Serialize;

use super::managed_state::{EventOrdinal, StateEventSubject};
use super::obligation::ProofObligation;
use super::reconciliation::{PolicyVersion, Principal};

/// The registered contract id every serialized record carries.
pub(crate) const PROOF_OBLIGATION_SCHEMA_VERSION: &str = "adoc.proof_obligation.v0";

/// §K8 obligation state (registered closed vocabulary).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ObligationState {
    Open,
    Satisfied,
    Waived,
    Failed,
    Expired,
}

impl ObligationState {
    pub(crate) const ALL: [Self; 5] = [
        Self::Open,
        Self::Satisfied,
        Self::Waived,
        Self::Failed,
        Self::Expired,
    ];

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Satisfied => "satisfied",
            Self::Waived => "waived",
            Self::Failed => "failed",
            Self::Expired => "expired",
        }
    }
}

/// §K8 `required_at` stage (registered closed vocabulary).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ObligationStage {
    ProposalValidation,
    Approval,
    Verification,
    Effectivity,
    ConnectorSynchronization,
    AgentAction,
}

impl ObligationStage {
    pub(crate) const ALL: [Self; 6] = [
        Self::ProposalValidation,
        Self::Approval,
        Self::Verification,
        Self::Effectivity,
        Self::ConnectorSynchronization,
        Self::AgentAction,
    ];

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ProposalValidation => "proposal_validation",
            Self::Approval => "approval",
            Self::Verification => "verification",
            Self::Effectivity => "effectivity",
            Self::ConnectorSynchronization => "connector_synchronization",
            Self::AgentAction => "agent_action",
        }
    }
}

/// How an obligation counts at a stage — enclosed by the
/// `adoc.proof_obligation.v0` envelope (registry rule: field vocabularies
/// governed by the envelope's schema).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ObligationClassification {
    Informational,
    Blocking,
}

/// One obligation's identity within a workspace's obligation ledger.
/// Non-blank without surrounding whitespace, never normalized — the
/// fail-closed posture of every identity value since E1.2.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub(crate) struct ObligationId(String);

impl ObligationId {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, ObligationError> {
        let value = value.into();
        if value.is_empty() || value.trim() != value {
            Err(ObligationError::InvalidObligationId)
        } else {
            Ok(Self(value))
        }
    }
}

/// The typed, stateful, stage-bound Proof Obligation record
/// (`adoc.proof_obligation.v0`; §K8/D16), bound to the exact managed
/// version it constrains. Every record starts `open`
/// ([`ProofObligationRecord::open`]); later states are ledger appends,
/// and `waived` is reachable only through a Waiver record (E1.6.T2).
/// Fields are private so the validating constructors are the only
/// doors ([`ObligationWaiver`] posture) — contract parity is
/// structural, never bypassable by struct literal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ProofObligationRecord {
    schema_version: &'static str,
    obligation_id: ObligationId,
    /// The exact immutable content version the obligation constrains
    /// (workspace canonical identity + managed version ID, E1.4).
    subject: StateEventSubject,
    reason: String,
    required_evidence: Vec<String>,
    required_at: ObligationStage,
    /// The producer's risk classification of the constrained change,
    /// matched against risk-scoped policy rules. Data, never authority.
    #[serde(skip_serializing_if = "Option::is_none")]
    risk: Option<String>,
    state: ObligationState,
}

impl ProofObligationRecord {
    /// Open a new stage-bound obligation. Rejects a blank reason, a
    /// blank required-evidence entry, and a blank risk — the published
    /// schema requires each to be non-empty (`minLength: 1`), and the
    /// domain must never construct what the contract rejects.
    pub(crate) fn open(
        obligation_id: ObligationId,
        subject: StateEventSubject,
        reason: impl Into<String>,
        required_evidence: Vec<String>,
        required_at: ObligationStage,
        risk: Option<String>,
    ) -> Result<Self, ObligationError> {
        let reason = reason.into();
        if reason.trim().is_empty() {
            return Err(ObligationError::InvalidReason);
        }
        if required_evidence
            .iter()
            .any(|entry| entry.trim().is_empty())
        {
            return Err(ObligationError::InvalidRequiredEvidence);
        }
        // `risk` is a MATCH KEY compared by exact equality in
        // `ObligationPolicy::classify` — surrounding whitespace would
        // silently miss its rule and fail OPEN into the default
        // classification, so it is unrepresentable (the
        // ObligationId/Principal/PolicyVersion posture).
        if risk
            .as_deref()
            .is_some_and(|risk| risk.is_empty() || risk.trim() != risk)
        {
            return Err(ObligationError::InvalidRisk);
        }
        Ok(Self {
            schema_version: PROOF_OBLIGATION_SCHEMA_VERSION,
            obligation_id,
            subject,
            reason,
            required_evidence,
            required_at,
            risk,
            state: ObligationState::Open,
        })
    }

    /// Bridge from the shipped stateless wire shape (relation, never
    /// mutation). Reason and required evidence carry over; the caller
    /// supplies the workspace-qualified subject the legacy flat
    /// `object_id` cannot carry — a bare Object ID is never a managed
    /// subject (E1.2) — and the stage the obligation binds to. A legacy
    /// obligation carrying a blank evidence entry fails closed here —
    /// the E1.2+ posture, never silently forwarded.
    pub(crate) fn from_legacy(
        legacy: &ProofObligation,
        obligation_id: ObligationId,
        subject: StateEventSubject,
        required_at: ObligationStage,
    ) -> Result<Self, ObligationError> {
        Self::open(
            obligation_id,
            subject,
            legacy.reason.clone(),
            legacy.required_evidence.clone(),
            required_at,
            None,
        )
    }
}

/// The classification policy (§K8): whether an obligation is
/// informational or blocking at each stage/risk/action is DATA carried
/// by this typed input — code never hard-codes a role or stage check,
/// so classification flips by policy data alone. Part of the
/// `adoc.proof_obligation.v0` envelope contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ObligationPolicy {
    pub(crate) policy_version: PolicyVersion,
    /// First matching rule wins.
    pub(crate) rules: Vec<ObligationPolicyRule>,
    /// Applies when no rule matches — itself policy data, so even the
    /// fallback is never hard-coded.
    pub(crate) default_classification: ObligationClassification,
}

/// One classification rule. `risk` and `action` are optional exact
/// matchers — absent means "any". `action` is matched at agent-action
/// evaluation (enforcement is E6.1+); it is part of the data shape now
/// so a policy can already scope rules per action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ObligationPolicyRule {
    stage: ObligationStage,
    #[serde(skip_serializing_if = "Option::is_none")]
    risk: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    action: Option<String>,
    classification: ObligationClassification,
}

impl ObligationPolicyRule {
    /// Construct a rule with validated matchers. A present-but-blank or
    /// whitespace-padded `risk`/`action` fails closed: the published
    /// `$defs/classificationRule` pins `minLength: 1` on both, and the
    /// matchers are compared by exact equality in
    /// [`ObligationPolicy::classify`] — a padded matcher would silently
    /// never match and fail OPEN into the default classification.
    /// Fields are private so this is the only door (the
    /// [`ObligationWaiver`] posture).
    pub(crate) fn new(
        stage: ObligationStage,
        risk: Option<String>,
        action: Option<String>,
        classification: ObligationClassification,
    ) -> Result<Self, ObligationError> {
        if [&risk, &action].into_iter().any(|matcher| {
            matcher
                .as_deref()
                .is_some_and(|value| value.is_empty() || value.trim() != value)
        }) {
            return Err(ObligationError::InvalidRuleMatcher);
        }
        Ok(Self {
            stage,
            risk,
            action,
            classification,
        })
    }
}

impl ObligationPolicy {
    /// Classify one obligation context from the rule data alone.
    pub(crate) fn classify(
        &self,
        stage: ObligationStage,
        risk: Option<&str>,
        action: Option<&str>,
    ) -> ObligationClassification {
        self.rules
            .iter()
            .find(|rule| {
                rule.stage == stage
                    && rule
                        .risk
                        .as_deref()
                        .is_none_or(|scoped| Some(scoped) == risk)
                    && rule
                        .action
                        .as_deref()
                        .is_none_or(|scoped| Some(scoped) == action)
            })
            .map_or(self.default_classification, |rule| rule.classification)
    }
}

/// An Obligation Waiver (§K8): the permission-controlled discharge of
/// one stage-bound obligation. Bound at the type level to the EXACT
/// obligation, workspace-qualified managed-version subject, principal,
/// and policy version — all non-optional — with a non-blank
/// justification, and time-bounded
/// where appropriate by an explicit event-ordinal expiry (never a wall
/// clock). Fields are private and [`ObligationWaiver::new`] is the only
/// constructor; like the E1.3 decision record, `Deserialize` is
/// deliberately absent — never derive it, or unprincipaled waivers
/// become constructible from bytes.
///
/// A waiver changes OBLIGATION state only ([`ObligationLedger::waive`]):
/// it cannot name, let alone advance, any §K4 dimension — waiving a
/// verification obligation leaves verification `unverified` by
/// construction (MILESTONES §E1.6 exit gate).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ObligationWaiver {
    obligation_id: ObligationId,
    /// The exact managed-version subject (workspace canonical identity
    /// plus version ID) the waived obligation constrains — a waiver
    /// never carries over to another version, and `ManagedVersionId`
    /// alone is workspace-minted (`mv-N` recurs in every workspace), so
    /// the subject is the only exact binding once the waiver leaves its
    /// workspace (E1.2: a bare identifier is never a managed subject).
    subject: StateEventSubject,
    principal: Principal,
    /// The waiver-GRANTING policy version — replay/audit data naming the
    /// authority the grant was made under. Distinct from the
    /// classification [`ObligationPolicy`] evaluated at assessment, so
    /// no comparison exists in this slice.
    // ponytail: recorded, never consulted — a waiver outlives the policy
    // that authorized it (the ordinal bound is its designed lifetime);
    // re-evaluating standing waivers against an in-force waiver policy
    // is a later slice's decision, taken with the E5.2/E5.3 approval
    // and gate machinery, not silently here.
    policy_version: PolicyVersion,
    justification: String,
    /// The last E1.4 event ordinal this waiver holds at; evaluated at an
    /// explicit ordinal input (`at > expires_after` = expired). `None`:
    /// not time-bounded.
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_after: Option<EventOrdinal>,
}

impl ObligationWaiver {
    /// Construct a fully bound waiver. Rejects a blank justification —
    /// an unjustified waiver must not exist (§K8).
    pub(crate) fn new(
        obligation_id: ObligationId,
        subject: StateEventSubject,
        principal: Principal,
        policy_version: PolicyVersion,
        justification: impl Into<String>,
        expires_after: Option<EventOrdinal>,
    ) -> Result<Self, ObligationError> {
        let justification = justification.into();
        if justification.trim().is_empty() {
            return Err(ObligationError::InvalidJustification);
        }
        Ok(Self {
            obligation_id,
            subject,
            principal,
            policy_version,
            justification,
            expires_after,
        })
    }

    /// Whether this waiver has lapsed at the explicit evaluation ordinal
    /// `at` — the only notion of time here (no wall clock).
    pub(crate) fn is_expired(&self, at: EventOrdinal) -> bool {
        self.expires_after.is_some_and(|bound| at > bound)
    }
}

/// One obligation's effective assessment at an explicit evaluation
/// ordinal: its state after waiver-expiry evaluation, and how it counts
/// per the classification policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct ObligationAssessment {
    pub(crate) state: ObligationState,
    pub(crate) classification: ObligationClassification,
}

/// Why an obligation write was rejected — fail closed, never a silent
/// fallback.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ObligationError {
    #[error("obligation id must be non-blank without surrounding whitespace")]
    InvalidObligationId,
    #[error("obligation reason must be non-blank")]
    InvalidReason,
    #[error("every required-evidence entry must be non-blank")]
    InvalidRequiredEvidence,
    #[error("an authored risk classification must be non-blank")]
    InvalidRisk,
    #[error("a classification-rule matcher must be non-blank when present")]
    InvalidRuleMatcher,
    #[error("waiver justification must be non-blank")]
    InvalidJustification,
    #[error("waiver names obligation {id:?} but is bound to a different managed-version subject")]
    WaiverBindingMismatch { id: ObligationId },
    #[error("obligation {id:?} is not open; only an open obligation is waivable")]
    NotWaivable { id: ObligationId },
    #[error("obligation {id:?} is already open in this ledger")]
    DuplicateObligation { id: ObligationId },
    #[error("obligation {id:?} is not recorded in this ledger")]
    UnknownObligation { id: ObligationId },
    #[error(
        "obligation {id:?} already stands `{}` — a terminal state never transitions again",
        standing.as_str()
    )]
    TerminalStateStanding {
        id: ObligationId,
        standing: ObligationState,
    },
    #[error("an opened obligation record must start in the `open` state")]
    OpenedRecordNotOpen,
    #[error("`waived` is reachable only through a waiver record, never a bare state append")]
    WaiverRequired,
    #[error("`open` is the opening state; reopening is derived from waiver expiry, never appended")]
    ReopenNotRecordable,
}

/// One appended obligation event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub(crate) enum ObligationEvent {
    Opened {
        record: ProofObligationRecord,
    },
    StateRecorded {
        obligation_id: ObligationId,
        state: ObligationState,
    },
    Waived {
        waiver: ObligationWaiver,
    },
}

/// The append-only obligation ledger. Appending is the ONLY mutation it
/// exposes (E1.4 posture), and it holds no reference to the E1.4 store
/// or any §K4 state — obligation activity cannot touch a managed
/// dimension by construction.
// ponytail: no digest chain here — ledger events are not exported as
// audit rows yet; enclosing them in the E1.4/E4.2 audit surfaces is
// those slices' concern and needs no reshaping of these records.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ObligationLedger {
    events: Vec<ObligationEvent>,
}

impl ObligationLedger {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// The appended events, in order — the replay input.
    pub(crate) fn events(&self) -> &[ObligationEvent] {
        &self.events
    }

    /// Append an opening record. A duplicate obligation id contradicts
    /// the recorded ledger and fails closed.
    pub(crate) fn open(&mut self, record: ProofObligationRecord) -> Result<(), ObligationError> {
        if record.state != ObligationState::Open {
            // Unreachable through the validating constructors (no
            // `Deserialize` exists), but fail closed rather than trust it.
            return Err(ObligationError::OpenedRecordNotOpen);
        }
        if self.opened_record(&record.obligation_id).is_some() {
            return Err(ObligationError::DuplicateObligation {
                id: record.obligation_id.clone(),
            });
        }
        self.events.push(ObligationEvent::Opened { record });
        Ok(())
    }

    /// Append a state transition. `satisfied`/`failed`/`expired` are the
    /// recordable transitions; `waived` requires a Waiver record
    /// (permission-controlled, E1.6.T2) and `open` is never re-appended —
    /// reopening is derived from waiver expiry, not authored.
    ///
    /// Source states fail closed: only a standing `open` or `waived`
    /// obligation accepts a recorded transition. `satisfied`/`failed`/
    /// `expired` are TERMINAL — `failed → satisfied` would upgrade a
    /// recorded outcome with no evidence trail (V10.2.5: state events
    /// never upgrade a recorded outcome), so a terminal state only exits
    /// via a correcting append in the E1.4 `corrects` posture, which is
    /// not this slice's. A standing `waived` obligation still records
    /// `satisfied`/`failed`/`expired` — the documented recorded exits
    /// from a waiver (see [`ObligationLedger::waive`]).
    // ponytail: StateRecorded carries no principal/policy/evidence —
    // attribution lands when ledger events are enclosed as E1.4/E4.2
    // audit rows (see the ledger's ponytail note); the waiver, the only
    // discharge with authority semantics, is fully bound already.
    pub(crate) fn record_state(
        &mut self,
        obligation_id: &ObligationId,
        state: ObligationState,
    ) -> Result<(), ObligationError> {
        match state {
            ObligationState::Waived => return Err(ObligationError::WaiverRequired),
            ObligationState::Open => return Err(ObligationError::ReopenNotRecordable),
            ObligationState::Satisfied | ObligationState::Failed | ObligationState::Expired => {}
        }
        let Some(standing) = self
            .standing()
            .get(obligation_id)
            .map(|record| record.state)
        else {
            return Err(ObligationError::UnknownObligation {
                id: obligation_id.clone(),
            });
        };
        if matches!(
            standing,
            ObligationState::Satisfied | ObligationState::Failed | ObligationState::Expired
        ) {
            return Err(ObligationError::TerminalStateStanding {
                id: obligation_id.clone(),
                standing,
            });
        }
        self.events.push(ObligationEvent::StateRecorded {
            obligation_id: obligation_id.clone(),
            state,
        });
        Ok(())
    }

    /// Append a waiver. Fail-closed validation against the recorded
    /// ledger: the obligation must exist, the waiver must be bound to
    /// that obligation's exact workspace-qualified managed-version
    /// subject, and only an `open` obligation is waivable. Structurally this method appends to THIS
    /// ledger and touches nothing else — the ledger holds no reference
    /// to the E1.4 store or any §K4 dimension, so a waiver cannot
    /// convert `unverified` to `verified` (MILESTONES §E1.6 exit gate).
    ///
    /// Waivability is judged against the RECORDED standing state, not
    /// the derived assessment: once a waiver lapses the obligation is
    /// assessed `open`+blocking, but its recorded state stays `waived`,
    /// so a second waiver is `NotWaivable` — the only recorded exits are
    /// `satisfied`/`failed`/`expired`. Fail closed: a lapsed waiver is
    /// never quietly replaced by a fresh one.
    // ponytail: no waiver-renewal path — deliberate ceiling; if policy
    // ever wants renewable waivers, `waive` takes an evaluation ordinal
    // and treats a lapsed last waiver as open (a new slice's decision,
    // not this one's).
    pub(crate) fn waive(&mut self, waiver: ObligationWaiver) -> Result<(), ObligationError> {
        let Some(record) = self.opened_record(&waiver.obligation_id) else {
            return Err(ObligationError::UnknownObligation {
                id: waiver.obligation_id.clone(),
            });
        };
        if record.subject != waiver.subject {
            return Err(ObligationError::WaiverBindingMismatch {
                id: waiver.obligation_id.clone(),
            });
        }
        // Fail closed rather than index: the `opened_record` check above
        // makes the key's presence provable today, but a panic path in
        // library code outlives proofs.
        let Some(standing_state) = self
            .standing()
            .get(&waiver.obligation_id)
            .map(|record| record.state)
        else {
            return Err(ObligationError::UnknownObligation {
                id: waiver.obligation_id.clone(),
            });
        };
        if standing_state != ObligationState::Open {
            return Err(ObligationError::NotWaivable {
                id: waiver.obligation_id.clone(),
            });
        }
        self.events.push(ObligationEvent::Waived { waiver });
        Ok(())
    }

    fn opened_record(&self, id: &ObligationId) -> Option<&ProofObligationRecord> {
        self.events.iter().find_map(|event| match event {
            ObligationEvent::Opened { record } if record.obligation_id == *id => Some(record),
            _ => None,
        })
    }

    /// The last appended waiver for one obligation. Present whenever the
    /// standing state is `waived` — [`ObligationLedger::waive`] is the
    /// only path to that state.
    fn last_waiver(&self, id: &ObligationId) -> Option<&ObligationWaiver> {
        self.events.iter().rev().find_map(|event| match event {
            ObligationEvent::Waived { waiver } if waiver.obligation_id == *id => Some(waiver),
            _ => None,
        })
    }

    /// One obligation's effective assessment at the explicit evaluation
    /// ordinal `at`. An expired waiver reopens its obligation as
    /// BLOCKING regardless of policy — a stale waiver never authorizes
    /// (MILESTONES §E1.6 adversarial acceptance); everything else takes
    /// its classification from the policy DATA. The agent-action
    /// `action` matcher is evaluated with no action here — action-scoped
    /// enforcement is E6.1+.
    pub(crate) fn assess(
        &self,
        id: &ObligationId,
        policy: &ObligationPolicy,
        at: EventOrdinal,
    ) -> Result<ObligationAssessment, ObligationError> {
        let record = self
            .standing()
            .remove(id)
            .ok_or_else(|| ObligationError::UnknownObligation { id: id.clone() })?;
        Ok(self.assess_record(&record, policy, at))
    }

    fn assess_record(
        &self,
        record: &ProofObligationRecord,
        policy: &ObligationPolicy,
        at: EventOrdinal,
    ) -> ObligationAssessment {
        if record.state == ObligationState::Waived
            && self
                .last_waiver(&record.obligation_id)
                .is_some_and(|waiver| waiver.is_expired(at))
        {
            return ObligationAssessment {
                state: ObligationState::Open,
                classification: ObligationClassification::Blocking,
            };
        }
        ObligationAssessment {
            state: record.state,
            classification: policy.classify(record.required_at, record.risk.as_deref(), None),
        }
    }

    /// The obligations blocking the `approval_required` gate at `at`,
    /// in deterministic id order: undischarged (a satisfied state or a
    /// still-valid waiver discharges; `open`/`failed`/`expired` do not),
    /// required at the approval gate stage, and classified blocking by
    /// the policy data. §K8: `approval_required` blocks ONLY obligations
    /// explicitly required before gate passage — every other obligation
    /// may permit merge while the E1.4 store keeps reporting unverified,
    /// pending effectivity, blocked synchronization, or high-risk
    /// ineligibility for its own stage.
    pub(crate) fn approval_blockers(
        &self,
        policy: &ObligationPolicy,
        at: EventOrdinal,
    ) -> Vec<ObligationId> {
        self.standing()
            .into_iter()
            .filter_map(|(id, record)| {
                let assessment = self.assess_record(&record, policy, at);
                let discharged = matches!(
                    assessment.state,
                    ObligationState::Satisfied | ObligationState::Waived
                );
                (!discharged
                    && record.required_at == ObligationStage::Approval
                    && assessment.classification == ObligationClassification::Blocking)
                    .then_some(id)
            })
            .collect()
    }

    /// The standing obligations: each opened record with its current
    /// state replayed from the appends. The serialized form of a
    /// standing record is exactly the wire record at this point of the
    /// ledger.
    pub(crate) fn standing(&self) -> BTreeMap<ObligationId, ProofObligationRecord> {
        let mut standing: BTreeMap<ObligationId, ProofObligationRecord> = BTreeMap::new();
        for event in &self.events {
            match event {
                ObligationEvent::Opened { record } => {
                    standing.insert(record.obligation_id.clone(), record.clone());
                }
                ObligationEvent::StateRecorded {
                    obligation_id,
                    state,
                } => {
                    if let Some(record) = standing.get_mut(obligation_id) {
                        record.state = *state;
                    }
                }
                ObligationEvent::Waived { waiver } => {
                    if let Some(record) = standing.get_mut(&waiver.obligation_id) {
                        record.state = ObligationState::Waived;
                    }
                }
            }
        }
        standing
    }
}

#[cfg(test)]
mod tests {
    //! E1.6.T1 exit tests (MILESTONES §E1.6; KNOWLEDGE-MODEL §K8; D16):
    //! an object can be Approved + Not verified + Pending effectivity
    //! simultaneously, and blocking-vs-informational flips by policy
    //! data alone.

    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;
    use crate::domain::graph::{
        GraphArtifactDocument, GraphKnowledgeObjectNode, GraphNode, GraphRelations,
        GraphRepositoryIdentity, GraphSourceSpan,
    };
    use crate::domain::managed::{ManagedWorkspace, WorkspaceId};
    use crate::domain::managed_state::{
        AuditRecord, AuditSink, AuditSinkError, ConnectorId, EffectivityState, EventEmitter,
        GovernanceState, ManagedStateChange, ManagedStateEvent, ManagedStateEventStore,
        RecordedDimension, RetentionFloor, StateEventSubject, SynchronizationState,
        VerificationState,
    };
    use crate::domain::obligation::ProofObligation;
    use crate::domain::reconciliation::PolicyVersion;

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

    fn floor(records: u64) -> RetentionFloor {
        RetentionFloor::new(records).expect("non-zero")
    }

    /// Import one real object and return its version-exact subject.
    fn imported_subject() -> StateEventSubject {
        let mut workspace =
            ManagedWorkspace::new(WorkspaceId::new("ws-acme").expect("workspace id is non-blank"));
        let outcome = workspace
            .import_artifact(&artifact(vec![knowledge_object(
                "billing.credits",
                "sha256:aaa",
            )]))
            .expect("import accepted");
        let imported = &outcome.imported[0];
        StateEventSubject::bound(&workspace, &imported.canonical, &imported.version_id)
            .expect("an imported version binds")
    }

    fn state_event(subject: StateEventSubject, change: ManagedStateChange) -> ManagedStateEvent {
        ManagedStateEvent {
            subject,
            change,
            emitter: EventEmitter::new("cloud.governance_service").expect("non-blank"),
            policy_version: PolicyVersion::new("state-policy-1").expect("non-blank"),
            corrects: None,
        }
    }

    fn obligation_id(value: &str) -> ObligationId {
        ObligationId::new(value).expect("non-blank")
    }

    fn open_record(subject: &StateEventSubject) -> ProofObligationRecord {
        ProofObligationRecord::open(
            obligation_id("ob-billing-credits-verification"),
            subject.clone(),
            "stale verified claim",
            vec!["source".to_string(), "reviewed_by".to_string()],
            ObligationStage::Verification,
            Some("high".to_string()),
        )
        .expect("valid record")
    }

    fn policy_version() -> PolicyVersion {
        PolicyVersion::new("obligation-policy-1").expect("non-blank")
    }

    fn rule(
        stage: ObligationStage,
        risk: Option<&str>,
        action: Option<&str>,
        classification: ObligationClassification,
    ) -> ObligationPolicyRule {
        ObligationPolicyRule::new(
            stage,
            risk.map(str::to_string),
            action.map(str::to_string),
            classification,
        )
        .expect("valid rule")
    }

    /// §K8's headline composite (the E1.6.T1 failing test): with real
    /// E1.4 state events one object is Approved + Not verified + Pending
    /// effectivity SIMULTANEOUSLY — and the approval discharges nothing:
    /// the verification-stage obligation stays `open`.
    #[test]
    fn approved_not_verified_pending_effectivity_coexist_and_discharge_nothing() {
        let subject = imported_subject();
        let mut sink = InMemoryAuditSink::default();
        let mut store = ManagedStateEventStore::new(floor(1));
        for change in [
            ManagedStateChange::Governance {
                state: GovernanceState::Approved,
            },
            ManagedStateChange::Verification {
                state: VerificationState::Unverified,
            },
            ManagedStateChange::Effectivity {
                state: EffectivityState::Pending,
            },
        ] {
            store
                .append(&mut sink, state_event(subject.clone(), change))
                .expect("append accepted");
        }
        let state = &store.current_state()[&subject];
        assert_eq!(
            state.governance,
            RecordedDimension::Recorded(GovernanceState::Approved)
        );
        assert_eq!(
            state.verification,
            RecordedDimension::Recorded(VerificationState::Unverified)
        );
        assert_eq!(
            state.effectivity,
            RecordedDimension::Recorded(EffectivityState::Pending)
        );

        let mut ledger = ObligationLedger::new();
        ledger.open(open_record(&subject)).expect("opens");
        let standing = ledger.standing();
        assert_eq!(
            standing[&obligation_id("ob-billing-credits-verification")].state,
            ObligationState::Open,
            "approval never discharges a verification-stage obligation"
        );
    }

    /// Blocking vs informational is decided by policy DATA alone: the
    /// same obligation under two policies differing only in rule data
    /// flips classification — no role or stage check exists in code.
    #[test]
    fn classification_flips_by_policy_data_alone() {
        let policy = |classification| ObligationPolicy {
            policy_version: policy_version(),
            rules: vec![rule(ObligationStage::Approval, None, None, classification)],
            default_classification: ObligationClassification::Informational,
        };
        assert_eq!(
            policy(ObligationClassification::Blocking).classify(
                ObligationStage::Approval,
                None,
                None
            ),
            ObligationClassification::Blocking
        );
        assert_eq!(
            policy(ObligationClassification::Informational).classify(
                ObligationStage::Approval,
                None,
                None
            ),
            ObligationClassification::Informational
        );
    }

    /// Rules scope by stage, risk, and action — absent matchers mean
    /// "any"; the fallback classification is itself policy data.
    #[test]
    fn rules_scope_by_stage_risk_and_action_with_a_data_default() {
        let policy = ObligationPolicy {
            policy_version: policy_version(),
            rules: vec![
                rule(
                    ObligationStage::AgentAction,
                    Some("high"),
                    None,
                    ObligationClassification::Blocking,
                ),
                rule(
                    ObligationStage::ConnectorSynchronization,
                    None,
                    Some("writeback"),
                    ObligationClassification::Blocking,
                ),
            ],
            default_classification: ObligationClassification::Informational,
        };
        let classify = |stage, risk, action| policy.classify(stage, risk, action);
        assert_eq!(
            classify(ObligationStage::AgentAction, Some("high"), None),
            ObligationClassification::Blocking
        );
        assert_eq!(
            classify(ObligationStage::AgentAction, Some("low"), None),
            ObligationClassification::Informational
        );
        assert_eq!(
            classify(ObligationStage::AgentAction, None, None),
            ObligationClassification::Informational
        );
        assert_eq!(
            classify(
                ObligationStage::ConnectorSynchronization,
                None,
                Some("writeback")
            ),
            ObligationClassification::Blocking
        );
        assert_eq!(
            classify(ObligationStage::ConnectorSynchronization, None, None),
            ObligationClassification::Informational
        );
        assert_eq!(
            classify(ObligationStage::Verification, Some("high"), None),
            ObligationClassification::Informational,
            "a rule never matches another stage"
        );
    }

    /// The shipped stateless wire shape is RELATED, never mutated: the
    /// bridge lifts reason + required evidence into a stage-bound open
    /// record, and the caller supplies the workspace-qualified subject
    /// the legacy flat `object_id` cannot carry (a bare Object ID is
    /// never a managed subject, E1.2).
    #[test]
    fn bridge_from_legacy_preserves_reason_and_required_evidence() {
        let legacy = ProofObligation {
            object_id: "billing.credits".to_string(),
            reason: "stale verified claim".to_string(),
            required_evidence: vec!["owner".to_string(), "verified_at".to_string()],
        };
        let record = ProofObligationRecord::from_legacy(
            &legacy,
            obligation_id("ob-1"),
            imported_subject(),
            ObligationStage::Verification,
        )
        .expect("bridges");
        assert_eq!(record.reason, legacy.reason);
        assert_eq!(record.required_evidence, legacy.required_evidence);
        assert_eq!(record.state, ObligationState::Open);
        assert_eq!(record.required_at, ObligationStage::Verification);
    }

    /// The serialized record the Cloud cut consumes — no wall-clock
    /// anywhere.
    #[test]
    fn serialized_record_shape_is_pinned() {
        let record = open_record(&imported_subject());
        assert_eq!(
            serde_json::to_value(&record).expect("record serializes"),
            json!({
                "schema_version": "adoc.proof_obligation.v0",
                "obligation_id": "ob-billing-credits-verification",
                "subject": {
                    "canonical": {
                        "workspace_id": "ws-acme",
                        "canonical_id": "mo-1"
                    },
                    "version_id": "mv-1"
                },
                "reason": "stale verified claim",
                "required_evidence": ["source", "reviewed_by"],
                "required_at": "verification",
                "risk": "high",
                "state": "open"
            })
        );
    }

    /// Serializer ↔ published-schema parity (E1.5 precedent): the record
    /// and the classification policy — part of the same envelope contract
    /// (playbook decision 13) — validate against
    /// `docs/agent/v0/schema/adoc.proof_obligation.v0.schema.json`.
    #[test]
    fn serialized_shapes_validate_against_the_published_schema() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/agent/v0/schema/adoc.proof_obligation.v0.schema.json");
        let schema: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).expect("schema is readable"))
                .expect("schema is json");
        let validator = jsonschema::validator_for(&schema).expect("schema compiles");
        let instance =
            serde_json::to_value(open_record(&imported_subject())).expect("record serializes");
        let errors: Vec<String> = validator
            .iter_errors(&instance)
            .map(|error| error.to_string())
            .collect();
        assert!(
            errors.is_empty(),
            "adoc.proof_obligation.v0 schema validation failed:\n{}\ninstance:\n{}",
            errors.join("\n"),
            serde_json::to_string_pretty(&instance).expect("instance pretty prints")
        );

        let policy = ObligationPolicy {
            policy_version: policy_version(),
            rules: vec![
                rule(
                    ObligationStage::Approval,
                    None,
                    None,
                    ObligationClassification::Blocking,
                ),
                rule(
                    ObligationStage::AgentAction,
                    Some("high"),
                    Some("delete_page"),
                    ObligationClassification::Blocking,
                ),
            ],
            default_classification: ObligationClassification::Informational,
        };
        let instance = serde_json::to_value(&policy).expect("policy serializes");
        assert_eq!(
            instance,
            json!({
                "policy_version": "obligation-policy-1",
                "rules": [
                    { "stage": "approval", "classification": "blocking" },
                    {
                        "stage": "agent_action",
                        "risk": "high",
                        "action": "delete_page",
                        "classification": "blocking"
                    }
                ],
                "default_classification": "informational"
            })
        );
        let policy_schema = json!({
            "$defs": schema["$defs"],
            "$ref": "#/$defs/classificationPolicy"
        });
        let validator = jsonschema::validator_for(&policy_schema).expect("subschema compiles");
        let errors: Vec<String> = validator
            .iter_errors(&instance)
            .map(|error| error.to_string())
            .collect();
        assert!(
            errors.is_empty(),
            "classificationPolicy schema validation failed:\n{}\ninstance:\n{}",
            errors.join("\n"),
            serde_json::to_string_pretty(&instance).expect("instance pretty prints")
        );
    }

    /// The ledger fails closed on every write-shaped request that
    /// contradicts it — and `waived`/`open` are never bare appends.
    #[test]
    fn ledger_rejects_duplicates_unknowns_and_unwaivered_states() {
        let subject = imported_subject();
        let mut ledger = ObligationLedger::new();
        ledger.open(open_record(&subject)).expect("opens");
        let id = obligation_id("ob-billing-credits-verification");

        assert_eq!(
            ledger.open(open_record(&subject)),
            Err(ObligationError::DuplicateObligation { id: id.clone() })
        );
        assert_eq!(
            ledger.record_state(&obligation_id("ob-unknown"), ObligationState::Satisfied),
            Err(ObligationError::UnknownObligation {
                id: obligation_id("ob-unknown")
            })
        );
        assert_eq!(
            ledger.record_state(&id, ObligationState::Waived),
            Err(ObligationError::WaiverRequired)
        );
        assert_eq!(
            ledger.record_state(&id, ObligationState::Open),
            Err(ObligationError::ReopenNotRecordable)
        );

        ledger
            .record_state(&id, ObligationState::Satisfied)
            .expect("satisfied is a recordable transition");
        assert_eq!(ledger.standing()[&id].state, ObligationState::Satisfied);
        assert_eq!(ledger.events().len(), 2);
    }

    /// Terminal standing states never transition again (fail closed):
    /// `failed → satisfied` would clear an approval blocker with no
    /// evidence trail (V10.2.5: never upgrade a recorded outcome), and
    /// `expired → satisfied` would resurrect a lapsed obligation. A
    /// standing `waived` obligation still records `satisfied` — the
    /// documented recorded exit from a waiver.
    #[test]
    fn terminal_states_never_transition_but_waived_still_exits() {
        let subject = imported_subject();
        let mut ledger = ObligationLedger::new();
        for id in ["ob-failed", "ob-expired", "ob-waived"] {
            ledger
                .open(record(&subject, id, ObligationStage::Approval, None))
                .expect("opens");
        }
        let policy = ObligationPolicy {
            policy_version: policy_version(),
            rules: Vec::new(),
            default_classification: ObligationClassification::Blocking,
        };

        let failed = obligation_id("ob-failed");
        ledger
            .record_state(&failed, ObligationState::Failed)
            .expect("records");
        assert_eq!(
            ledger.record_state(&failed, ObligationState::Satisfied),
            Err(ObligationError::TerminalStateStanding {
                id: failed.clone(),
                standing: ObligationState::Failed,
            }),
            "a failed obligation is never quietly satisfied"
        );
        assert!(
            ledger
                .approval_blockers(&policy, EventOrdinal(1))
                .contains(&failed),
            "the failed gate obligation keeps blocking approval"
        );

        let expired = obligation_id("ob-expired");
        ledger
            .record_state(&expired, ObligationState::Expired)
            .expect("records");
        assert_eq!(
            ledger.record_state(&expired, ObligationState::Satisfied),
            Err(ObligationError::TerminalStateStanding {
                id: expired.clone(),
                standing: ObligationState::Expired,
            })
        );

        let waived = obligation_id("ob-waived");
        ledger
            .waive(waiver(&subject, "ob-waived", None))
            .expect("waives");
        ledger
            .record_state(&waived, ObligationState::Satisfied)
            .expect("waived records its documented exits");
        assert_eq!(ledger.standing()[&waived].state, ObligationState::Satisfied);
    }

    /// The two §K8 vocabularies are closed and wire-stable; serde and
    /// `as_str` agree.
    #[test]
    fn the_k8_vocabularies_are_closed_and_wire_stable() {
        assert_eq!(
            ObligationState::ALL.map(ObligationState::as_str),
            ["open", "satisfied", "waived", "failed", "expired"]
        );
        assert_eq!(
            ObligationStage::ALL.map(ObligationStage::as_str),
            [
                "proposal_validation",
                "approval",
                "verification",
                "effectivity",
                "connector_synchronization",
                "agent_action"
            ]
        );
        for state in ObligationState::ALL {
            assert_eq!(json!(state), json!(state.as_str()));
        }
        for stage in ObligationStage::ALL {
            assert_eq!(json!(stage), json!(stage.as_str()));
        }
    }

    /// Blank identities, reasons, evidence entries, and risks fail
    /// closed — the published schema pins `minLength: 1` on each, and
    /// the domain never constructs what the contract rejects.
    #[test]
    fn blank_inputs_fail_closed() {
        assert_eq!(
            ObligationId::new(" ob-1"),
            Err(ObligationError::InvalidObligationId)
        );
        assert_eq!(
            ObligationId::new(""),
            Err(ObligationError::InvalidObligationId)
        );
        let subject = imported_subject();
        let open = |reason: &str, evidence: Vec<String>, risk: Option<String>| {
            ProofObligationRecord::open(
                obligation_id("ob-1"),
                subject.clone(),
                reason,
                evidence,
                ObligationStage::Approval,
                risk,
            )
        };
        assert_eq!(
            open("  ", Vec::new(), None),
            Err(ObligationError::InvalidReason)
        );
        assert_eq!(
            open("reason", vec![String::new()], None),
            Err(ObligationError::InvalidRequiredEvidence)
        );
        assert_eq!(
            open("reason", vec!["source".to_string(), " ".to_string()], None),
            Err(ObligationError::InvalidRequiredEvidence)
        );
        assert_eq!(
            open("reason", Vec::new(), Some(String::new())),
            Err(ObligationError::InvalidRisk)
        );
        // The legacy bridge inherits the same fail-closed boundary.
        let legacy = ProofObligation {
            object_id: "billing.credits".to_string(),
            reason: "stale verified claim".to_string(),
            required_evidence: vec![String::new()],
        };
        assert_eq!(
            ProofObligationRecord::from_legacy(
                &legacy,
                obligation_id("ob-1"),
                subject,
                ObligationStage::Verification,
            ),
            Err(ObligationError::InvalidRequiredEvidence)
        );
    }

    // ---- E1.6.T2: principal-bound obligation waivers ----

    fn waiver_for(
        subject: &StateEventSubject,
        expires_after: Option<EventOrdinal>,
    ) -> ObligationWaiver {
        ObligationWaiver::new(
            obligation_id("ob-billing-credits-verification"),
            subject.clone(),
            Principal::new("compliance.lead").expect("non-blank"),
            PolicyVersion::new("waiver-policy-3").expect("non-blank"),
            "vendor audit accepted for this exact version",
            expires_after,
        )
        .expect("valid waiver")
    }

    /// E1.6.T2 headline + exit gate (MILESTONES §E1.6 acceptance): waiving
    /// a verification-stage obligation leaves verification `unverified` —
    /// a waiver NEVER converts unverified→verified. Structurally the
    /// waiver appends to the obligation ledger only (it has no reference
    /// to the E1.4 store); observably the store's recorded events stay
    /// byte-identical and its current verification state unchanged.
    #[test]
    fn waiving_a_verification_obligation_never_converts_unverified_to_verified() {
        let subject = imported_subject();
        let mut sink = InMemoryAuditSink::default();
        let mut store = ManagedStateEventStore::new(floor(1));
        store
            .append(
                &mut sink,
                state_event(
                    subject.clone(),
                    ManagedStateChange::Verification {
                        state: VerificationState::Unverified,
                    },
                ),
            )
            .expect("append accepted");
        let recorded_before = store.events().to_vec();

        let mut ledger = ObligationLedger::new();
        ledger.open(open_record(&subject)).expect("opens");
        ledger.waive(waiver_for(&subject, None)).expect("waives");

        let id = obligation_id("ob-billing-credits-verification");
        assert_eq!(ledger.standing()[&id].state, ObligationState::Waived);
        assert_eq!(
            store.current_state()[&subject].verification,
            RecordedDimension::Recorded(VerificationState::Unverified),
            "a waiver never converts unverified to verified"
        );
        assert_eq!(
            store.events(),
            recorded_before.as_slice(),
            "waiver application must leave every recorded E1.4 event byte-identical"
        );
    }

    /// Two real minted versions from one import — for exact-binding
    /// tests needing a version the obligation is NOT bound to
    /// (`ManagedVersionId` is workspace-minted, never fabricated).
    fn imported_subjects() -> (StateEventSubject, StateEventSubject) {
        let mut workspace =
            ManagedWorkspace::new(WorkspaceId::new("ws-acme").expect("workspace id is non-blank"));
        let outcome = workspace
            .import_artifact(&artifact(vec![
                knowledge_object("billing.credits", "sha256:aaa"),
                knowledge_object("billing.tax", "sha256:bbb"),
            ]))
            .expect("import accepted");
        let subject = |index: usize| {
            StateEventSubject::bound(
                &workspace,
                &outcome.imported[index].canonical,
                &outcome.imported[index].version_id,
            )
            .expect("an imported version binds")
        };
        (subject(0), subject(1))
    }

    /// Waivers are permission-controlled, justified, and bound to the
    /// exact obligation + version — every unbound or unjustified shape
    /// fails closed.
    #[test]
    fn waivers_require_justification_and_exact_binding() {
        let (subject, other_subject) = imported_subjects();
        assert_eq!(
            ObligationWaiver::new(
                obligation_id("ob-billing-credits-verification"),
                subject.clone(),
                Principal::new("compliance.lead").expect("non-blank"),
                PolicyVersion::new("waiver-policy-3").expect("non-blank"),
                "   ",
                None,
            ),
            Err(ObligationError::InvalidJustification)
        );

        let mut ledger = ObligationLedger::new();
        ledger.open(open_record(&subject)).expect("opens");
        let id = obligation_id("ob-billing-credits-verification");

        let unknown = ObligationWaiver::new(
            obligation_id("ob-unknown"),
            subject.clone(),
            Principal::new("compliance.lead").expect("non-blank"),
            PolicyVersion::new("waiver-policy-3").expect("non-blank"),
            "bound to nothing",
            None,
        )
        .expect("valid waiver shape");
        assert_eq!(
            ledger.waive(unknown),
            Err(ObligationError::UnknownObligation {
                id: obligation_id("ob-unknown")
            })
        );

        let wrong_version = ObligationWaiver::new(
            id.clone(),
            other_subject.clone(),
            Principal::new("compliance.lead").expect("non-blank"),
            PolicyVersion::new("waiver-policy-3").expect("non-blank"),
            "bound to another version",
            None,
        )
        .expect("valid waiver shape");
        assert_eq!(
            ledger.waive(wrong_version),
            Err(ObligationError::WaiverBindingMismatch { id: id.clone() })
        );

        ledger
            .record_state(&id, ObligationState::Satisfied)
            .expect("satisfiable");
        assert_eq!(
            ledger.waive(waiver_for(&subject, None)),
            Err(ObligationError::NotWaivable { id: id.clone() }),
            "only an open obligation is waivable"
        );
    }

    /// The serialized waiver the Cloud cut consumes: exact obligation +
    /// workspace-qualified managed-version subject + principal + policy
    /// binding, a justification, and an optional event-ordinal
    /// time-bound — no wall-clock anywhere.
    #[test]
    fn serialized_waiver_shape_is_pinned_and_validates() {
        let subject = imported_subject();
        let waiver = waiver_for(&subject, Some(EventOrdinal(5)));
        let instance = serde_json::to_value(&waiver).expect("waiver serializes");
        assert_eq!(
            instance,
            json!({
                "obligation_id": "ob-billing-credits-verification",
                "subject": {
                    "canonical": {
                        "workspace_id": "ws-acme",
                        "canonical_id": "mo-1"
                    },
                    "version_id": "mv-1"
                },
                "principal": "compliance.lead",
                "policy_version": "waiver-policy-3",
                "justification": "vendor audit accepted for this exact version",
                "expires_after": 5
            })
        );
        let unbounded = serde_json::to_value(waiver_for(&subject, None)).expect("serializes");
        assert!(
            unbounded.get("expires_after").is_none(),
            "an unbounded waiver serializes no expiry field"
        );

        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/agent/v0/schema/adoc.proof_obligation.v0.schema.json");
        let schema: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).expect("schema is readable"))
                .expect("schema is json");
        let waiver_schema = json!({
            "$defs": schema["$defs"],
            "$ref": "#/$defs/waiver"
        });
        let validator = jsonschema::validator_for(&waiver_schema).expect("subschema compiles");
        let errors: Vec<String> = validator
            .iter_errors(&instance)
            .map(|error| error.to_string())
            .collect();
        assert!(
            errors.is_empty(),
            "waiver schema validation failed:\n{}\ninstance:\n{}",
            errors.join("\n"),
            serde_json::to_string_pretty(&instance).expect("instance pretty prints")
        );
    }

    /// The ledger's event stream is the published carrier for the
    /// record, the waiver, and every state transition (see the schema
    /// root description / registry row) — its serialized shape is pinned
    /// like every other wire shape in this module: the `event` tag, the
    /// three variant strings, and each payload's keys are contract
    /// surface for the E1.6.T4 consumer, never changeable without a red
    /// test.
    #[test]
    fn serialized_event_stream_shape_is_pinned() {
        let subject = imported_subject();
        let mut ledger = ObligationLedger::new();
        ledger
            .open(record(
                &subject,
                "ob-evidence",
                ObligationStage::Verification,
                None,
            ))
            .expect("opens");
        ledger
            .record_state(&obligation_id("ob-evidence"), ObligationState::Satisfied)
            .expect("records");
        ledger
            .open(record(
                &subject,
                "ob-waived",
                ObligationStage::Approval,
                None,
            ))
            .expect("opens");
        ledger
            .waive(waiver(&subject, "ob-waived", Some(EventOrdinal(5))))
            .expect("waives");

        let subject_json = json!({
            "canonical": { "workspace_id": "ws-acme", "canonical_id": "mo-1" },
            "version_id": "mv-1"
        });
        assert_eq!(
            serde_json::to_value(ledger.events()).expect("events serialize"),
            json!([
                {
                    "event": "opened",
                    "record": {
                        "schema_version": "adoc.proof_obligation.v0",
                        "obligation_id": "ob-evidence",
                        "subject": subject_json,
                        "reason": "requires renewed evidence",
                        "required_evidence": [],
                        "required_at": "verification",
                        "state": "open"
                    }
                },
                {
                    "event": "state_recorded",
                    "obligation_id": "ob-evidence",
                    "state": "satisfied"
                },
                {
                    "event": "opened",
                    "record": {
                        "schema_version": "adoc.proof_obligation.v0",
                        "obligation_id": "ob-waived",
                        "subject": subject_json,
                        "reason": "requires renewed evidence",
                        "required_evidence": [],
                        "required_at": "approval",
                        "state": "open"
                    }
                },
                {
                    "event": "waived",
                    "waiver": {
                        "obligation_id": "ob-waived",
                        "subject": subject_json,
                        "principal": "compliance.lead",
                        "policy_version": "waiver-policy-3",
                        "justification": "vendor audit accepted for this exact version",
                        "expires_after": 5
                    }
                }
            ])
        );
    }

    // ---- E1.6.T3: waiver expiry reopens obligations as blocking ----

    fn record(
        subject: &StateEventSubject,
        id: &str,
        stage: ObligationStage,
        risk: Option<&str>,
    ) -> ProofObligationRecord {
        ProofObligationRecord::open(
            obligation_id(id),
            subject.clone(),
            "requires renewed evidence",
            Vec::new(),
            stage,
            risk.map(str::to_string),
        )
        .expect("valid record")
    }

    fn waiver(
        subject: &StateEventSubject,
        id: &str,
        expires_after: Option<EventOrdinal>,
    ) -> ObligationWaiver {
        ObligationWaiver::new(
            obligation_id(id),
            subject.clone(),
            Principal::new("compliance.lead").expect("non-blank"),
            PolicyVersion::new("waiver-policy-3").expect("non-blank"),
            "vendor audit accepted for this exact version",
            expires_after,
        )
        .expect("valid waiver")
    }

    fn informational_policy(rules: Vec<ObligationPolicyRule>) -> ObligationPolicy {
        ObligationPolicy {
            policy_version: policy_version(),
            rules,
            default_classification: ObligationClassification::Informational,
        }
    }

    /// Adversarial exit (MILESTONES §E1.6 acceptance): an expired waiver
    /// reopens its obligation as BLOCKING — even under a policy whose
    /// data alone would classify it informational. A stale waiver never
    /// authorizes; the reopened obligation fails closed.
    #[test]
    fn expired_waiver_reopens_its_obligation_as_blocking() {
        let subject = imported_subject();
        let id = obligation_id("ob-approval-attestation");
        let mut ledger = ObligationLedger::new();
        ledger
            .open(record(
                &subject,
                "ob-approval-attestation",
                ObligationStage::Approval,
                None,
            ))
            .expect("opens");
        ledger
            .waive(waiver(
                &subject,
                "ob-approval-attestation",
                Some(EventOrdinal(5)),
            ))
            .expect("waives");
        let policy = informational_policy(Vec::new());

        let held = ledger
            .assess(&id, &policy, EventOrdinal(5))
            .expect("assessed");
        assert_eq!(held.state, ObligationState::Waived);
        assert!(
            ledger
                .approval_blockers(&policy, EventOrdinal(5))
                .is_empty()
        );

        let lapsed = ledger
            .assess(&id, &policy, EventOrdinal(6))
            .expect("assessed");
        assert_eq!(lapsed.state, ObligationState::Open, "expiry reopens");
        assert_eq!(
            lapsed.classification,
            ObligationClassification::Blocking,
            "a stale waiver never authorizes — reopened obligations block regardless of policy"
        );
        assert_eq!(ledger.approval_blockers(&policy, EventOrdinal(6)), vec![id]);
    }

    /// §K8: `approval_required` blocks ONLY obligations explicitly
    /// required before gate passage — one fixture branch per stage. All
    /// other obligations may permit merge while the E1.4 store honestly
    /// reports unverified / pending-effectivity / sync-blocked, and the
    /// high-risk agent-action obligation stays for its own stage.
    #[test]
    fn approval_gate_blocks_only_gate_stage_blocking_obligations() {
        let subject = imported_subject();
        let mut sink = InMemoryAuditSink::default();
        let mut store = ManagedStateEventStore::new(floor(1));
        for change in [
            ManagedStateChange::Verification {
                state: VerificationState::Unverified,
            },
            ManagedStateChange::Effectivity {
                state: EffectivityState::Pending,
            },
            ManagedStateChange::Synchronization {
                connector: ConnectorId::new("jira").expect("non-blank"),
                state: SynchronizationState::WritebackFailed,
                required_before_effective: false,
            },
        ] {
            store
                .append(&mut sink, state_event(subject.clone(), change))
                .expect("append accepted");
        }

        let mut ledger = ObligationLedger::new();
        for (id, stage, risk) in [
            ("ob-gate", ObligationStage::Approval, Some("high")),
            ("ob-gate-note", ObligationStage::Approval, Some("low")),
            ("ob-verify", ObligationStage::Verification, None),
            ("ob-effect", ObligationStage::Effectivity, None),
            ("ob-sync", ObligationStage::ConnectorSynchronization, None),
            ("ob-agent", ObligationStage::AgentAction, Some("high")),
        ] {
            ledger
                .open(record(&subject, id, stage, risk))
                .expect("opens");
        }
        // Every stage has a blocking rule for its own gate — but only
        // the APPROVAL-stage blocking obligation blocks approval_required.
        let policy = informational_policy(vec![
            rule(
                ObligationStage::Approval,
                Some("high"),
                None,
                ObligationClassification::Blocking,
            ),
            rule(
                ObligationStage::Verification,
                None,
                None,
                ObligationClassification::Blocking,
            ),
            rule(
                ObligationStage::Effectivity,
                None,
                None,
                ObligationClassification::Blocking,
            ),
            rule(
                ObligationStage::ConnectorSynchronization,
                None,
                None,
                ObligationClassification::Blocking,
            ),
            rule(
                ObligationStage::AgentAction,
                Some("high"),
                None,
                ObligationClassification::Blocking,
            ),
        ]);

        assert_eq!(
            ledger.approval_blockers(&policy, EventOrdinal(3)),
            vec![obligation_id("ob-gate")],
            "only the explicitly gate-stage blocking obligation blocks approval"
        );
        // The merge the other branches permit leaves state honest, never
        // upgraded: unverified / pending-effectivity / sync-blocked stand.
        let state = &store.current_state()[&subject];
        assert_eq!(
            state.verification,
            RecordedDimension::Recorded(VerificationState::Unverified)
        );
        assert_eq!(
            state.effectivity,
            RecordedDimension::Recorded(EffectivityState::Pending)
        );
        assert_eq!(
            state.synchronization[&ConnectorId::new("jira").expect("non-blank")].state,
            SynchronizationState::WritebackFailed
        );
        // The high-risk agent-action obligation still blocks ITS stage.
        assert_eq!(
            ledger
                .assess(&obligation_id("ob-agent"), &policy, EventOrdinal(3))
                .expect("assessed")
                .classification,
            ObligationClassification::Blocking
        );
    }

    /// V10.2.5 acceptance, tied to the E1.4 digest chain: state-only and
    /// approval events never alter recorded envelope bytes and never
    /// upgrade a recorded outcome — obligation activity included.
    #[test]
    fn state_only_and_approval_events_never_alter_recorded_bytes_or_outcomes() {
        let subject = imported_subject();
        let mut sink = InMemoryAuditSink::default();
        let mut store = ManagedStateEventStore::new(floor(1));
        for change in [
            ManagedStateChange::Verification {
                state: VerificationState::Unverified,
            },
            ManagedStateChange::Effectivity {
                state: EffectivityState::Pending,
            },
        ] {
            store
                .append(&mut sink, state_event(subject.clone(), change))
                .expect("append accepted");
        }
        let recorded: Vec<(String, String)> = store
            .events()
            .iter()
            .map(|r| (r.event_bytes.clone(), r.digest.clone()))
            .collect();

        // An approval event plus a full round of obligation activity.
        store
            .append(
                &mut sink,
                state_event(
                    subject.clone(),
                    ManagedStateChange::Governance {
                        state: GovernanceState::Approved,
                    },
                ),
            )
            .expect("append accepted");
        let mut ledger = ObligationLedger::new();
        ledger.open(open_record(&subject)).expect("opens");
        ledger
            .waive(waiver_for(&subject, Some(EventOrdinal(2))))
            .expect("waives");
        let policy = informational_policy(Vec::new());
        let id = obligation_id("ob-billing-credits-verification");
        ledger
            .assess(&id, &policy, EventOrdinal(9))
            .expect("assessed");

        for (index, (event_bytes, digest)) in recorded.iter().enumerate() {
            assert_eq!(
                &store.events()[index].event_bytes,
                event_bytes,
                "recorded event bytes must stay byte-identical"
            );
            assert_eq!(
                &store.events()[index].digest,
                digest,
                "the digest chain over the recorded prefix must stand"
            );
        }
        // No upgraded outcome: the historical answer at the old T is
        // unchanged, and approval touched governance only.
        let historical = store.reconstruct_through([subject.clone()], EventOrdinal(1));
        assert_eq!(
            historical[&subject].verification,
            RecordedDimension::Recorded(VerificationState::Unverified)
        );
        let current = &store.current_state()[&subject];
        assert_eq!(
            current.governance,
            RecordedDimension::Recorded(GovernanceState::Approved)
        );
        assert_eq!(
            current.verification,
            RecordedDimension::Recorded(VerificationState::Unverified),
            "approval never upgrades a recorded verification outcome"
        );
    }

    fn foreign_workspace_subject() -> StateEventSubject {
        let mut ws = ManagedWorkspace::new(WorkspaceId::new("ws-other").expect("non-blank"));
        let outcome = ws
            .import_artifact(&artifact(vec![knowledge_object(
                "other.claim",
                "sha256:ccc",
            )]))
            .expect("import accepted");
        StateEventSubject::bound(
            &ws,
            &outcome.imported[0].canonical,
            &outcome.imported[0].version_id,
        )
        .expect("an imported version binds")
    }

    /// `ManagedVersionId` is workspace-minted (`mv-N` recurs in every
    /// workspace), so exact binding must compare the whole
    /// workspace-qualified subject: a waiver built against another
    /// workspace's equal-numbered version fails closed.
    #[test]
    fn waivers_from_another_workspace_fail_closed_despite_equal_version_ids() {
        let subject = imported_subject();
        let foreign = foreign_workspace_subject();
        let subject_json = serde_json::to_value(&subject).expect("subject serializes");
        let foreign_json = serde_json::to_value(&foreign).expect("subject serializes");
        assert_eq!(
            subject_json["version_id"], foreign_json["version_id"],
            "both mint mv-1"
        );
        assert_ne!(subject_json["canonical"], foreign_json["canonical"]);
        let mut ledger = ObligationLedger::new();
        ledger.open(open_record(&subject)).expect("opens");
        let cross = waiver_for(&foreign, None);
        assert_eq!(
            ledger.waive(cross),
            Err(ObligationError::WaiverBindingMismatch {
                id: obligation_id("ob-billing-credits-verification")
            }),
            "a waiver never crosses workspaces"
        );
    }

    /// A present-but-blank rule matcher fails closed — the published
    /// `$defs/classificationRule` pins `minLength: 1` on `risk` and
    /// `action`, and the validating constructor is the only door.
    #[test]
    fn blank_rule_matchers_fail_closed() {
        assert_eq!(
            ObligationPolicyRule::new(
                ObligationStage::Approval,
                Some(String::new()),
                None,
                ObligationClassification::Blocking,
            ),
            Err(ObligationError::InvalidRuleMatcher)
        );
        assert_eq!(
            ObligationPolicyRule::new(
                ObligationStage::AgentAction,
                None,
                Some(" ".to_string()),
                ObligationClassification::Blocking,
            ),
            Err(ObligationError::InvalidRuleMatcher)
        );
    }

    /// `risk` and the rule matchers are match keys compared by exact
    /// equality — a whitespace-padded value would silently miss its
    /// blocking rule and fall OPEN into the default classification, so
    /// padded values are unrepresentable on both sides of the match.
    #[test]
    fn padded_match_keys_are_unrepresentable() {
        assert_eq!(
            ProofObligationRecord::open(
                obligation_id("ob-1"),
                imported_subject(),
                "reason",
                Vec::new(),
                ObligationStage::AgentAction,
                Some("high ".to_string()),
            ),
            Err(ObligationError::InvalidRisk)
        );
        assert_eq!(
            ObligationPolicyRule::new(
                ObligationStage::AgentAction,
                Some(" high".to_string()),
                None,
                ObligationClassification::Blocking,
            ),
            Err(ObligationError::InvalidRuleMatcher)
        );
        assert_eq!(
            ObligationPolicyRule::new(
                ObligationStage::AgentAction,
                None,
                Some("writeback ".to_string()),
                ObligationClassification::Blocking,
            ),
            Err(ObligationError::InvalidRuleMatcher)
        );
    }
}
