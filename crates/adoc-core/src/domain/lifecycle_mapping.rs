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
//!   no rule of any version can name one; no application lands any
//!   verification change — the dimension stays a gap until a
//!   verification run records an outcome, and a replayed application can
//!   never overwrite one.
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
    EffectivityState, GovernanceState, ManagedStateChange, ManagedVersionState, RecordedDimension,
    VerificationState,
};
use super::reconciliation::{PolicyVersion, Principal};

/// The registered contract id every serialized instance carries.
pub(crate) const LIFECYCLE_MAPPING_SCHEMA_VERSION: &str = "adoc.lifecycle_mapping.v0";

/// The only mapping rule set shipped so far. A rule change ships as "2":
/// the serialized version-1 contract is pinned in the domain tests, so
/// editing a rule under version "1" fails the pin by construction.
const MAPPING_VERSION_1: &str = "1";

/// The only projection rule set shipped so far. The mapping and
/// projection versions are carried as separate envelope fields but
/// advance in LOCKSTEP: each contract document pairs exactly one
/// mapping version with one projection version, so a rule change on
/// either side ships a new paired document (with its own byte pin) and
/// bumps both versions. Independent movement would make single-version
/// resolution ambiguous — if it is ever wanted, it ships as a
/// paired-resolution contract change, not by bumping one field alone.
const PROJECTION_VERSION_1: &str = "1";

/// The multi-dimension target of one flat authored word. Deliberately
/// carries NO verification field: no flat word — including the authored
/// word `verified` — can name a managed verification state in any mapping
/// rule of any version. Verification requires a verification run, never a
/// word (K5); no application lands any verification change.
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
    pub(crate) projection: ProjectionPolicy,
}

/// The versioned export projection (E1.5.T2): managed multi-dimension
/// state → flat `.adoc` status, inside the same envelope. Lossy by
/// nature — every application returns a machine-readable loss report,
/// never a silently narrowed word.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ProjectionPolicy {
    pub(crate) projection_version: &'static str,
    pub(crate) kinds: BTreeMap<&'static str, KindProjection>,
}

/// One kind's projection: ordered condition rules (first match wins) over
/// an unconditional fallback, so projection is total by construction — no
/// state can fail to render.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct KindProjection {
    pub(crate) rules: Vec<ProjectionRule>,
    /// The word rendered when no rule matches (gaps included). `None`:
    /// the kind authors no status.
    pub(crate) fallback: Option<&'static str>,
}

/// One projection rule: every present condition must hold on the RECORDED
/// dimension (a gap never matches a condition). The `verified` word is
/// only ever emitted by a rule requiring recorded
/// `verification: verified` — pinned by a table-wide test, so approval
/// cannot render as verification in any version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct ProjectionRule {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) governance: Option<GovernanceState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) verification: Option<VerificationState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) effectivity: Option<EffectivityState>,
    pub(crate) status: &'static str,
}

/// One export projection application: the flat word (or `null` for
/// statusless kinds) plus the loss report — one entry per recorded
/// dimension the projected word cannot carry back through this same
/// contract's import mapping. Gaps produce no entries: nothing recorded,
/// nothing dropped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct FlatProjection {
    /// Pinned per application, like the mapping version.
    pub(crate) projection_version: String,
    /// The mapping version the loss report's round-trip recovery ran
    /// under.
    pub(crate) mapping_version: String,
    pub(crate) kind: String,
    pub(crate) status: Option<String>,
    pub(crate) loss_report: Vec<DimensionLoss>,
}

/// One dropped dimension, named machine-readably: what was recorded and
/// what re-importing the projected word would recover (`None`: nothing —
/// either the dimension has no flat carriage at all, or, for
/// verification, import never lands the dimension).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct DimensionLoss {
    pub(crate) dimension: &'static str,
    /// Set only for the per-connector synchronization dimension.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) connector: Option<String>,
    pub(crate) recorded: &'static str,
    pub(crate) recovered: Option<&'static str>,
}

/// Why an attestation payload failed construction. Construction-level
/// only (same posture as [`Principal`]): a blank payload must never
/// reach [`LifecycleMappingContract::apply_import_mapping`] looking like
/// authority.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum AttestationError {
    #[error("source-control revision must be non-blank without surrounding whitespace")]
    InvalidRevision,
    #[error("governance event id must be non-blank without surrounding whitespace")]
    InvalidGovernanceEventId,
}

/// The reviewed source-control revision an attestation derives authority
/// from. Non-blank without surrounding whitespace (same fail-closed
/// posture as [`Principal`]); whether the revision EXISTS is E7.1's
/// binding validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub(crate) struct Revision(String);

impl Revision {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, AttestationError> {
        let value = value.into();
        if value.is_empty() || value.trim() != value {
            Err(AttestationError::InvalidRevision)
        } else {
            Ok(Self(value))
        }
    }
}

