use std::collections::BTreeMap;

use crate::domain::artifact::{
    AgentJsonDocument, AgentJsonObject, AgentJsonPage, AgentJsonRelations, AgentJsonSourceSpan,
};
use crate::domain::ast::{BlockAst, PageAst};
use crate::domain::diagnostic::Diagnostic;
use crate::domain::knowledge_object::{
    KnowledgeObject,
    claim::{
        Claim, Evidence, OWNER_FIELD, REVIEWED_BY_FIELD, SOURCE_FIELD, TEST_FIELD,
        VERIFIED_AT_FIELD,
    },
    decision::Decision,
};
use crate::domain::ports::artifact_writer::ArtifactWriter;

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct AgentJsonArtifact;

impl ArtifactWriter for AgentJsonArtifact {
    type Output = AgentJsonDocument;
    fn build(&self, pages: &[PageAst], diagnostics: &[Diagnostic]) -> AgentJsonDocument {
        let mut objects: Vec<AgentJsonObject> = Vec::new();
        for page in pages {
            for block in &page.blocks {
                match block {
                    BlockAst::KnowledgeObject(ko) => match ko.as_ref() {
                        KnowledgeObject::Claim(claim) => {
                            objects.push(claim_to_agent_object(claim, page.id.as_str()));
                        }
                        KnowledgeObject::Decision(decision) => {
                            objects.push(decision_to_agent_object(decision, page.id.as_str()));
                        }
                    },
                    BlockAst::KnowledgeObjectPending(_) => unreachable!(
                        "resolver must replace pending knowledge objects before artifact emission"
                    ),
                    _ => {}
                }
            }
        }
        AgentJsonDocument {
            schema_version: "adoc.agent.v0".to_string(),
            pages: pages.iter().map(AgentJsonPage::from).collect(),
            objects,
            diagnostics: diagnostics.to_vec(),
        }
    }
}

fn decision_to_agent_object(decision: &Decision, page_id: &str) -> AgentJsonObject {
    let span = decision.span();
    AgentJsonObject {
        id: decision.id().as_str().to_string(),
        kind: "decision".to_string(),
        status: decision.status().as_str().to_string(),
        body: decision.body().as_str().to_string(),
        page_id: page_id.to_string(),
        source_span: AgentJsonSourceSpan {
            path: span.file.display().to_string(),
            line: span.start.line,
            column: span.start.column,
        },
        fields: decision
            .fields()
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
        relations: AgentJsonRelations::default(),
    }
}

fn claim_to_agent_object(claim: &Claim, page_id: &str) -> AgentJsonObject {
    let span = claim.span();
    let fields = fields_for_claim(claim);
    AgentJsonObject {
        id: claim.id().as_str().to_string(),
        kind: "claim".to_string(),
        status: claim.status().as_str().to_string(),
        body: claim.body().as_str().to_string(),
        page_id: page_id.to_string(),
        source_span: AgentJsonSourceSpan {
            path: span.file.display().to_string(),
            line: span.start.line,
            column: span.start.column,
        },
        fields,
        relations: AgentJsonRelations::default(),
    }
}

