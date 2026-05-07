#![allow(dead_code)]

use std::fs;
use std::io;
use std::path::Path;

use crate::domain::artifact::SearchArtifactDocument;
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};

pub(crate) const SUPPORTED_SEARCH_SCHEMA_VERSION: &str = "adoc.search.v0";

pub(crate) fn read_search_artifact_document(
    path: &Path,
) -> Result<SearchArtifactDocument, Vec<Diagnostic>> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) => return Err(vec![read_error_diagnostic(path, error)]),
    };
    let document = match serde_json::from_str::<SearchArtifactDocument>(&contents) {
        Ok(document) => document,
        Err(error) => {
            return Err(vec![
                Diagnostic::error(
                    DiagnosticCode::IoArtifactMalformed,
                    format!("Artifact '{}' is malformed: {error}", path.display()),
                )
                .with_help("Rebuild docs.search.json from the source workspace."),
            ]);
        }
    };

    if document.schema_version != SUPPORTED_SEARCH_SCHEMA_VERSION {
        return Err(vec![
            Diagnostic::error(
                DiagnosticCode::SchemaUnsupportedVersion,
                format!(
                    "Artifact '{}' uses unsupported schema_version '{}'.",
                    path.display(),
                    document.schema_version
                ),
            )
            .with_help(format!(
                "Expected schema_version '{}'.",
                SUPPORTED_SEARCH_SCHEMA_VERSION
            )),
        ]);
    }

    Ok(document)
}

fn read_error_diagnostic(path: &Path, error: io::Error) -> Diagnostic {
    let code = if error.kind() == io::ErrorKind::NotFound {
        DiagnosticCode::IoArtifactMissing
    } else {
        DiagnosticCode::IoArtifactUnreadable
    };
    Diagnostic::error(
        code,
        format!("Unable to read artifact '{}': {error}", path.display()),
    )
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::domain::artifact::{SearchArtifactDocument, SearchEmbedding, SearchModelHeader};
    use crate::domain::diagnostic::DiagnosticCode;
    use crate::infrastructure::artifact::search_json::{
        SUPPORTED_SEARCH_SCHEMA_VERSION, read_search_artifact_document,
    };

    #[test]
    fn read_search_artifact_rejects_unsupported_schema_version() {
        let artifact = tempfile::Builder::new()
            .prefix("adoc-search-")
            .suffix(".json")
            .tempfile()
            .expect("temp artifact can be created");
        fs::write(
            artifact.path(),
            serde_json::json!({
                "schema_version": "adoc.search.v99",
                "model": { "id": "model", "provider": "test", "dim": 2 },
                "agent_artifact_hash": "sha256:agent",
                "embeddings": []
            })
            .to_string(),
        )
        .expect("artifact can be written");

        let diagnostics =
            read_search_artifact_document(artifact.path()).expect_err("version must fail");

        assert_eq!(
            diagnostics[0].code,
            DiagnosticCode::SchemaUnsupportedVersion
        );
    }

    #[test]
    fn search_artifact_pretty_json_round_trips() {
        let artifact = SearchArtifactDocument {
            schema_version: SUPPORTED_SEARCH_SCHEMA_VERSION.to_string(),
            model: SearchModelHeader {
                id: "in-memory".to_string(),
                provider: "test".to_string(),
                dim: 2,
            },
            agent_artifact_hash: "sha256:agent".to_string(),
            embeddings: vec![SearchEmbedding {
                id: "billing.credits".to_string(),
                content_hash: "sha256:content".to_string(),
                vector: vec![1.0, 0.0],
            }],
        };
        let artifact_file = tempfile::Builder::new()
            .prefix("adoc-search-")
            .suffix(".json")
            .tempfile()
            .expect("temp artifact can be created");
        fs::write(
            artifact_file.path(),
            artifact.to_pretty_json().expect("artifact serializes"),
        )
        .expect("artifact can be written");

        let read_back =
            read_search_artifact_document(artifact_file.path()).expect("artifact loads");

        assert_eq!(read_back, artifact);
    }
}
