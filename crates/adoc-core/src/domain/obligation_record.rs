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

use super::managed_state::StateEventSubject;
use super::obligation::ProofObligation;
use super::reconciliation::PolicyVersion;

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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ProofObligationRecord {
    pub(crate) schema_version: &'static str,
    pub(crate) obligation_id: ObligationId,
    /// The exact immutable content version the obligation constrains
    /// (workspace canonical identity + managed version ID, E1.4).
    pub(crate) subject: StateEventSubject,
    pub(crate) reason: String,
    pub(crate) required_evidence: Vec<String>,
    pub(crate) required_at: ObligationStage,
    /// The producer's risk classification of the constrained change,
    /// matched against risk-scoped policy rules. Data, never authority.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) risk: Option<String>,
    pub(crate) state: ObligationState,
}

impl ProofObligationRecord {
    /// Open a new stage-bound obligation. Rejects a blank reason — an
    /// obligation that cannot say what it requires is unrecordable.
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
    /// subject (E1.2) — and the stage the obligation binds to.
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
    pub(crate) stage: ObligationStage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) risk: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) action: Option<String>,
    pub(crate) classification: ObligationClassification,
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

/// Why an obligation write was rejected — fail closed, never a silent
/// fallback.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ObligationError {
    #[error("obligation id must be non-blank without surrounding whitespace")]
    InvalidObligationId,
    #[error("obligation reason must be non-blank")]
    InvalidReason,
    #[error("obligation {id:?} is already open in this ledger")]
    DuplicateObligation { id: ObligationId },
    #[error("obligation {id:?} is not recorded in this ledger")]
    UnknownObligation { id: ObligationId },
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
        if self.opened_record(obligation_id).is_none() {
            return Err(ObligationError::UnknownObligation {
                id: obligation_id.clone(),
            });
        }
        self.events.push(ObligationEvent::StateRecorded {
            obligation_id: obligation_id.clone(),
            state,
        });
        Ok(())
    }

    fn opened_record(&self, id: &ObligationId) -> Option<&ProofObligationRecord> {
        self.events.iter().find_map(|event| match event {
            ObligationEvent::Opened { record } if record.obligation_id == *id => Some(record),
            _ => None,
        })
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
        AuditRecord, AuditSink, AuditSinkError, EffectivityState, EventEmitter, GovernanceState,
        ManagedStateChange, ManagedStateEvent, ManagedStateEventStore, RecordedDimension,
        RetentionFloor, StateEventSubject, VerificationState,
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
        StateEventSubject {
            canonical: imported.canonical.clone(),
            version_id: imported.version_id.clone(),
        }
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
        ObligationPolicyRule {
            stage,
            risk: risk.map(str::to_string),
            action: action.map(str::to_string),
            classification,
        }
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

    /// Blank identities and reasons fail closed.
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
        assert_eq!(
            ProofObligationRecord::open(
                obligation_id("ob-1"),
                imported_subject(),
                "  ",
                Vec::new(),
                ObligationStage::Approval,
                None,
            ),
            Err(ObligationError::InvalidReason)
        );
    }
}
