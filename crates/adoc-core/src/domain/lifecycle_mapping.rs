//! Versioned lifecycle mapping between the flat `.adoc` authored status
//! vocabularies and the six-dimension managed state (E1.5;
//! KNOWLEDGE-MODEL §K5/§K2; ADR-0057 invariant 2).
//!
//! Standalone `.adoc` keeps its released flat status semantics untouched
//! (K1): this module never changes how a kind parses or validates its
//! authored status — it only defines what a flat word MEANS in the
//! managed model, as the registered `adoc.lifecycle_mapping.v0` contract.
//!
//! The two safety rules of §K5, both by construction here:
//!
//! - **Mapping alone never establishes authority.** Applying the mapping
//!   lands E1.4 state events for the mapped target ONLY when the
//!   application carries a typed [`MappingAttestation`] (migration
//!   attestation, source-control attestation, or Cloud Governance
//!   Event). Without one — the default absence, which nothing can forge —
//!   the mapped target is recorded as ADVISORY output and the applied
//!   governance stays `proposed` (candidate floor).
//! - **Approval is never mapped to verification.** A mapping rule's
//!   target ([`MappedManagedState`]) has no verification field at all, so
//!   no rule of any version can name one; every application lands
//!   `verification: unverified` explicitly.
//!
//! The mapping is versioned by `mapping_version`, resolved exact-match —
//! an unknown recorded version fails closed with
//! `schema.unsupported_version`, never coerces. A rule change requires a
//! new version by construction: the serialized version-1 contract is
//! pinned byte-for-byte in the domain tests, so editing a rule under the
//! same version fails the pin.

use std::collections::BTreeMap;

use serde::Serialize;

use super::diagnostic::DiagnosticCode;
use super::managed_state::{
    EffectivityState, GovernanceState, ManagedStateChange, VerificationState,
};
use super::reconciliation::{PolicyVersion, Principal};

/// The registered contract id every serialized instance carries.
pub(crate) const LIFECYCLE_MAPPING_SCHEMA_VERSION: &str = "adoc.lifecycle_mapping.v0";

/// The only mapping rule set shipped so far. A rule change ships as "2":
/// the serialized version-1 contract is pinned in the domain tests, so
/// editing a rule under version "1" fails the pin by construction.
const MAPPING_VERSION_1: &str = "1";

/// The multi-dimension target of one flat authored word. Deliberately
/// carries NO verification field: no flat word — including the authored
/// word `verified` — can name a managed verification state in any mapping
/// rule of any version. Verification requires a verification run, never a
/// word (K5); every application lands `verification: unverified`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct MappedManagedState {
    pub(crate) governance: GovernanceState,
    pub(crate) effectivity: EffectivityState,
}

/// One kind's import mapping: exact-word rules over its released flat
/// vocabulary, an optional open-vocabulary fallback (claim status is an
/// open string), and the target for an absent status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct KindImportMapping {
    /// Exact authored-word rules.
    pub(crate) statuses: BTreeMap<&'static str, MappedManagedState>,
    /// Open-vocabulary fallback. `None` for closed vocabularies: an
    /// unlisted word fails closed (`schema.invalid_status`), never
    /// defaults.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) unlisted_status: Option<MappedManagedState>,
    /// The target when the kind authors no status at all.
    pub(crate) absent: MappedManagedState,
}

/// How much of one §K4 dimension the flat side can carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FlatCarriage {
    /// No flat representation exists at all.
    None,
    /// The flat vocabulary carries some distinctions but not all.
    Partial,
}

/// One machine-readable loss-declaration entry: what the flat side
/// cannot carry of one §K4 dimension (E1.5.T1 exit).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct DimensionLossDeclaration {
    pub(crate) dimension: &'static str,
    pub(crate) carriage: FlatCarriage,
    pub(crate) note: &'static str,
}

