use std::fs;
use std::path::{Path, PathBuf};

use adoc_core::{BuildArtifacts, BuildEmbeddingMode, BuildInput, CompileResult, build_workspace};

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
    graph: PathBuf,
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
    let graph = config.outputs.graph.clone();
    let search = config.outputs.search.clone();

    match (html, agent_json, graph, search_required, search) {
        (Some(html), Some(agent_json), Some(graph), true, Some(search)) => Ok(BuildOutputPaths {
            html,
            agent_json,
            graph,
            search: Some(search),
        }),
        (Some(html), Some(agent_json), Some(graph), false, search) => Ok(BuildOutputPaths {
            html,
            agent_json,
            graph,
            search,
        }),
        _ => Err(CliError::ConfigMissing {
            message: if search_required {
                "adoc build requires outputs.dir or exact html, agent_json, graph, and search outputs"
            } else {
                "adoc build requires outputs.dir or exact html, agent_json, and graph outputs"
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

    let paths = match output_paths_for_dir(out, artifacts.search_json.is_some()) {
        Ok(paths) => paths,
        Err(error) => return report(error),
    };

    match write_artifacts_to_paths(&paths, &artifacts) {
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

    match write_artifacts_to_paths(paths, &artifacts) {
        Ok(()) if has_errors => 1,
        Ok(()) => 0,
        Err(error) => report(error),
    }
}

fn output_paths_for_dir(out: &Path, include_search: bool) -> Result<BuildOutputPaths, CliError> {
    if out.exists() && !out.is_dir() {
        return Err(CliError::OutputPathIsFile {
            path: out.to_path_buf(),
        });
    }

    fs::create_dir_all(out).map_err(|source| CliError::CreateOutputDirectory {
        path: out.to_path_buf(),
        source,
    })?;

    Ok(BuildOutputPaths {
        html: out.join("docs.html"),
        agent_json: out.join("docs.agent.json"),
        graph: out.join("docs.graph.json"),
        search: include_search.then(|| out.join("docs.search.json")),
    })
}

struct ArtifactWriteEntry {
    path: PathBuf,
    contents: Vec<u8>,
}

fn write_artifacts_to_paths(
    paths: &BuildOutputPaths,
    artifacts: &BuildArtifacts,
) -> Result<(), CliError> {
    for entry in serialize_artifacts(paths, artifacts)? {
        write_file_with_parents(&entry.path, &entry.contents)?;
    }

    Ok(())
}

fn serialize_artifacts(
    paths: &BuildOutputPaths,
    artifacts: &BuildArtifacts,
) -> Result<Vec<ArtifactWriteEntry>, CliError> {
    let mut entries = vec![ArtifactWriteEntry {
        path: paths.html.clone(),
        contents: artifacts.html.as_bytes().to_vec(),
    }];

    let agent_json_text = artifacts
        .agent_json
        .to_pretty_json()
        .map_err(|source| CliError::AgentJsonSerialize { source })?;
    entries.push(ArtifactWriteEntry {
        path: paths.agent_json.clone(),
        contents: agent_json_text.into_bytes(),
    });

    let graph_json_text = artifacts
        .graph_json
        .to_pretty_json()
        .map_err(|source| CliError::GraphJsonSerialize { source })?;
    entries.push(ArtifactWriteEntry {
        path: paths.graph.clone(),
        contents: graph_json_text.into_bytes(),
    });

    if let (Some(search_json), Some(search_path)) =
        (artifacts.search_json.as_ref(), paths.search.as_ref())
    {
        let search_json_text = search_json
            .to_pretty_json()
            .map_err(|source| CliError::SearchJsonSerialize { source })?;
        entries.push(ArtifactWriteEntry {
            path: search_path.clone(),
            contents: search_json_text.into_bytes(),
        });
    }

    Ok(entries)
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

    use adoc_core::{
        AgentJsonDocument, BuildArtifacts, Diagnostic, DiagnosticCode, GraphArtifactDocument,
        SearchArtifactDocument, Severity,
    };

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
                graph_json: GraphArtifactDocument {
                    schema_version: "adoc.graph.v0".to_string(),
                    agent_artifact_hash: "sha256:agent".to_string(),
                    nodes: Vec::new(),
                    edges: Vec::new(),
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
        assert!(
            fs::read_to_string(output_directory.join("docs.graph.json"))
                .expect("graph JSON is written")
                .contains("\"schema_version\": \"adoc.graph.v0\"")
        );
        assert_eq!(
            fs::read_to_string(output_directory.join("docs.search.json"))
                .expect("prior search artifact remains readable"),
            "prior search artifact"
        );
    }

    #[test]
    fn finish_build_result_at_paths_writes_serialized_artifacts_to_exact_paths() {
        let output_directory = unique_temp_dir("exact-output");
        let paths = BuildOutputPaths {
            html: output_directory.join("site/docs.html"),
            agent_json: output_directory.join("agent/docs.agent.json"),
            graph: output_directory.join("graph/docs.graph.json"),
            search: Some(output_directory.join("search/docs.search.json")),
        };
        let result = CompileResult {
            diagnostics: Vec::new(),
            artifacts: Some(BuildArtifacts {
                html: "<h1>Guide</h1>".to_string(),
                agent_json: AgentJsonDocument {
                    schema_version: "adoc.agent.v0".to_string(),
                    pages: Vec::new(),
                    objects: Vec::new(),
                    diagnostics: Vec::new(),
                },
                graph_json: GraphArtifactDocument {
                    schema_version: "adoc.graph.v0".to_string(),
                    agent_artifact_hash: "sha256:agent".to_string(),
                    nodes: Vec::new(),
                    edges: Vec::new(),
                },
                search_json: Some(
                    serde_json::from_value::<SearchArtifactDocument>(serde_json::json!({
                        "schema_version": "adoc.search.v0",
                        "model": { "id": "in-memory", "provider": "test", "dim": 2 },
                        "agent_artifact_hash": "sha256:agent",
                        "embeddings": []
                    }))
                    .expect("test search artifact is valid"),
                ),
            }),
        };

        let exit_code = finish_build_result_at_paths(result, &paths);

        assert_eq!(exit_code, 0);
        assert_eq!(
            fs::read_to_string(&paths.html).expect("HTML is written"),
            "<h1>Guide</h1>"
        );
        assert!(
            fs::read_to_string(&paths.agent_json)
                .expect("agent JSON is written")
                .contains("\"schema_version\": \"adoc.agent.v0\"")
        );
        assert!(
            fs::read_to_string(&paths.graph)
                .expect("graph JSON is written")
                .contains("\"schema_version\": \"adoc.graph.v0\"")
        );
        assert!(
            fs::read_to_string(paths.search.as_ref().expect("search path"))
                .expect("search JSON is written")
                .contains("\"schema_version\": \"adoc.search.v0\"")
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
