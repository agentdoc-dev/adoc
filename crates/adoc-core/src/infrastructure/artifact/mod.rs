pub(crate) mod graph_json;
pub(crate) mod patch_json;
pub(crate) mod search_json;

pub(crate) use graph_json::GraphJsonArtifact;
pub(crate) use patch_json::{PatchJsonArtifact, read_patch_document, read_patch_document_value};
pub(crate) use search_json::SearchJsonArtifact;

use std::path::Path;

fn artifact_schema_version(path: &Path) -> Option<String> {
    let contents = std::fs::read_to_string(path).ok()?;
    let value = serde_json::from_str::<serde_json::Value>(&contents).ok()?;
    value
        .get("schema_version")
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string)
}