/// The versioned `adoc.lifecycle_mapping.v0` contract document: the
/// import mapping for every released kind plus the per-dimension loss
/// declaration. Resolved exact-match by recorded version — Cloud consumes
/// this serialized document as data and never re-implements the rules
/// (E1.5.T3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct LifecycleMappingContract {
    pub(crate) schema_version: &'static str,
    pub(crate) mapping_version: &'static str,
    pub(crate) import_mapping: BTreeMap<&'static str, KindImportMapping>,
    pub(crate) loss_declaration: Vec<DimensionLossDeclaration>,
}

/// A typed attestation authorizing a mapping application to land its
/// mapped target (K5: migration attestation, source-control attestation,
/// or Cloud Governance Event). Authority is granted by the PRESENCE of
/// this typed value — the default absence (`None` at the application
/// boundary) cannot be forged into authority by any authored content.
/// Binding validation (principal authorization, revision existence,
/// event resolution) is the migration/governance slices' concern
/// (E7.1/E4.2); this contract only gates on the typed input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum MappingAttestation {
    /// K2 step 6: an authorized principal accepts the exact repository
    /// revision and qualifying governance history as sufficient
    /// initialization evidence.
    Migration {
        principal: Principal,
        qualification_policy_version: PolicyVersion,
    },
    /// Authority derived from the reviewed source-control revision.
    SourceControl {
        principal: Principal,
        revision: String,
    },
    /// A Cloud Governance Event grants the state (E4.2's
    /// `adoc.governance_event.v0` will enclose the reference).
    CloudGovernanceEvent { event_id: String },
}

/// Whether a mapping application carried authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "authority", rename_all = "snake_case")]
pub(crate) enum MappingAuthority {
    /// No attestation: the mapped target is advisory output only; the
    /// applied governance stays `proposed` (candidate floor).
    Advisory,
    /// The named attestation authorized landing the mapped target.
    Attested { attestation: MappingAttestation },
}

/// One recorded mapping application: the exact contract version it ran
/// under (historical replays resolve by this pin), the advisory mapped
/// target, its authority, and the E1.4 state changes it lands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct MappingApplication {
    /// Pinned per application: replaying a historical record resolves
    /// the contract for exactly this recorded version.
    pub(crate) mapping_version: String,
    pub(crate) kind: String,
    pub(crate) authored_status: Option<String>,
    /// What the contract maps the authored word to — advisory until
    /// attested.
    pub(crate) mapped: MappedManagedState,
    #[serde(flatten)]
    pub(crate) authority: MappingAuthority,
    /// The E1.4 state changes this application lands (append these to
    /// the managed state event log). Without attestation: the candidate
    /// floor. Always includes `verification: unverified` — approval is
    /// never mapped to verification.
    pub(crate) applied: Vec<ManagedStateChange>,
}

/// Why a mapping request was rejected. Every variant maps to a
/// registered wire code via
/// [`LifecycleMappingError::diagnostic_code`] — fail closed, never a
/// silent fallback.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum LifecycleMappingError {
    #[error(
        "lifecycle mapping version {recorded:?} is not supported (supported: \
         {MAPPING_VERSION_1}); versions resolve exact-match and are never coerced"
    )]
    UnsupportedMappingVersion { recorded: String },
    #[error("kind {kind:?} has no lifecycle mapping entry")]
    UnknownKind { kind: String },
    #[error("authored status {status:?} is outside kind {kind:?}'s released flat vocabulary")]
    UnmappedStatus { kind: String, status: String },
}

impl LifecycleMappingError {
    pub(crate) fn diagnostic_code(&self) -> DiagnosticCode {
        match self {
            Self::UnsupportedMappingVersion { .. } => DiagnosticCode::SchemaUnsupportedVersion,
            Self::UnknownKind { .. } => DiagnosticCode::SchemaUnknownKind,
            Self::UnmappedStatus { .. } => DiagnosticCode::SchemaInvalidStatus,
        }
    }
}

