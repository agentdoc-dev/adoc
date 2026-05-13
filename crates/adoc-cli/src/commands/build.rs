use std::fs;
use std::path::{Path, PathBuf};

use adoc_core::{
    AgentJsonDocument, BuildEmbeddingMode, BuildInput, CompileResult, SearchArtifactDocument,
    build_workspace,
};

use crate::config::{EmbeddingsProvider, ProjectConfig};
use crate::error::CliError;

use super::{
    discover_project_config_if, print_diagnostics, print_summary, report,
    resolve_docs_path_with_config,
};

pub(crate) fn build(path: Option<PathBuf>, out: Option<PathBuf>, no_embeddings: bool) -> i32 {
    let needs_config = path.is_none() || out.is_none() || !no_embeddings;
    let config = match discover_project_config_if(needs_config) {
        Ok(config) => config,
        Err(error) => return report(error),
    };
    let path = match resolve_docs_path_with_config(path, config.as_ref()) {
        Ok(path) => path,
        Err(error) => return report(error),
    };
    let embedding_mode = resolve_embedding_mode(config.as_ref(), no_embeddings);

    match out {
        Some(out) => build_to_dir(path, out, embedding_mode),
        None => {
            let output_paths = match resolve_build_output_paths(config.as_ref(), embedding_mode) {
                Ok(paths) => paths,
                Err(error) => return report(error),
            };
            build_to_paths(path, output_paths, embedding_mode)
        }
    }
}

fn build_to_dir(path: PathBuf, out: PathBuf, embedding_mode: BuildEmbeddingMode) -> i32 {
    let result = build_workspace(BuildInput {
        root: path,
        embeddings: embedding_mode,
        prior_search_artifact_path: Some(out.join("docs.search.json")),
    });
    print_diagnostics(&result.diagnostics);
    print_summary(&result.diagnostics);

    finish_build_result(result, &out)
}

fn build_to_paths(
    path: PathBuf,
    output_paths: BuildOutputPaths,
    embedding_mode: BuildEmbeddingMode,
) -> i32 {
    let result = build_workspace(BuildInput {
        root: path,
        embeddings: embedding_mode,
        prior_search_artifact_path: output_paths.search.clone(),
    });
    print_diagnostics(&result.diagnostics);
    print_summary(&result.diagnostics);

    finish_build_result_at_paths(result, &output_paths)
}

#[derive(Debug, Clone)]
struct BuildOutputPaths {
    html: PathBuf,
    agent_json: PathBuf,
    search: Option<PathBuf>,
}

fn resolve_embedding_mode(
    config: Option<&ProjectConfig>,
    no_embeddings: bool,
) -> BuildEmbeddingMode {
    if no_embeddings
        || config
            .map(|config| config.embeddings_provider == EmbeddingsProvider::None)
            .unwrap_or(false)
    {
        BuildEmbeddingMode::Skipped
    } else {
        BuildEmbeddingMode::Enabled
    }
}

fn resolve_build_output_paths(
    config: Option<&ProjectConfig>,
    embedding_mode: BuildEmbeddingMode,
) -> Result<BuildOutputPaths, CliError> {
    let Some(config) = config else {
        return Err(CliError::ConfigMissing {
            message: "adoc build requires --out or agentdoc.config.yaml outputs".to_string(),
            config_path: None,
        });
    };

    let search_required = embedding_mode == BuildEmbeddingMode::Enabled;
    let html = config.outputs.html.clone();
    let agent_json = config.outputs.agent_json.clone();
    let search = config.outputs.search.clone();

    match (html, agent_json, search_required, search) {
        (Some(html), Some(agent_json), true, Some(search)) => Ok(BuildOutputPaths {
            html,
            agent_json,
            search: Some(search),
        }),
        (Some(html), Some(agent_json), false, search) => Ok(BuildOutputPaths {
            html,
            agent_json,
            search,
        }),
        _ => Err(CliError::ConfigMissing {
            message: if search_required {
                "adoc build requires outputs.dir or exact html, agent_json, and search outputs"
            } else {
                "adoc build requires outputs.dir or exact html and agent_json outputs"
            }
            .to_string(),
            config_path: Some(config.path.clone()),
        }),
    }
}

fn finish_build_result(result: CompileResult, out: &Path) -> i32 {
    let has_errors = result.has_errors();

    let artifacts = match result.artifacts {
        Some(artifacts) => artifacts,
        None if has_errors => return 1,
        None => return report(CliError::BuildMissingArtifacts),
    };

    match write_artifacts(
        out,
        &artifacts.html,
        &artifacts.agent_json,
        artifacts.search_json.as_ref(),
    ) {
        Ok(()) if has_errors => 1,
        Ok(()) => 0,
        Err(error) => report(error),
    }
}

