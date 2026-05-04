use std::collections::BTreeMap;

use crate::domain::artifact::{
    AgentJsonDocument, AgentJsonObject, AgentJsonPage, AgentJsonRelations, AgentJsonSourceSpan,
};
use crate::domain::ast::{BlockAst, PageAst};
use crate::domain::diagnostic::Diagnostic;
use crate::domain::knowledge_object::{KnowledgeObject, claim::Claim};
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

fn claim_to_agent_object(claim: &Claim, page_id: &str) -> AgentJsonObject {
    let span = claim.span();
    let mut fields: BTreeMap<String, String> = BTreeMap::new();
    for (k, v) in claim.fields().iter() {
        fields.insert(k.clone(), v.clone());
    }
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use super::*;
    use crate::domain::ast::PageAst;
    use crate::domain::diagnostic::{SourcePosition, SourceSpan};
    use crate::domain::identity::PageId;
    use crate::domain::knowledge_object::{KnowledgeObject, claim::Claim};
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
        let claim =
            Claim::try_new(id, Some(status), body, fields, span()).expect("test claim is valid");
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
                "verified",
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
        assert_eq!(obj.status, "verified");
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
                "verified",
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
}