impl LifecycleMappingContract {
    /// Resolve the contract for a recorded mapping version — exact
    /// match, fail closed on anything else (playbook decision 12).
    pub(crate) fn for_mapping_version(recorded: &str) -> Result<Self, LifecycleMappingError> {
        match recorded {
            MAPPING_VERSION_1 => Ok(Self::version_1()),
            _ => Err(LifecycleMappingError::UnsupportedMappingVersion {
                recorded: recorded.to_string(),
            }),
        }
    }

    /// Apply the import mapping to one imported object's authored
    /// status. The mapped target lands as E1.4 state changes ONLY when
    /// `attestation` is present; otherwise the application records the
    /// target as advisory and lands the candidate floor. Verification
    /// always lands `unverified` — with or without attestation.
    pub(crate) fn apply_import_mapping(
        &self,
        kind: &str,
        authored_status: Option<&str>,
        attestation: Option<MappingAttestation>,
    ) -> Result<MappingApplication, LifecycleMappingError> {
        let mapped = self.mapped_target(kind, authored_status)?;
        let (authority, landed) = match attestation {
            Some(attestation) => (MappingAuthority::Attested { attestation }, mapped),
            None => (MappingAuthority::Advisory, CANDIDATE_FLOOR),
        };
        Ok(MappingApplication {
            mapping_version: self.mapping_version.to_string(),
            kind: kind.to_string(),
            authored_status: authored_status.map(str::to_string),
            mapped,
            authority,
            applied: vec![
                ManagedStateChange::Governance {
                    state: landed.governance,
                },
                ManagedStateChange::Effectivity {
                    state: landed.effectivity,
                },
                ManagedStateChange::Verification {
                    state: VerificationState::Unverified,
                },
            ],
        })
    }

    fn mapped_target(
        &self,
        kind: &str,
        authored_status: Option<&str>,
    ) -> Result<MappedManagedState, LifecycleMappingError> {
        let entry =
            self.import_mapping
                .get(kind)
                .ok_or_else(|| LifecycleMappingError::UnknownKind {
                    kind: kind.to_string(),
                })?;
        match authored_status {
            None => Ok(entry.absent),
            Some(word) => entry
                .statuses
                .get(word)
                .copied()
                .or(entry.unlisted_status)
                .ok_or_else(|| LifecycleMappingError::UnmappedStatus {
                    kind: kind.to_string(),
                    status: word.to_string(),
                }),
        }
    }

