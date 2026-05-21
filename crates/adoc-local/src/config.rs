use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::LocalError;

const CONFIG_FILE_NAME: &str = "agentdoc.config.yaml";

#[derive(Debug, Clone)]
pub struct ProjectConfig {
    pub path: PathBuf,
    pub docs_path: PathBuf,
    pub outputs: ConfigOutputs,
    pub embeddings_provider: EmbeddingsProvider,
}

#[derive(Debug, Clone, Default)]
pub struct ConfigOutputs {
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProjectConfig {
    version: u32,
    mode: String,
    docs_path: PathBuf,
    outputs: Option<RawOutputs>,
    embeddings: Option<RawEmbeddings>,
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

impl ProjectConfig {
    pub fn discover() -> Result<Option<Self>, LocalError> {
        let current_dir =
            std::env::current_dir().map_err(|source| LocalError::CurrentDir { source })?;
        Self::discover_from(&current_dir)
    }

    pub fn discover_from(start_dir: &Path) -> Result<Option<Self>, LocalError> {
        let mut current_dir =
            std::fs::canonicalize(start_dir).unwrap_or_else(|_| start_dir.to_path_buf());
        let home_boundary = home_boundary();

        loop {
            let candidate = current_dir.join(CONFIG_FILE_NAME);
            if candidate.exists() {
                return Self::read(&candidate).map(Some);
            }

            if current_dir.parent().is_none() {
                return Ok(None);
            }

            if is_git_boundary(&current_dir) {
                return Ok(None);
            }

            if home_boundary.as_deref() == Some(current_dir.as_path()) {
                return Ok(None);
            }

            if !current_dir.pop() {
                return Ok(None);
            }
        }
    }

    fn read(path: &Path) -> Result<Self, LocalError> {
        let text = fs::read_to_string(path).map_err(|source| LocalError::ConfigRead {
            path: path.to_path_buf(),
            source,
        })?;
        let raw: RawProjectConfig =
            serde_saphyr::from_str(&text).map_err(|source| LocalError::ConfigParse {
                path: path.to_path_buf(),
                source: Box::new(source),
            })?;
        raw.validate_and_resolve(path)
    }
}

impl RawProjectConfig {
    fn validate_and_resolve(self, path: &Path) -> Result<ProjectConfig, LocalError> {
        if self.version != 1 {
            return Err(LocalError::ConfigInvalid {
                path: path.to_path_buf(),
                message: format!("unsupported version {}; expected 1", self.version),
            });
        }

        if self.mode != "strict" {
            return Err(LocalError::ConfigInvalid {
                path: path.to_path_buf(),
                message: format!("unsupported mode {:?}; expected \"strict\"", self.mode),
            });
        }

        let config_dir = path.parent().unwrap_or_else(|| Path::new("."));
        let outputs = self.outputs.unwrap_or_default().resolve(config_dir);
        let embeddings_provider = match self.embeddings {
            Some(embeddings) => match embeddings.provider.as_str() {
                "local" => EmbeddingsProvider::Local,
                "deterministic" => EmbeddingsProvider::Deterministic,
                "none" => EmbeddingsProvider::None,
                provider => {
                    return Err(LocalError::ConfigInvalid {
                        path: path.to_path_buf(),
                        message: format!(
                            "unsupported embeddings provider {provider:?}; expected \"local\", \"deterministic\", or \"none\""
                        ),
                    });
                }
            },
            None => EmbeddingsProvider::Local,
        };

        Ok(ProjectConfig {
            path: path.to_path_buf(),
            docs_path: resolve_config_path(config_dir, self.docs_path),
            outputs,
            embeddings_provider,
        })
    }
}

impl RawOutputs {
    fn resolve(self, config_dir: &Path) -> ConfigOutputs {
        let dir = self.dir.map(|path| resolve_config_path(config_dir, path));
        ConfigOutputs {
            html: self
                .html
                .map(|path| resolve_config_path(config_dir, path))
                .or_else(|| dir.as_ref().map(|dir| dir.join("docs.html"))),
            graph: self
                .graph
                .map(|path| resolve_config_path(config_dir, path))
                .or_else(|| dir.as_ref().map(|dir| dir.join("docs.graph.json"))),
            search: self
                .search
                .map(|path| resolve_config_path(config_dir, path))
                .or_else(|| dir.as_ref().map(|dir| dir.join("docs.search.json"))),
        }
    }
}

fn resolve_config_path(config_dir: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        config_dir.join(path)
    }
}

fn is_git_boundary(path: &Path) -> bool {
    path.join(".git").exists()
}

fn home_boundary() -> Option<PathBuf> {
    std::env::var_os("HOME").and_then(|home| {
        let home = PathBuf::from(home);
        if home.as_os_str().is_empty() {
            None
        } else {
            Some(std::fs::canonicalize(&home).unwrap_or(home))
        }
    })
}
