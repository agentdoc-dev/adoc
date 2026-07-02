use crate::domain::graph::GraphEvidence;
use crate::domain::knowledge_object::{
    KnowledgeObject,
    api::{
        ApiStatus, INTERFACE_TYPE_FIELD, METHOD_FIELD, PATH_FIELD as API_PATH_FIELD, SYMBOL_FIELD,
    },
    claim::{
        ClaimStatus, Evidence, OWNER_FIELD, Owner, VERIFIED_AT_FIELD, Verification, VerifiedAt,
    },
    decision::{DECIDED_BY_FIELD, DecidedBy, DecisionStatus},
    example::{CHECKS_FIELD, ExampleStatus, FORMAT_FIELD, LANG_FIELD, SANDBOX_FIELD},
    observation::{OBSERVED_AT_FIELD, ObservationStatus, SAMPLE_SIZE_FIELD},
    policy::PolicyStatus,
    procedure::ProcedureStatus,
};
use crate::domain::value_objects::contradiction_status::ContradictionStatus;
use crate::domain::value_objects::effective_date::EffectiveDate;
use crate::domain::value_objects::review_interval::ReviewInterval;
use crate::domain::value_objects::scope::Scope;
use crate::domain::value_objects::severity::Severity;
use crate::domain::value_objects::trust::Trust;

const KIND_FIELD: &str = "kind";
const PATH_FIELD: &str = "path";
const URL_FIELD: &str = "url";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KnowledgeObjectMetadata<'a> {
    discriminant: Option<MetadataDiscriminant<'a>>,
    /// ADR-0039: the authored severity for `warning`/`constraint`/
    /// `contradiction`. Lives beside the lifecycle discriminant, never in it.
    severity: Option<&'a Severity>,
    /// ADR-0039: the authored trust level for `agent_instruction`.
    trust: Option<&'a Trust>,
    fields: Vec<MetadataField<'a>>,
    /// V5.8: typed evidence entries, separated from the flat `fields` map.
    /// These are converted to `GraphEvidence` and stored in
    /// `GraphKnowledgeObjectNode::evidence` rather than in `fields`.
    evidence: Vec<&'a Evidence>,
}

impl<'a> KnowledgeObjectMetadata<'a> {
    pub(crate) fn discriminant(&self) -> Option<MetadataDiscriminant<'a>> {
        self.discriminant
    }

    pub(crate) fn fields(&self) -> &[MetadataField<'a>] {
        &self.fields
    }

    /// The typed evidence entries for this object's verification. Empty for
    /// non-verified objects. Used by `graph_json.rs` to populate the node's
    /// `evidence` array.
    pub(crate) fn evidence(&self) -> &[&'a Evidence] {
        &self.evidence
    }

    /// ADR-0039: the typed severity for `warning`/`constraint`/
    /// `contradiction`. `None` for every other kind.
    pub(crate) fn severity(&self) -> Option<&'a Severity> {
        self.severity
    }

    /// ADR-0039: the typed trust level for `agent_instruction`. `None` for
    /// every other kind.
    pub(crate) fn trust(&self) -> Option<&'a Trust> {
        self.trust
    }

    /// Convert the typed evidence slice to the `GraphEvidence` wire format.
    ///
    /// `Inline` entries are converted to `GraphEvidence::inline(kind, value)`.
    /// `ObjectRef` entries are converted to `GraphEvidence::object_ref` with a
    /// placeholder kind string (`""`) because the target's kind is not
    /// accessible from this per-object context; the graph assembler resolves
    /// the kind in a post-assembly pass.
    pub(crate) fn graph_evidence(&self) -> Vec<GraphEvidence> {
        self.evidence
            .iter()
            .filter_map(|ev| match ev {
                crate::domain::value_objects::evidence::Evidence::Inline { kind, value } => {
                    Some(GraphEvidence::inline(kind.as_str(), value.as_str()))
                }
                // ObjectRef entries are not sourced from the verification
                // evidence slice — they come from the claim's `evidence_refs`
                // field and are appended by the graph assembler.
                crate::domain::value_objects::evidence::Evidence::ObjectRef(_) => None,
            })
            .collect()
    }
}