/// The Cloud Governance Event an attestation references. Non-blank
/// without surrounding whitespace (same fail-closed posture as
/// [`Principal`]); whether the event RESOLVES is E4.2's binding
/// validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub(crate) struct GovernanceEventId(String);

impl GovernanceEventId {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, AttestationError> {
        let value = value.into();
        if value.is_empty() || value.trim() != value {
            Err(AttestationError::InvalidGovernanceEventId)
        } else {
            Ok(Self(value))
        }
    }
}

/// A typed attestation authorizing a mapping application to land its
/// mapped target (K5: migration attestation, source-control attestation,
/// or Cloud Governance Event). Authority is granted by the PRESENCE of
/// this typed value — the default absence (`None` at the application
/// boundary) cannot be forged into authority by any authored content,
/// and every payload field is a non-blank value object, so an empty
/// husk that merely looks like authority is unconstructible.
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
        revision: Revision,
    },
    /// A Cloud Governance Event grants the state (E4.2's
    /// `adoc.governance_event.v0` will enclose the reference).
    CloudGovernanceEvent { event_id: GovernanceEventId },
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
    /// the managed state event log on FIRST import — re-application
    /// policy is E7.2's concern). Without attestation: the candidate
    /// floor. Never includes a verification change — approval is never
    /// mapped to verification, and a recorded verification outcome
    /// survives any replay.
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
    #[error(
        "lifecycle projection version {recorded:?} is not supported (supported: \
         {PROJECTION_VERSION_1}); versions resolve exact-match and are never coerced"
    )]
    UnsupportedProjectionVersion { recorded: String },
    #[error("kind {kind:?} has no lifecycle mapping entry")]
    UnknownKind { kind: String },
    #[error("authored status {status:?} is outside kind {kind:?}'s released flat vocabulary")]
    UnmappedStatus { kind: String, status: String },
}

