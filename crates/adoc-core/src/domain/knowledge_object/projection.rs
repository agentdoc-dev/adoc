use crate::domain::knowledge_object::{
    KnowledgeObject,
    claim::{
        ClaimStatus, Evidence, OWNER_FIELD, Owner, VERIFIED_AT_FIELD, Verification, VerifiedAt,
    },
    decision::{DECIDED_BY_FIELD, DecidedBy, DecisionStatus},
    warning::WarningSeverity,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KnowledgeObjectMetadata<'a> {
    discriminant: Option<MetadataDiscriminant<'a>>,
    fields: Vec<MetadataField<'a>>,
}

impl<'a> KnowledgeObjectMetadata<'a> {
    pub(crate) fn discriminant(&self) -> Option<MetadataDiscriminant<'a>> {
        self.discriminant
    }

    pub(crate) fn fields(&self) -> &[MetadataField<'a>] {
        &self.fields
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MetadataDiscriminant<'a> {
    ClaimStatus(&'a ClaimStatus),
    DecisionStatus(&'a DecisionStatus),
    WarningSeverity(&'a WarningSeverity),
}

impl<'a> MetadataDiscriminant<'a> {
    pub(crate) fn value_as_str(self) -> &'a str {
        match self {
            Self::ClaimStatus(status) => status.as_str(),
            Self::DecisionStatus(status) => status.as_str(),
            Self::WarningSeverity(severity) => severity.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MetadataField<'a> {
    Stored { key: &'a str, value: &'a str },
    Owner(&'a Owner),
    VerifiedAt(&'a VerifiedAt),
    Evidence(&'a Evidence),
    DecidedBy(&'a DecidedBy),
}

impl MetadataField<'_> {
    pub(crate) fn key(&self) -> &str {
        match self {
            Self::Stored { key, .. } => key,
            Self::Owner(_) => OWNER_FIELD,
            Self::VerifiedAt(_) => VERIFIED_AT_FIELD,
            Self::Evidence(evidence) => evidence.field_key(),
            Self::DecidedBy(_) => DECIDED_BY_FIELD,
        }
    }

    pub(crate) fn value_as_str(&self) -> &str {
        match self {
            Self::Stored { value, .. } => value,
            Self::Owner(owner) => owner.as_str(),
            Self::VerifiedAt(verified_at) => verified_at.as_str(),
            Self::Evidence(evidence) => evidence.value().as_str(),
            Self::DecidedBy(decided_by) => decided_by.as_str(),
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

        let discriminant = match self {
            Self::Claim(claim) => {
                append_verification_fields(&mut fields, claim.verification());
                Some(MetadataDiscriminant::ClaimStatus(claim.status()))
            }
            Self::Decision(decision) => {
                if let Some(verdict) = decision.verdict() {
                    fields.push(MetadataField::DecidedBy(verdict.decided_by()));
                }
                Some(MetadataDiscriminant::DecisionStatus(decision.status()))
            }
            Self::Glossary(_) => None,
            Self::Warning(warning) => {
                Some(MetadataDiscriminant::WarningSeverity(warning.severity()))
            }
        };

        KnowledgeObjectMetadata {
            discriminant,
            fields,
        }
    }
}

fn append_verification_fields<'a>(
    fields: &mut Vec<MetadataField<'a>>,
    verification: Option<&'a Verification>,
) {
    let Some(verification) = verification else {
        return;
    };

    fields.push(MetadataField::Owner(verification.owner()));
    fields.push(MetadataField::VerifiedAt(verification.verified_at()));
    fields.extend(verification.evidence().iter().map(MetadataField::Evidence));
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
    use crate::domain::values::NonEmpty;

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
        let object = KnowledgeObject::Claim(
            Claim::try_new(
                "billing.verified",
                Some("verified"),
                "Credits are verified.",
                BTreeMap::from([("audience".to_string(), "support".to_string())]),
                Some(Verification::new(
                    Owner::try_new("team-billing").expect("owner"),
                    VerifiedAt::try_new("2026-05-06").expect("verified_at"),
                    NonEmpty::from_vec(vec![
                        Evidence::source("ledger").expect("source"),
                        Evidence::test("integration test").expect("test"),
                        Evidence::reviewed_by("architecture").expect("reviewed_by"),
                    ])
                    .expect("non-empty evidence"),
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
        assert_eq!(
            field_entries(&projection),
            vec![
                entry("audience", "support"),
                entry("owner", "team-billing"),
                entry("verified_at", "2026-05-06"),
                entry("source", "ledger"),
                entry("test", "integration test"),
                entry("reviewed_by", "architecture"),
            ]
        );
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
    fn warning_projection_has_severity_discriminant_and_sorted_stored_fields() {
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

        assert_eq!(
            projection
                .discriminant()
                .map(MetadataDiscriminant::value_as_str),
            Some("critical")
        );
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
}