fn fields_for_claim(claim: &Claim) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();

    for (key, value) in claim.fields().iter() {
        fields.insert(key.clone(), value.clone());
    }

    if let Some(verification) = claim.verification() {
        fields.insert(
            OWNER_FIELD.to_string(),
            verification.owner().as_str().to_string(),
        );
        fields.insert(
            VERIFIED_AT_FIELD.to_string(),
            verification.verified_at().as_str().to_string(),
        );
        for evidence in verification.evidence() {
            match evidence {
                Evidence::Source(value) => {
                    fields.insert(SOURCE_FIELD.to_string(), value.as_str().to_string());
                }
                Evidence::Test(value) => {
                    fields.insert(TEST_FIELD.to_string(), value.as_str().to_string());
                }
                Evidence::ReviewedBy(value) => {
                    fields.insert(REVIEWED_BY_FIELD.to_string(), value.as_str().to_string());
                }
            }
        }
    }

    fields
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use super::*;
    use crate::domain::ast::PageAst;
    use crate::domain::diagnostic::{SourcePosition, SourceSpan};
    use crate::domain::identity::PageId;
    use crate::domain::knowledge_object::{
        KnowledgeObject,
        claim::{Claim, Evidence, NonEmpty, Owner, Verification, VerifiedAt},
    };
    use crate::domain::ports::artifact_writer::ArtifactWriter;

    fn span() -> SourceSpan {
        SourceSpan {
            file: PathBuf::from("sample.adoc"),
            start: SourcePosition {
                line: 5,
                column: 1,
                offset: 0,
            },
            end: SourcePosition {
                line: 9,
                column: 2,
                offset: 80,
            },
        }
    }

    fn make_claim(
        id: &str,
        status: &str,
        body: &str,
        fields: BTreeMap<String, String>,
    ) -> BlockAst {
        let verification = (status == "verified").then(|| {
            Verification::new(
                Owner::try_new(fields.get(OWNER_FIELD).expect("owner")).expect("owner"),
                VerifiedAt::try_new(fields.get(VERIFIED_AT_FIELD).expect("verified_at"))
                    .expect("verified_at"),
                NonEmpty::from_vec(vec![
                    Evidence::source(fields.get(SOURCE_FIELD).expect("source")).expect("source"),
                ])
                .expect("non-empty evidence"),
            )
        });
        let claim = Claim::try_new(id, Some(status), body, fields, verification, span())
            .expect("test claim is valid");
        BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Claim(claim)))
    }

    fn make_page(id: &str, blocks: Vec<BlockAst>) -> PageAst {
        PageAst {
            id: PageId::from_string(id).expect("test page id is valid"),
            title: Some("Test Page".to_string()),
            source_path: PathBuf::from("sample.adoc"),
            blocks,
        }
    }

    #[test]
    fn agent_json_artifact_emits_one_object_per_claim() {
        let page = make_page(
            "team.guide",
            vec![make_claim(
                "billing.credits",
                "plain",
                "The system credits users automatically.",
                BTreeMap::new(),
            )],
        );
        let artifact = AgentJsonArtifact;
        let doc = artifact.build(&[page], &[]);

        assert_eq!(doc.objects.len(), 1);
        let obj = &doc.objects[0];
        assert_eq!(obj.id, "billing.credits");
        assert_eq!(obj.kind, "claim");
        assert_eq!(obj.status, "plain");
        assert_eq!(obj.body, "The system credits users automatically.");
        assert_eq!(obj.page_id, "team.guide");
        assert!(
            obj.relations.depends_on.is_empty(),
            "depends_on must be empty"
        );
        assert!(
            obj.relations.supersedes.is_empty(),
            "supersedes must be empty"
        );
        assert!(
            obj.relations.related_to.is_empty(),
            "related_to must be empty"
        );
    }

    #[test]
    fn agent_json_artifact_strips_status_from_fields_map() {
        // The resolver ensures status is not in optional_fields; verify here
        // that a claim with owner field produces exactly one field entry.
        let fields = BTreeMap::from([("owner".to_string(), "team-a".to_string())]);
        let page = make_page(
            "team.guide",
            vec![make_claim(
                "billing.credits",
                "plain",
                "The system credits users automatically.",
                fields,
            )],
        );
        let artifact = AgentJsonArtifact;
        let doc = artifact.build(&[page], &[]);

        assert_eq!(doc.objects.len(), 1);
        let obj = &doc.objects[0];
        assert_eq!(obj.fields.len(), 1, "expected exactly one field entry");
        assert_eq!(obj.fields.get("owner").map(String::as_str), Some("team-a"));
        assert!(
            !obj.fields.contains_key("status"),
            "status must not appear in fields map"
        );
    }

    #[test]
    fn agent_json_artifact_preserves_typed_verified_fields_in_flat_map() {
        let fields = BTreeMap::from([
            (OWNER_FIELD.to_string(), "team-billing".to_string()),
            (VERIFIED_AT_FIELD.to_string(), "2026-05-05".to_string()),
            (SOURCE_FIELD.to_string(), "payments ledger".to_string()),
            ("audience".to_string(), "support".to_string()),
        ]);
        let page = make_page(
            "team.guide",
            vec![make_claim(
                "billing.credits",
                "verified",
                "The system credits users automatically.",
                fields,
            )],
        );
        let artifact = AgentJsonArtifact;
        let doc = artifact.build(&[page], &[]);

        let obj = &doc.objects[0];
        assert_eq!(
            obj.fields.get(OWNER_FIELD).map(String::as_str),
            Some("team-billing")
        );
        assert_eq!(
            obj.fields.get(VERIFIED_AT_FIELD).map(String::as_str),
            Some("2026-05-05")
        );
        assert_eq!(
            obj.fields.get(SOURCE_FIELD).map(String::as_str),
            Some("payments ledger")
        );
        assert_eq!(
            obj.fields.get("audience").map(String::as_str),
            Some("support")
        );
    }
}