impl LifecycleMappingError {
    pub(crate) fn diagnostic_code(&self) -> DiagnosticCode {
        match self {
            Self::UnsupportedMappingVersion { .. } | Self::UnsupportedProjectionVersion { .. } => {
                DiagnosticCode::SchemaUnsupportedVersion
            }
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

    /// Resolve the contract for a recorded projection version — exact
    /// match, fail closed on anything else (playbook decision 12).
    /// Versions advance in lockstep (see [`PROJECTION_VERSION_1`]):
    /// resolving by projection version yields the same paired document
    /// as resolving by its mapping version.
    pub(crate) fn for_projection_version(recorded: &str) -> Result<Self, LifecycleMappingError> {
        match recorded {
            PROJECTION_VERSION_1 => Ok(Self::version_1()),
            _ => Err(LifecycleMappingError::UnsupportedProjectionVersion {
                recorded: recorded.to_string(),
            }),
        }
    }

    /// Project one managed state to its flat `.adoc` status. Total for
    /// every known kind (rules over an unconditional fallback), and
    /// explicit about loss: the report names every recorded dimension the
    /// projected word cannot carry back through this contract's own
    /// import mapping — the round trip is computed, never asserted.
    pub(crate) fn project_export(
        &self,
        kind: &str,
        state: &ManagedVersionState,
    ) -> Result<FlatProjection, LifecycleMappingError> {
        let projection =
            self.projection
                .kinds
                .get(kind)
                .ok_or_else(|| LifecycleMappingError::UnknownKind {
                    kind: kind.to_string(),
                })?;
        let status = projection
            .rules
            .iter()
            .find(|rule| Self::rule_matches(rule, state))
            .map(|rule| Some(rule.status))
            .unwrap_or(projection.fallback);
        // What re-importing the projected word recovers under this same
        // contract; every recorded difference is a named loss.
        let recovered = self.mapped_target(kind, status)?;

        let mut loss_report = Vec::new();
        if let RecordedDimension::Recorded(governance) = state.governance
            && governance != recovered.governance
        {
            loss_report.push(DimensionLoss {
                dimension: "governance",
                connector: None,
                recorded: governance.as_str(),
                recovered: Some(recovered.governance.as_str()),
            });
        }
        // Import never lands a verification change (K5): every recorded
        // verification state is dropped — even when the projected word
        // is `verified`, re-import recovers nothing for the dimension.
        if let RecordedDimension::Recorded(verification) = state.verification {
            loss_report.push(DimensionLoss {
                dimension: "verification",
                connector: None,
                recorded: verification.as_str(),
                recovered: None,
            });
        }
        if let RecordedDimension::Recorded(effectivity) = state.effectivity
            && effectivity != recovered.effectivity
        {
            loss_report.push(DimensionLoss {
                dimension: "effectivity",
                connector: None,
                recorded: effectivity.as_str(),
                recovered: Some(recovered.effectivity.as_str()),
            });
        }
        if let RecordedDimension::Recorded(freshness) = state.freshness {
            loss_report.push(DimensionLoss {
                dimension: "freshness",
                connector: None,
                recorded: freshness.as_str(),
                recovered: None,
            });
        }
        if let RecordedDimension::Recorded(integrity) = state.integrity {
            loss_report.push(DimensionLoss {
                dimension: "integrity",
                connector: None,
                recorded: integrity.as_str(),
                recovered: None,
            });
        }
        for (connector, record) in &state.synchronization {
            loss_report.push(DimensionLoss {
                dimension: "synchronization",
                connector: Some(connector.as_str().to_string()),
                recorded: record.state.as_str(),
                recovered: None,
            });
        }

        Ok(FlatProjection {
            projection_version: self.projection.projection_version.to_string(),
            mapping_version: self.mapping_version.to_string(),
            kind: kind.to_string(),
            status: status.map(str::to_string),
            loss_report,
        })
    }

    /// Every present condition must hold on the recorded dimension; a
    /// gap never matches a condition.
    fn rule_matches(rule: &ProjectionRule, state: &ManagedVersionState) -> bool {
        fn holds<S: PartialEq + Copy>(required: Option<S>, recorded: RecordedDimension<S>) -> bool {
            match required {
                None => true,
                Some(required) => {
                    matches!(recorded, RecordedDimension::Recorded(state) if state == required)
                }
            }
        }
        holds(rule.governance, state.governance)
            && holds(rule.verification, state.verification)
            && holds(rule.effectivity, state.effectivity)
    }

    /// Apply the import mapping to one imported object's authored
    /// status. The mapped target lands as E1.4 state changes ONLY when
    /// `attestation` is present; otherwise the application records the
    /// target as advisory and lands the candidate floor — which REPLACES
    /// the target in both directions (see [`CANDIDATE_FLOOR`]): an
    /// unattested application discards restrictive targets like a
    /// revocation as well as permissive ones. No application
    /// ever lands a verification change — with or without attestation —
    /// so the dimension stays a gap until a verification run records an
    /// outcome, and a replayed application can never overwrite one.
    ///
    /// `applied` carries first-import initialization changes; whether and
    /// how a mapping may be re-applied to an already-governed subject
    /// (catch-up, rollback) is the migration slices' concern (E7.2), not
    /// this contract's.
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

        // Projection rules, first match wins over an unconditional
        // fallback. The `verified` word always requires recorded
        // verification:verified — approval alone renders a
        // non-verification word.
        let rule = |governance: Option<GovernanceState>,
                    verification: Option<VerificationState>,
                    effectivity: Option<EffectivityState>,
                    status: &'static str| ProjectionRule {
            governance,
            verification,
            effectivity,
            status,
        };
        let adopted_rule = |status: &'static str| {
            rule(
                Some(GovernanceState::Approved),
                None,
                Some(EffectivityState::Effective),
                status,
            )
        };
        let verified_rule = rule(
            Some(GovernanceState::Approved),
            Some(VerificationState::Verified),
            Some(EffectivityState::Effective),
            "verified",
        );
        let statusless_projection = KindProjection {
            rules: Vec::new(),
            fallback: None,
        };
        let lifecycle_projection = KindProjection {
            rules: vec![
                verified_rule,
                rule(
                    Some(GovernanceState::Approved),
                    None,
                    Some(EffectivityState::Expired),
                    "deprecated",
                ),
                rule(Some(GovernanceState::Revoked), None, None, "deprecated"),
            ],
            fallback: Some("draft"),
        };
        let kinds: BTreeMap<&'static str, KindProjection> = [
            // Claim projects only claim's own released words: `verified`
            // under recorded verification, else the `draft` fallback with
            // the loss report naming every drop (approval included) —
            // never an invented word like policy's `active`. NOTE: flat
            // authoring couples `status: verified` to a claim
            // verification block (`Claim::from_parts` rejects the word
            // alone); the rule fires only when verification:verified is
            // recorded, and rendering those companion fields from the
            // recorded outcome is export tooling's job (E6.6).
            (
                "claim",
                KindProjection {
                    rules: vec![verified_rule],
                    fallback: Some("draft"),
                },
            ),
            (
                "decision",
                KindProjection {
                    rules: vec![adopted_rule("accepted")],
                    fallback: Some("proposed"),
                },
            ),
            (
                "policy",
                KindProjection {
                    rules: vec![
                        rule(Some(GovernanceState::Revoked), None, None, "revoked"),
                        rule(
                            Some(GovernanceState::Approved),
                            None,
                            Some(EffectivityState::Expired),
                            "archived",
                        ),
                        adopted_rule("active"),
                    ],
                    fallback: Some("proposed"),
                },
            ),
            ("example", lifecycle_projection.clone()),
            ("procedure", lifecycle_projection.clone()),
            ("api", lifecycle_projection),
            (
                "contradiction",
                KindProjection {
                    rules: vec![
                        rule(Some(GovernanceState::Rejected), None, None, "dismissed"),
                        adopted_rule("resolved"),
                    ],
                    fallback: Some("unresolved"),
                },
            ),
            (
                "observation",
                KindProjection {
                    rules: Vec::new(),
                    fallback: Some("observed"),
                },
            ),
            (
                "question",
                KindProjection {
                    rules: vec![adopted_rule("answered")],
                    fallback: Some("open"),
                },
            ),
            (
                "task",
                KindProjection {
                    rules: vec![adopted_rule("done")],
                    fallback: Some("open"),
                },
            ),
            ("glossary", statusless_projection.clone()),
            ("source", statusless_projection.clone()),
            ("warning", statusless_projection.clone()),
            ("constraint", statusless_projection.clone()),
            ("agent_instruction", statusless_projection),
        ]
        .into_iter()
        .collect();

        Self {
            schema_version: LIFECYCLE_MAPPING_SCHEMA_VERSION,
            mapping_version: MAPPING_VERSION_1,
            import_mapping,
            projection: ProjectionPolicy {
                projection_version: PROJECTION_VERSION_1,
                kinds,
            },
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
                           verification — import lands no verification state at \
                           all; only a recorded verification outcome may render \
                           `verified` on export",
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
///
/// The floor REPLACES the mapped target rather than bounding it: an
/// unattested application discards the target in both directions, so a
/// restrictive authored word (`policy: revoked`, `contradiction:
/// dismissed`) also lands as a proposed/pending candidate awaiting
/// review — never as a landed revocation/rejection, which would itself
/// be authored content acting with authority. The discarded target
/// survives as the advisory [`MappingApplication::mapped`].
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
        AuditRecord, AuditSink, AuditSinkError, ConnectorId, ConnectorSyncRecord, EffectivityState,
        EventEmitter, FreshnessState, GovernanceState, IntegrityState, ManagedStateChange,
        ManagedStateEvent, ManagedStateEventStore, ManagedVersionState, RecordedDimension,
        RetentionFloor, StateEventSubject, SynchronizationState, VerificationState,
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
            RecordedDimension::Gap,
            "no verification state may be fabricated by an import"
        );
        assert!(
            !store.events().iter().any(|record| matches!(
                record.event.change,
                ManagedStateChange::Governance {
                    state: GovernanceState::Approved
                } | ManagedStateChange::Effectivity {
                    state: EffectivityState::Effective
                } | ManagedStateChange::Verification { .. }
            )),
            "no approved/effective/verification event may exist anywhere in the log"
        );
    }

    /// With a typed attestation the mapped target lands as E1.4 state
    /// events — and still no verification change (K5).
    #[test]
    fn attested_application_lands_the_mapped_target() {
        // All three K5 attestation kinds grant authority equally.
        let attestations = [
            migration_attestation(),
            MappingAttestation::SourceControl {
                principal: Principal::new("release-manager@acme").expect("non-blank"),
                revision: Revision::new("9c4f2ab").expect("non-blank"),
            },
            MappingAttestation::CloudGovernanceEvent {
                event_id: GovernanceEventId::new("ge-42").expect("non-blank"),
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
                ]
            );
        }
    }

    /// K5's adversarial encoding from the PR-#153 review: an attestation
    /// whose payload is blank — something that looks like authority but
    /// is backed by nothing — must be unconstructible, matching the
    /// non-blank posture of `Principal`/`PolicyVersion`/`EventEmitter`.
    #[test]
    fn blank_attestation_payloads_are_unconstructible() {
        for blank in ["", " ", "  9c4f2ab", "9c4f2ab "] {
            assert_eq!(
                Revision::new(blank).expect_err("blank revision must be rejected"),
                AttestationError::InvalidRevision
            );
            assert_eq!(
                GovernanceEventId::new(blank).expect_err("blank event id must be rejected"),
                AttestationError::InvalidGovernanceEventId
            );
        }
    }

    /// The PR-#153 review scenario: a real verification outcome recorded
    /// after import survives a blind re-application of the mapping —
    /// `applied` carries no verification change, so E1.4's gate-less
    /// append cannot be handed a downgrade.
    #[test]
    fn a_replayed_application_never_overwrites_a_recorded_verification_outcome() {
        let mut workspace = ManagedWorkspace::new(WorkspaceId::new("ws-acme").expect("non-blank"));
        let outcome = workspace
            .import_artifact(&artifact(vec![knowledge_object(
                "billing.refunds",
                "policy",
                Some("active"),
                "sha256:aaa",
            )]))
            .expect("import accepted");
        let subject = StateEventSubject {
            canonical: outcome.imported[0].canonical.clone(),
            version_id: outcome.imported[0].version_id.clone(),
        };
        let mut store = ManagedStateEventStore::new(RetentionFloor::new(1).expect("non-zero"));
        let application = contract()
            .apply_import_mapping("policy", Some("active"), Some(migration_attestation()))
            .expect("mapping applies");
        land(&mut store, &subject, &application);

        // A verification run later records `verified`.
        let mut sink = InMemoryAuditSink::default();
        store
            .append(
                &mut sink,
                ManagedStateEvent {
                    subject: subject.clone(),
                    change: ManagedStateChange::Verification {
                        state: VerificationState::Verified,
                    },
                    emitter: EventEmitter::new("cloud.verification_runtime").expect("non-blank"),
                    policy_version: PolicyVersion::new("verification-policy-1").expect("non-blank"),
                    corrects: None,
                },
            )
            .expect("append accepted");

        // Blind replay of the same mapping application.
        land(&mut store, &subject, &application);
        assert_eq!(
            store.current_state()[&subject].verification,
            RecordedDimension::Recorded(VerificationState::Verified),
            "a mapping replay must never destroy a recorded verification outcome"
        );
    }

    /// Every application — any kind, any word, attested or not — lands
    /// NO verification change at all: the mapped target type has no
    /// verification member, so approval can never be mapped to
    /// verification, and a replayed application can never overwrite a
    /// recorded verification outcome (K5, exit gate).
    #[test]
    fn no_application_lands_a_verification_change() {
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
                    assert!(
                        !application.applied.iter().any(|change| matches!(
                            change,
                            ManagedStateChange::Verification { .. }
                        )),
                        "kind {kind} status {status:?} must land no verification \
                         change — a verification state comes only from a \
                         verification run"
                    );
                }
            }
        }
    }

    /// The released flat vocabulary of every kind, derived from the
    /// contract's own word table — which
    /// [`import_mapping_words_are_pinned_to_the_kind_status_enums`] pins
    /// to the kind status enums both directions, so the kind aggregates
    /// are the source of truth by construction. Claim's open vocabulary
    /// contributes an arbitrary unlisted word as well.
    fn flat_vocabularies() -> Vec<(&'static str, Vec<&'static str>)> {
        let contract = contract();
        contract
            .import_mapping
            .iter()
            .map(|(kind, entry)| {
                let mut words: Vec<&'static str> = entry.statuses.keys().copied().collect();
                if entry.unlisted_status.is_some() {
                    words.push("anything-open");
                }
                (*kind, words)
            })
            .collect()
    }

    /// Both directions of the released-vocabulary pin (PR #153 review):
    /// every import-mapping word is accepted by the kind's own status
    /// parser (a rename fails here), and every parser variant has an
    /// import-mapping rule (an added word fails here) — the kind
    /// aggregates are the source of truth by construction, not by
    /// comment. The `all()` accessors carry exhaustive matches, so a new
    /// enum variant cannot even compile without reaching this table.
    #[test]
    fn import_mapping_words_are_pinned_to_the_kind_status_enums() {
        use crate::domain::knowledge_object::claim::ClaimStatus;
        use crate::domain::knowledge_object::decision::DecisionStatus;
        use crate::domain::knowledge_object::observation::ObservationStatus;
        use crate::domain::knowledge_object::policy::PolicyStatus;
        use crate::domain::knowledge_object::question::QuestionStatus;
        use crate::domain::knowledge_object::task::TaskStatus;
        use crate::domain::value_objects::contradiction_status::ContradictionStatus;
        use crate::domain::value_objects::lifecycle_status::LifecycleStatus;

        fn assert_pinned(
            kind: &str,
            words: &[String],
            expected: Vec<String>,
            parses: impl Fn(&str) -> bool,
        ) {
            for word in words {
                assert!(
                    parses(word),
                    "kind {kind}: the kind's status parser rejects contract word {word:?}"
                );
            }
            let mut expected = expected;
            expected.sort_unstable();
            assert_eq!(
                words,
                &expected[..],
                "kind {kind}: the contract word table drifted from the kind's \
                 status enum — amend the contract (new mapping version) together \
                 with the vocabulary, never one alone"
            );
        }

        let contract = contract();
        for (kind, entry) in &contract.import_mapping {
            let words: Vec<String> = entry.statuses.keys().map(|w| w.to_string()).collect();
            match *kind {
                // Open vocabulary: `verified` is the only exact-word
                // rule; any other non-blank word parses and falls to the
                // open-vocabulary floor.
                "claim" => {
                    assert!(
                        entry.unlisted_status.is_some(),
                        "claim keeps its open-vocabulary fallback"
                    );
                    assert_pinned(kind, &words, vec!["verified".to_string()], |w| {
                        ClaimStatus::try_new(w).is_ok()
                    });
                }
                "policy" => assert_pinned(
                    kind,
                    &words,
                    PolicyStatus::all()
                        .iter()
                        .map(|s| s.as_str().to_string())
                        .collect(),
                    |w| PolicyStatus::try_new(w).is_ok(),
                ),
                "decision" => assert_pinned(
                    kind,
                    &words,
                    DecisionStatus::all()
                        .iter()
                        .map(|s| s.as_str().to_string())
                        .collect(),
                    |w| DecisionStatus::try_new(w).is_ok(),
                ),
                "example" | "procedure" | "api" => assert_pinned(
                    kind,
                    &words,
                    LifecycleStatus::all()
                        .iter()
                        .map(|s| s.as_str().to_string())
                        .collect(),
                    |w| LifecycleStatus::try_new(w).is_ok(),
                ),
                "contradiction" => assert_pinned(
                    kind,
                    &words,
                    ContradictionStatus::all()
                        .iter()
                        .map(|s| s.as_str().to_string())
                        .collect(),
                    |w| ContradictionStatus::try_new(w).is_ok(),
                ),
                "observation" => assert_pinned(
                    kind,
                    &words,
                    ObservationStatus::all()
                        .iter()
                        .map(|s| s.as_str().to_string())
                        .collect(),
                    |w| ObservationStatus::try_new(w).is_ok(),
                ),
                "question" => assert_pinned(
                    kind,
                    &words,
                    QuestionStatus::all()
                        .iter()
                        .map(|s| s.as_str().to_string())
                        .collect(),
                    |w| QuestionStatus::try_new(w).is_ok(),
                ),
                "task" => assert_pinned(
                    kind,
                    &words,
                    TaskStatus::all()
                        .iter()
                        .map(|s| s.as_str().to_string())
                        .collect(),
                    |w| TaskStatus::try_new(w).is_ok(),
                ),
                "glossary" | "source" | "warning" | "constraint" | "agent_instruction" => {
                    assert!(
                        words.is_empty() && entry.unlisted_status.is_none(),
                        "statusless kind {kind} must carry no words and no open fallback"
                    );
                }
                unknown => panic!(
                    "kind {unknown} has no vocabulary pin — a new kind must be \
                     added here alongside its contract entry"
                ),
            }
        }
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

    // ------------------------------------------------------------------
    // E1.5.T2 — export projection (managed → flat) with explicit loss.
    // ------------------------------------------------------------------

    fn recorded_state(
        governance: RecordedDimension<GovernanceState>,
        verification: RecordedDimension<VerificationState>,
        effectivity: RecordedDimension<EffectivityState>,
    ) -> ManagedVersionState {
        ManagedVersionState {
            governance,
            verification,
            effectivity,
            freshness: RecordedDimension::Gap,
            integrity: RecordedDimension::Gap,
            synchronization: BTreeMap::new(),
        }
    }

    /// The T2 headline fixture (MILESTONES §E1.5): a full six-dimension
    /// state round-trips through the flat projection and EVERY dropped
    /// dimension is named machine-readably — all six §K4 names, the
    /// verification drop included even though the projected word is not
    /// `verified`.
    #[test]
    fn round_trip_export_names_every_dropped_dimension() {
        let mut state = recorded_state(
            RecordedDimension::Recorded(GovernanceState::Revoked),
            RecordedDimension::Recorded(VerificationState::Verified),
            RecordedDimension::Recorded(EffectivityState::Suspended),
        );
        state.freshness = RecordedDimension::Recorded(FreshnessState::Stale);
        state.integrity = RecordedDimension::Recorded(IntegrityState::Contradicted);
        state.synchronization.insert(
            ConnectorId::new("jira").expect("non-blank"),
            ConnectorSyncRecord {
                state: SynchronizationState::SourceDiverged,
                required_before_effective: false,
            },
        );

        let projection = contract()
            .project_export("example", &state)
            .expect("projection applies");
        assert_eq!(projection.status.as_deref(), Some("deprecated"));

        let value = serde_json::to_value(&projection).expect("projection serializes");
        assert_eq!(
            value["loss_report"],
            json!([
                {
                    "dimension": "governance",
                    "recorded": "revoked",
                    "recovered": "approved"
                },
                {
                    "dimension": "verification",
                    "recorded": "verified",
                    "recovered": null
                },
                {
                    "dimension": "effectivity",
                    "recorded": "suspended",
                    "recovered": "expired"
                },
                {
                    "dimension": "freshness",
                    "recorded": "stale",
                    "recovered": null
                },
                {
                    "dimension": "integrity",
                    "recorded": "contradicted",
                    "recovered": null
                },
                {
                    "dimension": "synchronization",
                    "connector": "jira",
                    "recorded": "source_diverged",
                    "recovered": null
                }
            ]),
            "every dropped dimension must be named in the loss report"
        );
    }

    /// The slice exit gate: a governance:approved + verification:unverified
    /// object never exports a status implying verification — for any kind.
    #[test]
    fn approved_but_unverified_never_exports_a_verified_status() {
        let contract = contract();
        let state = recorded_state(
            RecordedDimension::Recorded(GovernanceState::Approved),
            RecordedDimension::Recorded(VerificationState::Unverified),
            RecordedDimension::Recorded(EffectivityState::Effective),
        );
        for (kind, _) in flat_vocabularies() {
            let projection = contract
                .project_export(kind, &state)
                .expect("projection applies");
            assert_ne!(
                projection.status.as_deref(),
                Some("verified"),
                "kind {kind}: approval must never render as verification"
            );
        }
    }

    /// By construction over the whole rule table (any version): a rule
    /// rendering the word `verified` must require recorded
    /// verification:verified — no other condition set may emit it.
    #[test]
    fn rendering_verified_requires_recorded_verification_in_every_rule() {
        let contract = contract();
        for (kind, projection) in &contract.projection.kinds {
            for rule in &projection.rules {
                if rule.status == "verified" {
                    assert_eq!(
                        rule.verification,
                        Some(VerificationState::Verified),
                        "kind {kind}: a rule rendering `verified` must require \
                         recorded verification"
                    );
                }
            }
            assert_ne!(
                projection.fallback,
                Some("verified"),
                "kind {kind}: the catch-all must never render `verified`"
            );
        }
    }

    /// The projection policy covers exactly the released kind set, like
    /// the import mapping.
    #[test]
    fn the_projection_covers_every_released_kind_exactly() {
        let contract = contract();
        let projected: Vec<&str> = contract.projection.kinds.keys().copied().collect();
        let mut released = block_kind_names();
        released.sort_unstable();
        assert_eq!(projected, released);
    }

    /// Every projected word round-trips through the same contract's
    /// import mapping — the loss report's recovery comparison is total by
    /// construction.
    #[test]
    fn every_projected_word_is_importable_under_the_same_contract() {
        let contract = contract();
        for (kind, projection) in &contract.projection.kinds {
            for word in projection
                .rules
                .iter()
                .map(|rule| Some(rule.status))
                .chain(std::iter::once(projection.fallback))
            {
                contract
                    .apply_import_mapping(kind, word, None)
                    .unwrap_or_else(|error| {
                        panic!("kind {kind} projected word {word:?} must import: {error}")
                    });
            }
        }
    }

    /// Claim projection words come from claim's own released vocabulary
    /// (PR #153 review): every projected word parses as a claim status,
    /// and no word outside the corpus vocabulary `verified`/`draft` is
    /// invented — an approved-but-unverified claim falls to the `draft`
    /// fallback with the loss report naming the drop, never to a word no
    /// claim surface recognizes.
    #[test]
    fn claim_projection_only_uses_claims_own_released_words() {
        let contract = contract();
        let claim = &contract.projection.kinds["claim"];
        for word in claim
            .rules
            .iter()
            .map(|rule| rule.status)
            .chain(claim.fallback)
        {
            crate::domain::knowledge_object::claim::ClaimStatus::try_new(word).unwrap_or_else(
                |error| {
                    panic!("claim projected word {word:?} must parse as a claim status: {error:?}")
                },
            );
            assert!(
                ["verified", "draft"].contains(&word),
                "claim projection must not invent the word {word:?} — no claim \
                 surface recognizes it"
            );
        }
    }

    /// A statusless kind projects `status: null`; recorded dimensions
    /// still produce loss entries. A dimension nobody recorded (a gap)
    /// is not a loss — nothing existed to drop.
    #[test]
    fn statusless_kinds_and_gaps_project_without_fabricating_loss() {
        let contract = contract();
        let projection = contract
            .project_export(
                "glossary",
                &recorded_state(
                    RecordedDimension::Recorded(GovernanceState::Approved),
                    RecordedDimension::Gap,
                    RecordedDimension::Gap,
                ),
            )
            .expect("projection applies");
        assert_eq!(projection.status, None);
        assert_eq!(
            projection
                .loss_report
                .iter()
                .map(|entry| entry.dimension)
                .collect::<Vec<_>>(),
            ["governance"],
            "only the recorded dimension is reported; gaps fabricate nothing"
        );

        let all_gaps = recorded_state(
            RecordedDimension::Gap,
            RecordedDimension::Gap,
            RecordedDimension::Gap,
        );
        let projection = contract
            .project_export("task", &all_gaps)
            .expect("projection applies");
        assert_eq!(projection.status.as_deref(), Some("open"));
        assert!(projection.loss_report.is_empty());
    }

    /// Projection versions resolve exact-match too (playbook decision
    /// 12) — unknown recorded versions fail closed with
    /// `schema.unsupported_version`.
    #[test]
    fn unknown_projection_version_fails_exact_match_closed() {
        for recorded in ["0", "2", "1.0", "v1", ""] {
            let error = LifecycleMappingContract::for_projection_version(recorded)
                .expect_err("unsupported version must fail closed");
            assert_eq!(
                error,
                LifecycleMappingError::UnsupportedProjectionVersion {
                    recorded: recorded.to_string(),
                }
            );
            assert_eq!(
                error.diagnostic_code(),
                DiagnosticCode::SchemaUnsupportedVersion
            );
        }
        assert_eq!(
            LifecycleMappingContract::for_projection_version("1")
                .expect("version 1 is supported")
                .projection
                .projection_version,
            "1"
        );
    }

    /// The mapping and projection versions advance in lockstep (PR #153
    /// review): both resolvers yield the same paired document, so the
    /// byte pin protects an unambiguous thing — a rule change on either
    /// side ships a new paired document and bumps both versions.
    #[test]
    fn mapping_and_projection_versions_resolve_to_the_same_paired_document() {
        let by_mapping =
            LifecycleMappingContract::for_mapping_version("1").expect("version 1 resolves");
        let by_projection =
            LifecycleMappingContract::for_projection_version("1").expect("version 1 resolves");
        assert_eq!(
            by_mapping, by_projection,
            "the two resolvers must return the same paired contract document"
        );
        assert_eq!(by_mapping.mapping_version, "1");
        assert_eq!(by_mapping.projection.projection_version, "1");
    }

    /// The candidate floor replaces the mapped target in BOTH directions
    /// (PR #153 review): an unattested import of an authored revocation
    /// lands as a proposed/pending candidate — authored content never
    /// lands a revocation with authority — while the advisory `mapped`
    /// target preserves what the word means for the reviewer.
    #[test]
    fn unattested_applications_discard_restrictive_targets_too() {
        let application = contract()
            .apply_import_mapping("policy", Some("revoked"), None)
            .expect("mapping applies");
        assert_eq!(
            application.mapped,
            MappedManagedState {
                governance: GovernanceState::Revoked,
                effectivity: EffectivityState::Expired,
            },
            "the advisory target must preserve the authored meaning"
        );
        assert_eq!(
            application.applied,
            vec![
                ManagedStateChange::Governance {
                    state: GovernanceState::Proposed
                },
                ManagedStateChange::Effectivity {
                    state: EffectivityState::Pending
                },
            ],
            "an unattested revocation lands the candidate floor, not the target"
        );
    }

    /// Historical replay: resolving the contract by a recorded version
    /// and re-applying yields byte-identical records — the version is
    /// pinned per application, so an already-recorded application never
    /// changes meaning when newer rule sets ship.
    #[test]
    fn historical_applications_replay_identically_under_their_recorded_version() {
        let recorded_mapping_version = "1";
        let replay = |version: &str| {
            LifecycleMappingContract::for_mapping_version(version)
                .expect("recorded version resolves")
                .apply_import_mapping("policy", Some("archived"), Some(migration_attestation()))
                .expect("mapping applies")
        };
        let first = replay(recorded_mapping_version);
        let second = replay(recorded_mapping_version);
        assert_eq!(first, second);
        assert_eq!(
            serde_json::to_string(&first).expect("serializes"),
            serde_json::to_string(&second).expect("serializes"),
            "replay must be byte-identical"
        );
        assert_eq!(first.mapping_version, recorded_mapping_version);
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

        // The per-application projection output is part of the same
        // envelope contract (playbook decision 12): validate a real
        // application — the T2 headline fixture — against the schema's
        // projectionOutput definition.
        let mut state = recorded_state(
            RecordedDimension::Recorded(GovernanceState::Revoked),
            RecordedDimension::Recorded(VerificationState::Verified),
            RecordedDimension::Recorded(EffectivityState::Suspended),
        );
        state.freshness = RecordedDimension::Recorded(FreshnessState::Stale);
        state.integrity = RecordedDimension::Recorded(IntegrityState::Contradicted);
        state.synchronization.insert(
            ConnectorId::new("jira").expect("non-blank"),
            ConnectorSyncRecord {
                state: SynchronizationState::SourceDiverged,
                required_before_effective: false,
            },
        );
        let output = contract()
            .project_export("example", &state)
            .expect("projection applies");
        let output_schema = json!({
            "$defs": schema["$defs"],
            "$ref": "#/$defs/projectionOutput"
        });
        let validator = jsonschema::validator_for(&output_schema).expect("subschema compiles");
        let instance = serde_json::to_value(&output).expect("projection serializes");
        let errors: Vec<String> = validator
            .iter_errors(&instance)
            .map(|error| error.to_string())
            .collect();
        assert!(
            errors.is_empty(),
            "projectionOutput schema validation failed:\n{}\ninstance:\n{}",
            errors.join("\n"),
            serde_json::to_string_pretty(&instance).expect("instance pretty prints")
        );
    }

    /// A rule change under version "1" is unshippable by construction:
    /// this pin holds the exact serialized version-1 contract, so any
    /// edit to the rule set fails here until it ships as a new version
    /// with its own pin (MILESTONES §E1.5 acceptance).
    #[test]
    fn the_serialized_version_1_contract_is_pinned() {
        let value = serde_json::to_value(contract()).expect("contract serializes");
        let digest = crate::domain::hashing::sha256_prefixed(
            serde_json::to_string(&value)
                .expect("contract serializes to a string")
                .as_bytes(),
        );
        assert_eq!(
            digest,
            "sha256:9a8dec7c1a548a4cc80b3384db6fa0f6f576913c960526ad07adfde1a3bca721",
            "the version-1 rule set changed: a rule change requires a new \
             mapping/projection version, never an in-place edit. Serialized \
             contract:\n{}",
            serde_json::to_string_pretty(&value).expect("contract pretty prints")
        );
    }
}
