//! Reconciliation Decision — the governed adjudication of a
//! reconciliation candidate pair (E1.3; RT-03; MILESTONES §E1.3).
//!
//! A decision is a Governance Event: it binds a closed verb to both
//! parties' exact workspace canonical identity + managed version ID, a
//! principal, and a policy version. All four bindings are non-optional at
//! the type level — a decision authored by model output alone, or without
//! authority context, cannot exist (stop-ship: model-created authority).
//! The record is `adoc.reconciliation_decision.v0` (registered planned in
//! CONTRACT-REGISTRY.md); E4.2's `adoc.governance_event.v0` (cloud) will
//! enclose it as the event payload, carrying occurrence time — the
//! decision record itself has no wall-clock field, which keeps replay
//! deterministic.
//!
//! Recording a decision is the ONLY path that transitions a pair:
//! imports, matching IDs, hashes, titles, or similarity yield candidates
//! at most (RT-03/D36). Recording never rewrites or drops any managed
//! version, Source Binding, or Source Assertion — reconciliation state is
//! derived from the append-only decision log alone.

use std::collections::BTreeMap;

use serde::Serialize;

use super::managed::{ManagedVersionId, WorkspaceCanonicalIdentity};

/// The closed decision verb set (MILESTONES §E1.3). Direction: the
/// `subject` party is primary — an alias points at it, it supersedes, it
/// survives a merge; the `counterpart` is the affected party.
/// `keep_distinct` is symmetric.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReconciliationVerb {
    KeepDistinct,
    LinkAlias,
    Supersede,
    MergeRehome,
}

/// The deciding authority. Non-blank without surrounding whitespace, never
/// normalized (same fail-closed posture as `WorkspaceId`): a blank
/// principal would be an unprincipaled Governance Event, which must not
/// exist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub(crate) struct Principal(String);

impl Principal {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, ReconciliationDecisionError> {
        let value = value.into();
        if value.is_empty() || value.trim() != value {
            Err(ReconciliationDecisionError::InvalidPrincipal)
        } else {
            Ok(Self(value))
        }
    }
}

/// The exact version of the policy the decision was taken under. Same
/// validation posture as [`Principal`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub(crate) struct PolicyVersion(String);

impl PolicyVersion {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, ReconciliationDecisionError> {
        let value = value.into();
        if value.is_empty() || value.trim() != value {
            Err(ReconciliationDecisionError::InvalidPolicyVersion)
        } else {
            Ok(Self(value))
        }
    }
}

/// One party of a decision: a Managed Object named by workspace canonical
/// identity, bound to the exact managed version the decision adjudicated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct DecisionParty {
    pub(crate) canonical: WorkspaceCanonicalIdentity,
    pub(crate) version_id: ManagedVersionId,
}

/// The typed decision record (`adoc.reconciliation_decision.v0`). Fields
/// are private: [`ReconciliationDecision::new`] is the only constructor,
/// and it requires every binding — verb, both exact-version parties,
/// principal, policy version — so an unbound decision is unconstructible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ReconciliationDecision {
    verb: ReconciliationVerb,
    subject: DecisionParty,
    counterpart: DecisionParty,
    principal: Principal,
    policy_version: PolicyVersion,
}

impl ReconciliationDecision {
    /// Construct a fully bound decision. Rejects a decision naming one
    /// object on both sides — there is no pair to adjudicate.
    pub(crate) fn new(
        verb: ReconciliationVerb,
        subject: DecisionParty,
        counterpart: DecisionParty,
        principal: Principal,
        policy_version: PolicyVersion,
    ) -> Result<Self, ReconciliationDecisionError> {
        if subject.canonical == counterpart.canonical {
            return Err(ReconciliationDecisionError::SelfDecision);
        }
        Ok(Self {
            verb,
            subject,
            counterpart,
            principal,
            policy_version,
        })
    }

    pub(crate) fn verb(&self) -> ReconciliationVerb {
        self.verb
    }

    pub(crate) fn subject(&self) -> &DecisionParty {
        &self.subject
    }

    pub(crate) fn counterpart(&self) -> &DecisionParty {
        &self.counterpart
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ReconciliationDecisionError {
    #[error("principal must be non-blank without surrounding whitespace")]
    InvalidPrincipal,
    #[error("policy version must be non-blank without surrounding whitespace")]
    InvalidPolicyVersion,
    #[error("a decision must name two distinct managed objects")]
    SelfDecision,
    #[error("decision names a managed object unknown to this workspace")]
    UnknownParty,
    #[error("decision is bound to a version that is not the party's latest")]
    StaleVersionBinding,
}

/// The derived reconciliation state: the standing (last-recorded) decision
/// per unordered pair, ordered deterministically by pair key. Derived from
/// the append-only decision log alone — no wall clock, no iteration-order
/// dependence — so replaying the same log over the same import history
/// yields byte-identical state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ReconciliationState {
    pub(crate) standing: Vec<ReconciliationDecision>,
}

impl ReconciliationState {
    pub(crate) fn derive(log: &[ReconciliationDecision]) -> Self {
        let mut standing: BTreeMap<
            (WorkspaceCanonicalIdentity, WorkspaceCanonicalIdentity),
            ReconciliationDecision,
        > = BTreeMap::new();
        for decision in log {
            let a = decision.subject().canonical.clone();
            let b = decision.counterpart().canonical.clone();
            let key = if a <= b { (a, b) } else { (b, a) };
            standing.insert(key, decision.clone());
        }
        Self {
            standing: standing.into_values().collect(),
        }
    }
}