    /// The version-1 rule set over the released flat vocabularies. The
    /// vocabularies themselves live in the kind aggregates and are
    /// untouched (K1) — these rules only assign managed meaning.
    fn version_1() -> Self {
        let floor = CANDIDATE_FLOOR;
        let adopted = MappedManagedState {
            governance: GovernanceState::Approved,
            effectivity: EffectivityState::Effective,
        };
        let retired = MappedManagedState {
            governance: GovernanceState::Approved,
            effectivity: EffectivityState::Expired,
        };
        let closed = |words: &[(&'static str, MappedManagedState)]| KindImportMapping {
            statuses: words.iter().copied().collect(),
            unlisted_status: None,
            absent: floor,
        };
        let statusless = closed(&[]);
        let lifecycle_status = closed(&[
            ("draft", floor),
            ("verified", adopted),
            ("deprecated", retired),
        ]);

        let import_mapping: BTreeMap<&'static str, KindImportMapping> = [
            (
                "claim",
                KindImportMapping {
                    // Only the special word `verified` reads as adopted —
                    // and even it never grants managed verification.
                    statuses: [("verified", adopted)].into_iter().collect(),
                    // Claim status is an open string: any other word is
                    // valid flat authoring and maps to the candidate
                    // floor, never to approval.
                    unlisted_status: Some(floor),
                    absent: floor,
                },
            ),
            (
                "decision",
                closed(&[("proposed", floor), ("accepted", adopted)]),
            ),
            (
                "policy",
                closed(&[
                    ("proposed", floor),
                    ("active", adopted),
                    ("archived", retired),
                    (
                        "revoked",
                        MappedManagedState {
                            governance: GovernanceState::Revoked,
                            effectivity: EffectivityState::Expired,
                        },
                    ),
                ]),
            ),
            ("example", lifecycle_status.clone()),
            ("procedure", lifecycle_status.clone()),
            ("api", lifecycle_status),
            (
                "contradiction",
                closed(&[
                    ("unresolved", floor),
                    ("resolved", adopted),
                    (
                        "dismissed",
                        MappedManagedState {
                            governance: GovernanceState::Rejected,
                            effectivity: EffectivityState::Pending,
                        },
                    ),
                ]),
            ),
            ("observation", closed(&[("observed", floor)])),
            (
                "question",
                closed(&[("open", floor), ("answered", adopted)]),
            ),
            ("task", closed(&[("open", floor), ("done", adopted)])),
            ("glossary", statusless.clone()),
            ("source", statusless.clone()),
            ("warning", statusless.clone()),
            ("constraint", statusless.clone()),
            ("agent_instruction", statusless),
        ]
        .into_iter()
        .collect();

        Self {
            schema_version: LIFECYCLE_MAPPING_SCHEMA_VERSION,
            mapping_version: MAPPING_VERSION_1,
            import_mapping,
            loss_declaration: vec![
                DimensionLossDeclaration {
                    dimension: "governance",
                    carriage: FlatCarriage::Partial,
                    note: "flat words carry proposed/adopted and policy revocation; \
                           rejection has no authored word in most vocabularies",
                },
                DimensionLossDeclaration {
                    dimension: "verification",
                    carriage: FlatCarriage::Partial,
                    note: "the authored word `verified` never maps to managed \
                           verification — import always lands unverified; only a \
                           recorded verification outcome may render `verified` on \
                           export",
                },
                DimensionLossDeclaration {
                    dimension: "effectivity",
                    carriage: FlatCarriage::Partial,
                    note: "scheduled and suspended have no flat representation",
                },
                DimensionLossDeclaration {
                    dimension: "freshness",
                    carriage: FlatCarriage::None,
                    note: "no flat representation; standalone staleness is derived \
                           at read time, never authored",
                },
                DimensionLossDeclaration {
                    dimension: "integrity",
                    carriage: FlatCarriage::None,
                    note: "no flat representation; standalone contradiction state \
                           is derived, never authored",
                },
                DimensionLossDeclaration {
                    dimension: "synchronization",
                    carriage: FlatCarriage::None,
                    note: "per-connector synchronization has no flat representation",
                },
            ],
        }
    }
}

/// The candidate floor every unattested application lands (K2 step 8):
/// governance stays proposed, nothing becomes effective.
const CANDIDATE_FLOOR: MappedManagedState = MappedManagedState {
    governance: GovernanceState::Proposed,
    effectivity: EffectivityState::Pending,
};

#[cfg(test)]
mod tests {
    //! E1.5.T1 exit tests (MILESTONES §E1.5; KNOWLEDGE-MODEL §K5/§K2):
    //! import mapping alone never grants authority, approval never maps
    //! to verification, versions resolve exact-match.

    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;
    use crate::domain::diagnostic::DiagnosticCode;
    use crate::domain::graph::{
        GraphArtifactDocument, GraphKnowledgeObjectNode, GraphNode, GraphRelations,
        GraphRepositoryIdentity, GraphSourceSpan,
    };
    use crate::domain::knowledge_object::block_kind_names;
    use crate::domain::managed::{ManagedWorkspace, WorkspaceId};
    use crate::domain::managed_state::{
        AuditRecord, AuditSink, AuditSinkError, EffectivityState, EventEmitter, GovernanceState,
        ManagedStateChange, ManagedStateEvent, ManagedStateEventStore, RecordedDimension,
        RetentionFloor, StateEventSubject, VerificationState,
    };
    use crate::domain::reconciliation::{PolicyVersion, Principal};

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

