use std::fs;
use std::io;
use std::path::Path;

use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::graph::{GRAPH_ARTIFACT_SCHEMA_VERSION, GraphArtifactDocument};
use crate::domain::ports::artifact_reader::{ArtifactReadError, ArtifactReader};
use crate::infrastructure::artifact::artifact_schema_version;

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct GraphJsonArtifact;

pub(crate) fn read_graph_artifact_document(
    path: &Path,
) -> Result<GraphArtifactDocument, Vec<Diagnostic>> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) => return Err(vec![read_error_diagnostic(path, error)]),
    };
    let document = match serde_json::from_str::<GraphArtifactDocument>(&contents) {
        Ok(document) => document,
        Err(error) => {
            return Err(vec![
                Diagnostic::error(
                    DiagnosticCode::IoArtifactMalformed,
                    format!("Artifact '{}' is malformed: {error}", path.display()),
                )
                .with_help("Rebuild docs.graph.json from the source workspace."),
            ]);
        }
    };

    if document.schema_version != GRAPH_ARTIFACT_SCHEMA_VERSION {
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
                GRAPH_ARTIFACT_SCHEMA_VERSION
            )),
        ]);
    }

    Ok(document)
}

impl ArtifactReader for GraphJsonArtifact {
    type Output = GraphArtifactDocument;

    fn read(&self, path: &Path) -> Result<Self::Output, ArtifactReadError> {
        read_graph_artifact_document(path).map_err(|diagnostics| {
            ArtifactReadError::from_diagnostics(diagnostics)
                .with_schema_version(artifact_schema_version(path))
        })
    }
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