fn finish_build_result_at_paths(result: CompileResult, paths: &BuildOutputPaths) -> i32 {
    let has_errors = result.has_errors();

    let artifacts = match result.artifacts {
        Some(artifacts) => artifacts,
        None if has_errors => return 1,
        None => return report(CliError::BuildMissingArtifacts),
    };

    match write_artifacts_to_paths(
        paths,
        &artifacts.html,
        &artifacts.agent_json,
        artifacts.search_json.as_ref(),
    ) {
        Ok(()) if has_errors => 1,
        Ok(()) => 0,
        Err(error) => report(error),
    }
}

fn write_artifacts(
    out: &Path,
    html: &str,
    agent_json: &AgentJsonDocument,
    search_json: Option<&SearchArtifactDocument>,
) -> Result<(), CliError> {
    if out.exists() && !out.is_dir() {
        return Err(CliError::OutputPathIsFile {
            path: out.to_path_buf(),
        });
    }

    fs::create_dir_all(out).map_err(|source| CliError::CreateOutputDirectory {
        path: out.to_path_buf(),
        source,
    })?;

    let html_path = out.join("docs.html");
    fs::write(&html_path, html).map_err(|source| CliError::WriteFailed {
        path: html_path,
        source,
    })?;

    let agent_json_text = agent_json
        .to_pretty_json()
        .map_err(|source| CliError::AgentJsonSerialize { source })?;
    let agent_json_path = out.join("docs.agent.json");
    fs::write(&agent_json_path, agent_json_text).map_err(|source| CliError::WriteFailed {
        path: agent_json_path,
        source,
    })?;

    if let Some(search_json) = search_json {
        let search_json_text = search_json
            .to_pretty_json()
            .map_err(|source| CliError::SearchJsonSerialize { source })?;
        let search_json_path = out.join("docs.search.json");
        fs::write(&search_json_path, search_json_text).map_err(|source| CliError::WriteFailed {
            path: search_json_path,
            source,
        })?;
    }

    Ok(())
}

fn write_artifacts_to_paths(
    paths: &BuildOutputPaths,
    html: &str,
    agent_json: &AgentJsonDocument,
    search_json: Option<&SearchArtifactDocument>,
) -> Result<(), CliError> {
    write_file_with_parents(&paths.html, html.as_bytes())?;

    let agent_json_text = agent_json
        .to_pretty_json()
        .map_err(|source| CliError::AgentJsonSerialize { source })?;
    write_file_with_parents(&paths.agent_json, agent_json_text.as_bytes())?;

    if let (Some(search_json), Some(search_path)) = (search_json, paths.search.as_ref()) {
        let search_json_text = search_json
            .to_pretty_json()
            .map_err(|source| CliError::SearchJsonSerialize { source })?;
        write_file_with_parents(search_path, search_json_text.as_bytes())?;
    }

    Ok(())
}

fn write_file_with_parents(path: &Path, contents: &[u8]) -> Result<(), CliError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|source| CliError::CreateOutputDirectory {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    fs::write(path, contents).map_err(|source| CliError::WriteFailed {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use adoc_core::{AgentJsonDocument, BuildArtifacts, Diagnostic, DiagnosticCode, Severity};

    use super::*;

    #[test]
    fn finish_build_result_writes_v0_artifacts_and_preserves_prior_search_on_embedding_error() {
        let output_directory = unique_temp_dir("embedding-error-output");
        fs::create_dir_all(&output_directory).expect("output directory can be created");
        fs::write(
            output_directory.join("docs.search.json"),
            "prior search artifact",
        )
        .expect("prior search artifact can be written");
        let result = CompileResult {
            diagnostics: vec![Diagnostic {
                code: DiagnosticCode::EmbedComputeFailed,
                severity: Severity::Error,
                message: "embedding computation failed: encoder failed".to_string(),
                span: None,
                object_id: None,
                help: None,
            }],
            artifacts: Some(BuildArtifacts {
                html: "<h1>Guide</h1>".to_string(),
                agent_json: AgentJsonDocument {
                    schema_version: "adoc.agent.v0".to_string(),
                    pages: Vec::new(),
                    objects: Vec::new(),
                    diagnostics: Vec::new(),
                },
                search_json: None,
            }),
        };

        let exit_code = finish_build_result(result, &output_directory);

        assert_eq!(exit_code, 1);
        assert_eq!(
            fs::read_to_string(output_directory.join("docs.html")).expect("HTML is written"),
            "<h1>Guide</h1>"
        );
        assert!(
            fs::read_to_string(output_directory.join("docs.agent.json"))
                .expect("agent JSON is written")
                .contains("\"schema_version\": \"adoc.agent.v0\"")
        );
        assert_eq!(
            fs::read_to_string(output_directory.join("docs.search.json"))
                .expect("prior search artifact remains readable"),
            "prior search artifact"
        );
    }

    fn unique_temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("adoc-cli-{name}-{}-{nanos}", std::process::id()))
    }
}