    fn knowledge_object(
        id: &str,
        kind: &str,
        status: Option<&str>,
        content_hash: &str,
    ) -> GraphKnowledgeObjectNode {
        GraphKnowledgeObjectNode {
            id: id.to_string(),
            kind: kind.to_string(),
            content_hash: content_hash.to_string(),
            status: status.map(str::to_string),
            severity: None,
            trust: None,
            body: "Refunds require finance approval.".to_string(),
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

    fn contract() -> LifecycleMappingContract {
        LifecycleMappingContract::for_mapping_version("1").expect("version 1 is supported")
    }

    fn migration_attestation() -> MappingAttestation {
        MappingAttestation::Migration {
            principal: Principal::new("governance-lead@acme").expect("non-blank"),
            qualification_policy_version: PolicyVersion::new("migration-qualification-1")
                .expect("non-blank"),
        }
    }

    /// Append an application's changes to the E1.4 log, recording the
    /// mapping version as the event's policy version — the per-application
    /// pin historical replay resolves the contract by.
    fn land(
        store: &mut ManagedStateEventStore,
        subject: &StateEventSubject,
        application: &MappingApplication,
    ) {
        let mut sink = InMemoryAuditSink::default();
        for change in &application.applied {
            store
                .append(
                    &mut sink,
                    ManagedStateEvent {
                        subject: subject.clone(),
                        change: change.clone(),
                        emitter: EventEmitter::new("cloud.import_mapper").expect("non-blank"),
                        policy_version: PolicyVersion::new(format!(
                            "{LIFECYCLE_MAPPING_SCHEMA_VERSION}/{}",
                            application.mapping_version
                        ))
                        .expect("non-blank"),
                        corrects: None,
                    },
                )
                .expect("append accepted");
        }
    }

    /// The slice's headline adversarial acceptance (MILESTONES §E1.5):
    /// a managed import of an object authored `status: active` with NO
    /// attestation lands governance `proposed` — candidate, never
    /// approved, never effective. The mapped target survives only as
    /// advisory output.
    #[test]
    fn authored_active_without_attestation_stays_proposed() {
        let mut workspace = ManagedWorkspace::new(WorkspaceId::new("ws-acme").expect("non-blank"));
        let outcome = workspace
            .import_artifact(&artifact(vec![knowledge_object(
                "billing.refunds",
                "policy",
                Some("active"),
                "sha256:aaa",
            )]))
            .expect("import accepted");
        let imported = &outcome.imported[0];
        let subject = StateEventSubject {
            canonical: imported.canonical.clone(),
            version_id: imported.version_id.clone(),
        };

        let application = contract()
            .apply_import_mapping("policy", Some("active"), None)
            .expect("mapping applies");

        // The contract still says what `active` MEANS…
        assert_eq!(
            application.mapped,
            MappedManagedState {
                governance: GovernanceState::Approved,
                effectivity: EffectivityState::Effective,
            },
            "the advisory mapped target must be recorded"
        );
        assert_eq!(application.authority, MappingAuthority::Advisory);

        // …but without attestation nothing approved/effective is landed.
        let mut store = ManagedStateEventStore::new(RetentionFloor(0));
        land(&mut store, &subject, &application);
        let state = &store.current_state()[&subject];
        assert_eq!(
            state.governance,
            RecordedDimension::Recorded(GovernanceState::Proposed),
            "import mapping alone must never grant approval"
        );
        assert_eq!(
            state.effectivity,
            RecordedDimension::Recorded(EffectivityState::Pending),
            "import mapping alone must never grant effectivity"
        );
        assert_eq!(
            state.verification,
            RecordedDimension::Recorded(VerificationState::Unverified)
        );
        assert!(
            !store.events().iter().any(|record| matches!(
                record.event.change,
                ManagedStateChange::Governance {
                    state: GovernanceState::Approved
                } | ManagedStateChange::Effectivity {
                    state: EffectivityState::Effective
                }
            )),
            "no approved/effective event may exist anywhere in the log"
        );
    }

    /// With a typed attestation the mapped target lands as E1.4 state
    /// events — and verification still lands `unverified` (K5).
    #[test]
    fn attested_application_lands_the_mapped_target() {
        // All three K5 attestation kinds grant authority equally.
        let attestations = [
            migration_attestation(),
            MappingAttestation::SourceControl {
                principal: Principal::new("release-manager@acme").expect("non-blank"),
                revision: "9c4f2ab".to_string(),
            },
            MappingAttestation::CloudGovernanceEvent {
                event_id: "ge-42".to_string(),
            },
        ];
        for attestation in attestations {
            let application = contract()
                .apply_import_mapping("policy", Some("active"), Some(attestation.clone()))
                .expect("mapping applies");
            assert_eq!(
                application.authority,
                MappingAuthority::Attested { attestation }
            );
            assert_eq!(
                application.applied,
                vec![
                    ManagedStateChange::Governance {
                        state: GovernanceState::Approved
                    },
                    ManagedStateChange::Effectivity {
                        state: EffectivityState::Effective
                    },
                    ManagedStateChange::Verification {
                        state: VerificationState::Unverified
                    },
                ]
            );
        }
    }

    /// Every application — any kind, any word, attested or not — lands
    /// exactly one verification change, and it is always `unverified`:
    /// approval is never mapped to verification (K5, exit gate).
    #[test]
    fn every_application_lands_verification_unverified() {
        let contract = contract();
        for (kind, words) in flat_vocabularies() {
            for status in words
                .iter()
                .map(|word| Some(*word))
                .chain(std::iter::once(None))
            {
                for attestation in [None, Some(migration_attestation())] {
                    let application = contract
                        .apply_import_mapping(kind, status, attestation)
                        .expect("mapping applies");
                    let verifications: Vec<_> = application
                        .applied
                        .iter()
                        .filter(|change| matches!(change, ManagedStateChange::Verification { .. }))
                        .collect();
                    assert_eq!(
                        verifications,
                        vec![&ManagedStateChange::Verification {
                            state: VerificationState::Unverified
                        }],
                        "kind {kind} status {status:?} must land exactly \
                         verification:unverified"
                    );
                }
            }
        }
    }

    /// The released flat vocabulary of every kind that authors a status
    /// (brief §6; the kind aggregates are the source of truth and are
    /// untouched by E1.5).
    fn flat_vocabularies() -> Vec<(&'static str, Vec<&'static str>)> {
        vec![
            ("claim", vec!["verified", "draft", "anything-open"]),
            ("decision", vec!["proposed", "accepted"]),
            ("policy", vec!["proposed", "active", "archived", "revoked"]),
            ("example", vec!["draft", "verified", "deprecated"]),
            ("procedure", vec!["draft", "verified", "deprecated"]),
            ("api", vec!["draft", "verified", "deprecated"]),
            ("contradiction", vec!["unresolved", "resolved", "dismissed"]),
            ("observation", vec!["observed"]),
            ("question", vec!["open", "answered"]),
            ("task", vec!["open", "done"]),
            ("glossary", vec![]),
            ("source", vec![]),
            ("warning", vec![]),
            ("constraint", vec![]),
            ("agent_instruction", vec![]),
        ]
    }

    /// The contract covers exactly the released kind set — no kind
    /// unmapped, no rule for a kind that does not exist.
    #[test]
    fn the_contract_covers_every_released_kind_exactly() {
        let contract = contract();
        let mapped: Vec<&str> = contract.import_mapping.keys().copied().collect();
        let mut released = block_kind_names();
        released.sort_unstable();
        assert_eq!(mapped, released);
    }

    #[test]
    fn closed_vocabulary_words_map_and_unknown_words_fail_closed() {
        let contract = contract();
        for (kind, words) in flat_vocabularies() {
            for word in &words {
                contract
                    .apply_import_mapping(kind, Some(word), None)
                    .unwrap_or_else(|error| panic!("kind {kind} status {word} must map: {error}"));
            }
            // Absent status always maps (the candidate floor).
            let absent = contract
                .apply_import_mapping(kind, None, None)
                .expect("absent status maps");
            assert_eq!(
                absent.mapped,
                MappedManagedState {
                    governance: GovernanceState::Proposed,
                    effectivity: EffectivityState::Pending,
                }
            );
        }
        // A word outside a closed vocabulary fails closed…
        let error = contract
            .apply_import_mapping("decision", Some("active"), None)
            .expect_err("decision has no `active`");
        assert_eq!(
            error,
            LifecycleMappingError::UnmappedStatus {
                kind: "decision".to_string(),
                status: "active".to_string(),
            }
        );
        assert_eq!(error.diagnostic_code(), DiagnosticCode::SchemaInvalidStatus);
        // …while claim's open vocabulary falls back to the candidate
        // floor — an arbitrary authored word never reaches approval.
        let open = contract
            .apply_import_mapping("claim", Some("active"), None)
            .expect("claim status is an open string");
        assert_eq!(
            open.mapped,
            MappedManagedState {
                governance: GovernanceState::Proposed,
                effectivity: EffectivityState::Pending,
            },
            "an unlisted claim word must map to the candidate floor"
        );
        // An unknown kind fails closed.
        let error = contract
            .apply_import_mapping("chunk", Some("active"), None)
            .expect_err("unknown kind");
        assert_eq!(error.diagnostic_code(), DiagnosticCode::SchemaUnknownKind);
    }

    /// Version resolution is exact-match (playbook decision 12): an
    /// unknown recorded mapping version fails closed with
    /// `schema.unsupported_version`, never coerces or defaults.
    #[test]
    fn unknown_mapping_version_fails_exact_match_closed() {
        for recorded in ["0", "2", "1.0", "v1", ""] {
            let error = LifecycleMappingContract::for_mapping_version(recorded)
                .expect_err("unsupported version must fail closed");
            assert_eq!(
                error,
                LifecycleMappingError::UnsupportedMappingVersion {
                    recorded: recorded.to_string(),
                }
            );
            assert_eq!(
                error.diagnostic_code(),
                DiagnosticCode::SchemaUnsupportedVersion
            );
        }
    }

    /// The envelope carries its registered contract id and version, and
    /// declares — machine-readably, per §K4 dimension — what the flat
    /// side cannot carry.
    #[test]
    fn the_envelope_names_its_contract_and_declares_loss_per_dimension() {
        let value = serde_json::to_value(contract()).expect("contract serializes");
        assert_eq!(
            value["schema_version"],
            json!(LIFECYCLE_MAPPING_SCHEMA_VERSION)
        );
        assert_eq!(value["mapping_version"], json!("1"));
        let declared: Vec<&str> = value["loss_declaration"]
            .as_array()
            .expect("loss declaration is an array")
            .iter()
            .map(|entry| entry["dimension"].as_str().expect("dimension is named"))
            .collect();
        assert_eq!(
            declared,
            [
                "governance",
                "verification",
                "effectivity",
                "freshness",
                "integrity",
                "synchronization"
            ],
            "every §K4 dimension must have a loss-declaration entry"
        );
        // The verification distinction is declared: the flat word
        // `verified` is not managed verification.
        assert_eq!(value["loss_declaration"][1]["carriage"], json!("partial"));
    }

    /// Serializer ↔ published-schema parity (E1.1 precedent): the
    /// serialized contract validates against
    /// `docs/agent/v0/schema/adoc.lifecycle_mapping.v0.schema.json`.
    #[test]
    fn serialized_contract_validates_against_the_published_schema() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/agent/v0/schema/adoc.lifecycle_mapping.v0.schema.json");
        let schema: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).expect("schema is readable"))
                .expect("schema is json");
        let validator = jsonschema::validator_for(&schema).expect("schema compiles");
        let instance = serde_json::to_value(contract()).expect("contract serializes");
        let errors: Vec<String> = validator
            .iter_errors(&instance)
            .map(|error| error.to_string())
            .collect();
        assert!(
            errors.is_empty(),
            "adoc.lifecycle_mapping.v0 schema validation failed:\n{}\ninstance:\n{}",
            errors.join("\n"),
            serde_json::to_string_pretty(&instance).expect("instance pretty prints")
        );
    }
}
