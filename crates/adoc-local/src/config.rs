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
    /// V6.4 TB4 (ADR-0037): the MCP `adoc_patch_apply` gate. Absent `mcp:`
    /// block ⇒ disabled; `adoc init` never writes the key — opting in is a
    /// deliberate human edit. Note the back-compat consequence of
    /// `deny_unknown_fields`: a project that adds the `mcp:` block becomes
    /// unreadable by pre-V6.4 binaries (a loud config-parse failure that
    /// only bites opted-in projects; deliberately no version bump).
    pub mcp_patch_apply_enabled: bool,
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

        let mcp_patch_apply_enabled = match self.mcp {
            Some(mcp) => match mcp.patch_apply.as_str() {
                "enabled" => true,
                "disabled" => false,
                value => {
                    return Err(LocalError::ConfigInvalid {
                        path: path.to_path_buf(),
                        message: format!(
                            "unsupported mcp.patch_apply {value:?}; expected \"enabled\" or \"disabled\""
                        ),
                    });
                }
            },
            None => false,
        };

        Ok(ProjectConfig {
            path: path.to_path_buf(),
            docs_path: resolve_config_path(config_dir, self.docs_path),
            outputs,
            embeddings_provider,
            mcp_patch_apply_enabled,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn config_in_tempdir(contents: &str) -> (tempfile::TempDir, Result<Option<ProjectConfig>, LocalError>) {
        let dir = tempfile::tempdir().expect("create tempdir");
        fs::write(dir.path().join(CONFIG_FILE_NAME), contents).expect("write config");
        let result = ProjectConfig::discover_from(dir.path());
        (dir, result)
    }

    const BASE_CONFIG: &str = "version: 1\nmode: strict\ndocs_path: docs\n";

    #[test]
    fn mcp_patch_apply_defaults_to_disabled_when_block_absent() {
        let (_dir, result) = config_in_tempdir(BASE_CONFIG);
        let config = result.expect("config parses").expect("config found");
        assert!(!config.mcp_patch_apply_enabled);
    }

    #[test]
    fn mcp_patch_apply_enabled_parses_to_true() {
        let contents = format!("{BASE_CONFIG}mcp:\n  patch_apply: enabled\n");
        let (_dir, result) = config_in_tempdir(&contents);
        let config = result.expect("config parses").expect("config found");
        assert!(config.mcp_patch_apply_enabled);
    }

    #[test]
    fn mcp_patch_apply_disabled_parses_to_false() {
        let contents = format!("{BASE_CONFIG}mcp:\n  patch_apply: disabled\n");
        let (_dir, result) = config_in_tempdir(&contents);
        let config = result.expect("config parses").expect("config found");
        assert!(!config.mcp_patch_apply_enabled);
    }

    #[test]
    fn unknown_mcp_key_fails_loudly() {
        let contents = format!("{BASE_CONFIG}mcp:\n  unknown_key: x\n");
        let (_dir, result) = config_in_tempdir(&contents);
        assert!(
            matches!(result, Err(LocalError::ConfigParse { .. })),
            "deny_unknown_fields must reject unknown mcp keys"
        );
    }

    #[test]
    fn unsupported_mcp_patch_apply_value_names_the_key_and_allowed_values() {
        let contents = format!("{BASE_CONFIG}mcp:\n  patch_apply: yes\n");
        let (_dir, result) = config_in_tempdir(&contents);
        match result {
            Err(LocalError::ConfigInvalid { message, .. }) => {
                assert!(message.contains("mcp.patch_apply"), "message: {message}");
                assert!(message.contains("enabled"), "message: {message}");
                assert!(message.contains("disabled"), "message: {message}");
            }
            other => panic!("expected ConfigInvalid, got {other:?}"),
        }
    }
}
