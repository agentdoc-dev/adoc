use std::collections::BTreeMap;

use crate::domain::artifact::AgentJsonObject;
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LookupIndex {
    ids: BTreeMap<String, usize>,
}

impl LookupIndex {
    pub(crate) fn build(objects: &[AgentJsonObject]) -> Result<Self, Vec<Diagnostic>> {
        let mut ids = BTreeMap::new();
        let mut diagnostics = Vec::new();

        for (index, object) in objects.iter().enumerate() {
            if ids.insert(object.id.clone(), index).is_some() {
                diagnostics.push(
                    Diagnostic::error(
                        DiagnosticCode::IdDuplicateInArtifact,
                        format!("duplicate Object ID `{}` in agent artifact", object.id),
                    )
                    .with_object_id(object.id.clone())
                    .with_help(
                        "Run `adoc build` to regenerate docs.agent.json from validated AgentDoc Source.",
                    ),
                );
            }
        }

        if diagnostics.is_empty() {
            Ok(Self { ids })
        } else {
            Err(diagnostics)
        }
    }

    pub(crate) fn get<'a>(
        &self,
        objects: &'a [AgentJsonObject],
        id: &str,
    ) -> Option<&'a AgentJsonObject> {
        self.ids.get(id).and_then(|index| objects.get(*index))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::domain::artifact::{AgentJsonRelations, AgentJsonSourceSpan};

    fn object(id: &str) -> AgentJsonObject {
        AgentJsonObject {
            id: id.to_string(),
            kind: "claim".to_string(),
            status: Some("draft".to_string()),
            body: "Body.".to_string(),
            page_id: "team.page".to_string(),
            source_span: AgentJsonSourceSpan {
                path: "docs/page.adoc".to_string(),
                line: 1,
                column: 1,
            },
            fields: BTreeMap::new(),
            relations: AgentJsonRelations::default(),
        }
    }

    #[test]
    fn lookup_index_returns_object_by_id() {
        let objects = vec![object("billing.one"), object("billing.two")];
        let index = LookupIndex::build(&objects).expect("ids are unique");

        let found = index.get(&objects, "billing.two").expect("object found");

        assert_eq!(found.id, "billing.two");
    }

    #[test]
    fn lookup_index_reports_duplicate_ids() {
        let objects = vec![object("billing.same"), object("billing.same")];

        let diagnostics = LookupIndex::build(&objects).expect_err("duplicate id must fail");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::IdDuplicateInArtifact);
        assert_eq!(diagnostics[0].object_id.as_deref(), Some("billing.same"));
    }
}
