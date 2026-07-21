use std::fs;
use std::path::{Path, PathBuf};

pub use adoc_core::EmbeddingsProvider;
use adoc_core::{ParsedConfigOutputs, ProjectConfigDocumentError, parse_project_config};

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
        let parsed = parse_project_config(&text).map_err(|error| match error {
            ProjectConfigDocumentError::Parse(source) => LocalError::ConfigParse {
                path: path.to_path_buf(),
                source: Box::new(source),
            },
            ProjectConfigDocumentError::Invalid(message) => LocalError::ConfigInvalid {
                path: path.to_path_buf(),
                message,
            },
        })?;
        let config_dir = path.parent().unwrap_or_else(|| Path::new("."));
        Ok(ProjectConfig {
            path: path.to_path_buf(),
            docs_path: resolve_config_path(config_dir, parsed.docs_path),
            outputs: resolve_outputs(parsed.outputs, config_dir),
            embeddings_provider: parsed.embeddings_provider,
            mcp_patch_apply_enabled: parsed.mcp_patch_apply_enabled,
        })
    }
}

fn resolve_outputs(outputs: ParsedConfigOutputs, config_dir: &Path) -> ConfigOutputs {
    let dir = outputs
        .dir
        .map(|path| resolve_config_path(config_dir, path));
    ConfigOutputs {
        html: outputs
            .html
            .map(|path| resolve_config_path(config_dir, path))
            .or_else(|| dir.as_ref().map(|dir| dir.join("docs.html"))),
        graph: outputs
            .graph
            .map(|path| resolve_config_path(config_dir, path))
            .or_else(|| dir.as_ref().map(|dir| dir.join("docs.graph.json"))),
        search: outputs
            .search
            .map(|path| resolve_config_path(config_dir, path))
            .or_else(|| dir.as_ref().map(|dir| dir.join("docs.search.json"))),
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

    fn config_in_tempdir(
        contents: &str,
    ) -> (tempfile::TempDir, Result<Option<ProjectConfig>, LocalError>) {
        let dir = tempfile::tempdir().expect("create tempdir");
        fs::write(dir.path().join(CONFIG_FILE_NAME), contents).expect("write config");
        let result = ProjectConfig::discover_from(dir.path());
        (dir, result)
    }

    const BASE_CONFIG: &str = "version: 1\nmode: strict\ndocs_path: docs\n";

    #[test]
    fn docs_path_must_be_a_portable_project_relative_path() {
        for docs_path in [
            "../docs",
            "/tmp/docs",
            "docs\\knowledge",
            " docs",
            "docs ",
            "C:/docs",
        ] {
            let contents = format!("version: 1\nmode: strict\ndocs_path: '{docs_path}'\n");
            let (_dir, result) = config_in_tempdir(&contents);

            match result {
                Err(LocalError::ConfigInvalid { message, .. }) => {
                    assert!(message.contains("docs_path"), "message: {message}");
                }
                other => panic!("unsafe docs_path {docs_path:?} must fail, got {other:?}"),
            }
        }
    }

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