/// ADR-0039: lifecycle-only by construction. Kinds without a lifecycle
/// (`warning`, `constraint`, `agent_instruction`, `source`) have no
/// discriminant; their Severity/Trust live in the dedicated projection slots.
// The uniform `Status` postfix is the point: every variant IS a lifecycle status.
#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MetadataDiscriminant<'a> {
    ClaimStatus(&'a ClaimStatus),
    DecisionStatus(&'a DecisionStatus),
    PolicyStatus(&'a PolicyStatus),
    ProcedureStatus(&'a ProcedureStatus),
    ExampleStatus(&'a ExampleStatus),
    /// V5.6: lifecycle status for `contradiction`.
    ContradictionStatus(&'a ContradictionStatus),
    /// V6.5.1: lifecycle status for `api`.
    ApiStatus(&'a ApiStatus),
    /// V6.5.2: lifecycle status for `observation`.
    ObservationStatus(&'a ObservationStatus),
}

impl<'a> MetadataDiscriminant<'a> {
    pub(crate) fn value_as_str(self) -> &'a str {
        match self {
            Self::ClaimStatus(status) => status.as_str(),
            Self::DecisionStatus(status) => status.as_str(),
            Self::PolicyStatus(status) => status.as_str(),
            Self::ProcedureStatus(status) => status.as_str(),
            Self::ExampleStatus(status) => status.as_str(),
            Self::ContradictionStatus(status) => status.as_str(),
            Self::ApiStatus(status) => status.as_str(),
            Self::ObservationStatus(status) => status.as_str(),
        }
    }
}

const EFFECTIVE_AT_FIELD: &str = "effective_at";
const REVIEW_INTERVAL_FIELD: &str = "review_interval";
const SCOPE_FIELD: &str = "scope";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MetadataField<'a> {
    Stored {
        key: &'a str,
        value: &'a str,
    },
    Owner(&'a Owner),
    VerifiedAt(&'a VerifiedAt),
    DecidedBy(&'a DecidedBy),
    /// V5.4: policy `effective_at` — borrows the typed value; `value_as_str`
    /// returns the canonical `YYYY-MM-DD` string stored inside `EffectiveDate`
    /// without allocation.
    EffectiveAt(&'a EffectiveDate),
    /// V5.4: policy `review_interval` — borrows the typed value; `value_as_str`
    /// returns the `[0-9]+d` string stored inside `ReviewInterval`.
    ReviewInterval(&'a ReviewInterval),
    /// V5.5: agent_instruction `scope` glob string.
    Scope(&'a Scope),
}

impl MetadataField<'_> {
    pub(crate) fn key(&self) -> &str {
        match self {
            Self::Stored { key, .. } => key,
            Self::Owner(_) => OWNER_FIELD,
            Self::VerifiedAt(_) => VERIFIED_AT_FIELD,
            Self::DecidedBy(_) => DECIDED_BY_FIELD,
            Self::EffectiveAt(_) => EFFECTIVE_AT_FIELD,
            Self::ReviewInterval(_) => REVIEW_INTERVAL_FIELD,
            Self::Scope(_) => SCOPE_FIELD,
        }
    }

    pub(crate) fn value_as_str(&self) -> &str {
        match self {
            Self::Stored { value, .. } => value,
            Self::Owner(owner) => owner.as_str(),
            Self::VerifiedAt(verified_at) => verified_at.as_str(),
            Self::DecidedBy(decided_by) => decided_by.as_str(),
            Self::EffectiveAt(effective_at) => effective_at.as_str(),
            Self::ReviewInterval(review_interval) => review_interval.as_str(),
            Self::Scope(scope) => scope.as_str(),
        }
    }
}

impl KnowledgeObject {
    pub(crate) fn metadata_projection(&self) -> KnowledgeObjectMetadata<'_> {
        let mut fields: Vec<MetadataField<'_>> = self
            .fields()
            .iter()
            .map(|(key, value)| MetadataField::Stored {
                key: key.as_str(),
                value: value.as_str(),
            })
            .collect();

        let mut evidence: Vec<&Evidence> = Vec::new();
        let mut severity: Option<&Severity> = None;
        let mut trust: Option<&Trust> = None;

        let discriminant = match self {
            Self::Claim(claim) => {
                append_verification_fields(&mut fields, &mut evidence, claim.verification());
                Some(MetadataDiscriminant::ClaimStatus(claim.status()))
            }
            Self::Decision(decision) => {
                if let Some(verdict) = decision.verdict() {
                    fields.push(MetadataField::DecidedBy(verdict.decided_by()));
                    // V5.8 TB3: inline evidence from the accepted verdict goes to
                    // the typed `evidence` vec, NOT into the flat fields map.
                    evidence.extend(verdict.evidence().iter());
                }
                Some(MetadataDiscriminant::DecisionStatus(decision.status()))
            }
            Self::Glossary(_) => None,
            // ADR-0039: warning/constraint have no lifecycle — severity lives
            // in its dedicated slot, never in the status discriminant.
            Self::Warning(warning) => {
                severity = Some(warning.severity());
                None
            }
            Self::Constraint(constraint) => {
                severity = Some(constraint.severity());
                None
            }
            Self::Policy(policy) => {
                fields.push(MetadataField::Owner(policy.owner()));
                fields.push(MetadataField::EffectiveAt(policy.effective_at()));
                if let Some(ri) = policy.review_interval() {
                    fields.push(MetadataField::ReviewInterval(ri));
                }
                Some(MetadataDiscriminant::PolicyStatus(policy.status()))
            }
            Self::Procedure(procedure) => {
                append_verification_fields(&mut fields, &mut evidence, procedure.verification());
                Some(MetadataDiscriminant::ProcedureStatus(procedure.status()))
            }
            Self::Example(example) => {
                append_example_fields(&mut fields, example);
                example.status().map(MetadataDiscriminant::ExampleStatus)
            }
            Self::AgentInstruction(ai) => {
                fields.push(MetadataField::Scope(ai.scope()));
                trust = Some(ai.trust());
                None
            }
            Self::Contradiction(contradiction) => {
                // ADR-0039: severity's sole home is the dedicated slot — the
                // v3 fields["severity"] copy is gone.
                severity = Some(contradiction.severity());
                Some(MetadataDiscriminant::ContradictionStatus(
                    contradiction.status(),
                ))
            }
            Self::Api(api) => {
                // Operation and location project as stored scalars so they
                // enter the hashed graph `fields` map (and the diff/review
                // projection reads them from there).
                if let Some(method) = api.method() {
                    fields.push(MetadataField::Stored {
                        key: METHOD_FIELD,
                        value: method.as_str(),
                    });
                } else if let Some(interface_type) = api.interface_type() {
                    fields.push(MetadataField::Stored {
                        key: INTERFACE_TYPE_FIELD,
                        value: interface_type,
                    });
                }
                if let Some(path) = api.path() {
                    fields.push(MetadataField::Stored {
                        key: API_PATH_FIELD,
                        value: path,
                    });
                } else if let Some(symbol) = api.symbol() {
                    fields.push(MetadataField::Stored {
                        key: SYMBOL_FIELD,
                        value: symbol,
                    });
                }
                append_verification_fields(&mut fields, &mut evidence, api.verification());
                api.status().map(MetadataDiscriminant::ApiStatus)
            }
            Self::Observation(observation) => {
                // Typed optionals project as stored scalars so they enter the
                // hashed graph `fields` map.
                if let Some(sample_size) = observation.sample_size() {
                    fields.push(MetadataField::Stored {
                        key: SAMPLE_SIZE_FIELD,
                        value: sample_size.as_str(),
                    });
                }
                if let Some(observed_at) = observation.observed_at() {
                    fields.push(MetadataField::Stored {
                        key: OBSERVED_AT_FIELD,
                        value: observed_at.as_str(),
                    });
                }
                // Inline `source:` evidence joins the typed evidence vec, so
                // derived evidence_quality applies unchanged (V5 model).
                evidence.extend(observation.source_evidence());
                Some(MetadataDiscriminant::ObservationStatus(
                    observation.status(),
                ))
            }
            Self::Source(source) => {
                // Evidence kind projected as a stored scalar under key "kind".
                fields.push(MetadataField::Stored {
                    key: KIND_FIELD,
                    value: source.kind().as_str(),
                });
                // Path or URL projected under key "path" / "url".
                if let Some(path) = source.path() {
                    fields.push(MetadataField::Stored {
                        key: PATH_FIELD,
                        value: path.as_str(),
                    });
                } else if let Some(url) = source.url() {
                    fields.push(MetadataField::Stored {
                        key: URL_FIELD,
                        value: url.as_str(),
                    });
                }
                // Source has no status discriminant.
                None
            }
        };

        KnowledgeObjectMetadata {
            discriminant,
            severity,
            trust,
            fields,
            evidence,
        }
    }
}

fn append_verification_fields<'a>(
    fields: &mut Vec<MetadataField<'a>>,
    evidence: &mut Vec<&'a Evidence>,
    verification: Option<&'a Verification>,
) {
    let Some(verification) = verification else {
        return;
    };

    fields.push(MetadataField::Owner(verification.owner()));
    fields.push(MetadataField::VerifiedAt(verification.verified_at()));
    // V5.8: evidence goes to the typed `evidence` vec, NOT into the flat fields map.
    evidence.extend(verification.evidence().iter());
}

fn append_example_fields<'a>(
    fields: &mut Vec<MetadataField<'a>>,
    example: &'a crate::domain::knowledge_object::example::Example,
) {
    if let Some(lang) = example.lang() {
        fields.push(MetadataField::Stored {
            key: LANG_FIELD,
            value: lang.as_str(),
        });
    }
    if let Some(format) = example.format() {
        fields.push(MetadataField::Stored {
            key: FORMAT_FIELD,
            value: format,
        });
    }
    if let Some(checks) = example.checks() {
        fields.push(MetadataField::Stored {
            key: CHECKS_FIELD,
            value: checks,
        });
    }
    if let Some(sandbox) = example.sandbox() {
        fields.push(MetadataField::Stored {
            key: SANDBOX_FIELD,
            value: sandbox.as_str(),
        });
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use super::*;
    use crate::domain::diagnostic::{SourcePosition, SourceSpan};
    use crate::domain::knowledge_object::{
        KnowledgeObject,
        claim::{Claim, Evidence, Owner, Verification, VerifiedAt},
        decision::{AcceptedVerdict, DECIDED_BY_FIELD, DecidedBy, Decision},
        glossary::Glossary,
        warning::Warning,
    };

    fn span() -> SourceSpan {
        SourceSpan {
            file: PathBuf::from("test.adoc"),
            start: SourcePosition {
                line: 1,
                column: 1,
                offset: 0,
            },
            end: SourcePosition {
                line: 1,
                column: 12,
                offset: 11,
            },
        }
    }

    fn field_entries(projection: &KnowledgeObjectMetadata<'_>) -> Vec<(String, String)> {
        projection
            .fields()
            .iter()
            .map(|field| (field.key().to_string(), field.value_as_str().to_string()))
            .collect()
    }

    fn entry(key: &str, value: &str) -> (String, String) {
        (key.to_string(), value.to_string())
    }

    #[test]
    fn plain_claim_projection_has_status_and_sorted_stored_fields_only() {
        let object = KnowledgeObject::Claim(
            Claim::try_new(
                "billing.credits",
                Some("plain"),
                "Credits are applied automatically.",
                BTreeMap::from([
                    ("zeta".to_string(), "last".to_string()),
                    ("alpha".to_string(), "first".to_string()),
                ]),
                None,
                span(),
            )
            .expect("valid claim"),
        );

        let projection = object.metadata_projection();

        assert_eq!(
            projection
                .discriminant()
                .map(MetadataDiscriminant::value_as_str),
            Some("plain")
        );
        assert_eq!(
            field_entries(&projection),
            vec![entry("alpha", "first"), entry("zeta", "last")]
        );
    }

    #[test]
    fn verified_claim_projection_appends_typed_verification_fields_after_stored_fields() {
        use crate::domain::value_objects::evidence_kind::EvidenceKind;

        let object = KnowledgeObject::Claim(
            Claim::try_new(
                "billing.verified",
                Some("verified"),
                "Credits are verified.",
                BTreeMap::from([("audience".to_string(), "support".to_string())]),
                Some(Verification::new(
                    Owner::try_new("team-billing").expect("owner"),
                    VerifiedAt::try_new("2026-05-06").expect("verified_at"),
                    vec![
                        Evidence::inline(EvidenceKind::SourceCode, "ledger").expect("source"),
                        Evidence::inline(EvidenceKind::Test, "integration test").expect("test"),
                        Evidence::inline(EvidenceKind::HumanReview, "architecture")
                            .expect("reviewed_by"),
                    ],
                )),
                span(),
            )
            .expect("valid verified claim"),
        );

        let projection = object.metadata_projection();

        assert_eq!(
            projection
                .discriminant()
                .map(MetadataDiscriminant::value_as_str),
            Some("verified")
        );
        // V5.8: evidence is no longer in the flat fields — only stored fields and
        // owner/verified_at appear here.
        assert_eq!(
            field_entries(&projection),
            vec![
                entry("audience", "support"),
                entry("owner", "team-billing"),
                entry("verified_at", "2026-05-06"),
            ]
        );
        // Evidence is in the typed evidence slice.
        let ev = projection.evidence();
        assert_eq!(ev.len(), 3);
        assert_eq!(ev[0].kind(), Some(EvidenceKind::SourceCode));
        assert_eq!(ev[0].value().expect("inline value").as_str(), "ledger");
        assert_eq!(ev[1].kind(), Some(EvidenceKind::Test));
        assert_eq!(ev[2].kind(), Some(EvidenceKind::HumanReview));
    }

    #[test]
    fn accepted_decision_projection_appends_typed_decided_by_after_stored_fields() {
        let object = KnowledgeObject::Decision(
            Decision::try_new(
                "billing.policy",
                Some("accepted"),
                "Use the existing policy.",
                BTreeMap::from([("audience".to_string(), "ops".to_string())]),
                Some(AcceptedVerdict::new(
                    DecidedBy::try_new("architecture").expect("decided_by"),
                    Vec::new(),
                )),
                span(),
            )
            .expect("valid decision"),
        );

        let projection = object.metadata_projection();

        assert_eq!(
            projection
                .discriminant()
                .map(MetadataDiscriminant::value_as_str),
            Some("accepted")
        );
        assert_eq!(
            field_entries(&projection),
            vec![
                entry("audience", "ops"),
                entry(DECIDED_BY_FIELD, "architecture")
            ]
        );
    }

    #[test]
    fn proposed_decision_projection_keeps_decided_by_as_stored_metadata() {
        let object = KnowledgeObject::Decision(
            Decision::try_new(
                "billing.policy",
                Some("proposed"),
                "Consider the policy.",
                BTreeMap::from([(DECIDED_BY_FIELD.to_string(), "architecture".to_string())]),
                None,
                span(),
            )
            .expect("valid decision"),
        );

        let projection = object.metadata_projection();

        assert_eq!(
            projection
                .discriminant()
                .map(MetadataDiscriminant::value_as_str),
            Some("proposed")
        );
        assert_eq!(
            field_entries(&projection),
            vec![entry(DECIDED_BY_FIELD, "architecture")]
        );
    }

    #[test]
    fn warning_projection_is_lifecycle_free_with_dedicated_severity_slot() {
        let object = KnowledgeObject::Warning(
            Warning::try_new(
                "auth.session",
                Some("critical"),
                "Clock skew breaks sessions.",
                BTreeMap::from([
                    ("owner".to_string(), "platform".to_string()),
                    ("audience".to_string(), "sre".to_string()),
                ]),
                span(),
            )
            .expect("valid warning"),
        );

        let projection = object.metadata_projection();

        // ADR-0039: no lifecycle discriminant; severity has its own slot.
        assert_eq!(projection.discriminant(), None);
        assert_eq!(projection.severity().map(|s| s.as_str()), Some("critical"));
        assert_eq!(
            field_entries(&projection),
            vec![entry("audience", "sre"), entry("owner", "platform")]
        );
    }

    #[test]
    fn glossary_projection_has_no_discriminant_and_preserves_sorted_stored_fields() {
        let object = KnowledgeObject::Glossary(
            Glossary::try_new(
                "billing.ledger",
                "A record of billing movements.",
                BTreeMap::from([
                    ("status".to_string(), "draft".to_string()),
                    ("owner".to_string(), "team-billing".to_string()),
                ]),
                span(),
            )
            .expect("valid glossary"),
        );

        let projection = object.metadata_projection();

        assert_eq!(projection.discriminant(), None);
        assert_eq!(
            field_entries(&projection),
            vec![entry("owner", "team-billing"), entry("status", "draft")]
        );
    }

    #[test]
    fn example_projection_appends_typed_fields_and_maps_status_discriminant() {
        use crate::domain::knowledge_object::example::Example;

        let object = KnowledgeObject::Example(
            Example::try_new(
                "auth.credits.example",
                Some("draft"),
                Some("ts"),
                None,
                "const x = 1 + 1;",
                Some("npm test"),
                Some("node-test"),
                BTreeMap::new(),
                span(),
            )
            .expect("valid example"),
        );

        let projection = object.metadata_projection();

        assert_eq!(
            projection
                .discriminant()
                .map(MetadataDiscriminant::value_as_str),
            Some("draft")
        );
        assert_eq!(
            field_entries(&projection),
            vec![
                entry(LANG_FIELD, "ts"),
                entry(CHECKS_FIELD, "npm test"),
                entry(SANDBOX_FIELD, "node-test"),
            ]
        );
    }

    #[test]
    fn example_projection_has_no_discriminant_when_status_absent() {
        use crate::domain::knowledge_object::example::Example;

        let object = KnowledgeObject::Example(
            Example::try_new(
                "auth.credits.example",
                None,
                Some("ts"),
                None,
                "const x = 1 + 1;",
                None,
                None,
                BTreeMap::new(),
                span(),
            )
            .expect("valid example"),
        );

        let projection = object.metadata_projection();

        assert_eq!(projection.discriminant(), None);
        assert_eq!(field_entries(&projection), vec![entry(LANG_FIELD, "ts")]);
    }
}
