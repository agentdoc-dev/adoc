use std::path::PathBuf;

use crate::domain::artifact::AgentJsonDocument;
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::retrieval::{LookupIndex, RetrievalRecord};
use crate::infrastructure::artifact::agent_json::read_agent_json_document;

#[derive(Debug, Clone)]
pub struct RetrievalInput {
    pub artifact_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct RetrievalLoadResult {
    pub session: Option<RetrievalSession>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone)]
pub struct RetrievalSession {
    document: AgentJsonDocument,
    lookup: LookupIndex,
}

#[derive(Debug, Clone)]
pub struct ExplainResult {
    pub records: Vec<RetrievalRecord>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn load_retrieval_session(input: RetrievalInput) -> RetrievalLoadResult {
    let document = match read_agent_json_document(&input.artifact_path) {
        Ok(document) => document,
        Err(diagnostics) => {
            return RetrievalLoadResult {
                session: None,
                diagnostics,
            };
        }
    };

    let lookup = match LookupIndex::build(&document.objects) {
        Ok(lookup) => lookup,
        Err(diagnostics) => {
            return RetrievalLoadResult {
                session: None,
                diagnostics,
            };
        }
    };

    RetrievalLoadResult {
        session: Some(RetrievalSession { document, lookup }),
        diagnostics: Vec::new(),
    }
}

pub fn explain_object(session: &RetrievalSession, id: &str) -> ExplainResult {
    if let Some(object) = session.lookup.get(&session.document.objects, id) {
        return ExplainResult {
            records: vec![RetrievalRecord::from(object)],
            diagnostics: Vec::new(),
        };
    }

    ExplainResult {
        records: Vec::new(),
        diagnostics: vec![
            Diagnostic::error(
                DiagnosticCode::RetrievalObjectNotFound,
                format!("Object ID `{id}` was not found in the agent artifact."),
            )
            .with_object_id(id)
            .with_help(
                "Run `adoc build` if the source was changed after the artifact was generated.",
            ),
        ],
    }
}
