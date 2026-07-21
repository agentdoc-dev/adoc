use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::source::LogicalPath;

#[derive(Debug, Clone)]
pub struct ParsedProjectConfig {
    pub docs_path: PathBuf,
    pub outputs: ParsedConfigOutputs,
    pub embeddings_provider: EmbeddingsProvider,
    pub mcp_patch_apply_enabled: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ParsedConfigOutputs {
    pub dir: Option<PathBuf>,
    pub html: Option<PathBuf>,
    pub graph: Option<PathBuf>,
    pub search: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddingsProvider {
    Local,
    Deterministic,
    None,
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectConfigDocumentError {
    #[error("{0}")]
    Parse(#[from] serde_saphyr::Error),
    #[error("{0}")]
    Invalid(String),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProjectConfig {
    version: u32,
    mode: String,
    docs_path: PathBuf,
    outputs: Option<RawOutputs>,
    embeddings: Option<RawEmbeddings>,
    mcp: Option<RawMcp>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawOutputs {
    dir: Option<PathBuf>,
    html: Option<PathBuf>,
    graph: Option<PathBuf>,
    search: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEmbeddings {
    provider: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMcp {
    patch_apply: String,
}

pub fn parse_project_config(text: &str) -> Result<ParsedProjectConfig, ProjectConfigDocumentError> {
    let raw: RawProjectConfig = serde_saphyr::from_str(text)?;
    if raw.version != 1 {
        return Err(ProjectConfigDocumentError::Invalid(format!(
            "unsupported version {}; expected 1",
            raw.version
        )));
    }
    if raw.mode != "strict" {
        return Err(ProjectConfigDocumentError::Invalid(format!(
            "unsupported mode {:?}; expected \"strict\"",
            raw.mode
        )));
    }
    if !portable_docs_path(&raw.docs_path) {
        return Err(ProjectConfigDocumentError::Invalid(
            "docs_path must be a portable project-relative path".to_string(),
        ));
    }
    let embeddings_provider = match raw.embeddings.map(|value| value.provider) {
        Some(provider) if provider == "local" => EmbeddingsProvider::Local,
        Some(provider) if provider == "deterministic" => EmbeddingsProvider::Deterministic,
        Some(provider) if provider == "none" => EmbeddingsProvider::None,
        Some(provider) => {
            return Err(ProjectConfigDocumentError::Invalid(format!(
                "unsupported embeddings provider {provider:?}; expected \"local\", \"deterministic\", or \"none\""
            )));
        }
        None => EmbeddingsProvider::Local,
    };
    let mcp_patch_apply_enabled = match raw.mcp.map(|value| value.patch_apply) {
        Some(value) if value == "enabled" => true,
        Some(value) if value == "disabled" => false,
        Some(value) => {
            return Err(ProjectConfigDocumentError::Invalid(format!(
                "unsupported mcp.patch_apply {value:?}; expected \"enabled\" or \"disabled\""
            )));
        }
        None => false,
    };
    let outputs = raw.outputs.unwrap_or_default();
    Ok(ParsedProjectConfig {
        docs_path: raw.docs_path,
        outputs: ParsedConfigOutputs {
            dir: outputs.dir,
            html: outputs.html,
            graph: outputs.graph,
            search: outputs.search,
        },
        embeddings_provider,
        mcp_patch_apply_enabled,
    })
}

fn portable_docs_path(path: &Path) -> bool {
    path == Path::new(".")
        || path
            .to_str()
            .is_some_and(|value| LogicalPath::parse(value).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_complete_shipped_configuration() {
        let parsed = parse_project_config(
            r#"
version: 1
mode: strict
docs_path: .
outputs:
  dir: build
  html: site.html
  graph: graph.json
  search: search.json
embeddings:
  provider: deterministic
mcp:
  patch_apply: enabled
"#,
        )
        .expect("valid config");

        assert_eq!(parsed.docs_path, PathBuf::from("."));
        assert_eq!(parsed.outputs.dir, Some(PathBuf::from("build")));
        assert_eq!(parsed.outputs.html, Some(PathBuf::from("site.html")));
        assert_eq!(parsed.outputs.graph, Some(PathBuf::from("graph.json")));
        assert_eq!(parsed.outputs.search, Some(PathBuf::from("search.json")));
        assert_eq!(
            parsed.embeddings_provider,
            EmbeddingsProvider::Deterministic
        );
        assert!(parsed.mcp_patch_apply_enabled);
    }

    #[test]
    fn rejects_unknown_fields() {
        let error = parse_project_config(
            r#"
version: 1
mode: strict
docs_path: docs
future_setting: enabled
"#,
        )
        .expect_err("unknown field must fail closed");

        assert!(error.to_string().contains("unknown field"));
    }
}
