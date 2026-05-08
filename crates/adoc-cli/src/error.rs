use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub(crate) enum CliError {
    #[error("error[init.already_exists] target already exists: {}", path.display())]
    InitTargetExists { path: PathBuf },

    #[error("error[io.output_not_directory] output path exists as a file: {}", path.display())]
    OutputPathIsFile { path: PathBuf },

    #[error("error[io.output_not_directory] could not create output directory {}: {source}", path.display())]
    CreateOutputDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("error[io.write_failed] could not write {}: {source}", path.display())]
    WriteFailed {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("error[artifact.agent_json] could not serialize agent JSON: {source}")]
    AgentJsonSerialize {
        #[source]
        source: serde_json::Error,
    },

    #[error("error[artifact.search_json] could not serialize search JSON: {source}")]
    SearchJsonSerialize {
        #[source]
        source: serde_json::Error,
    },

    #[error("build did not produce artifacts")]
    BuildMissingArtifacts,

    #[error("error[retrieval.io] could not write retrieval output: {source}")]
    RetrievalIo {
        #[source]
        source: std::io::Error,
    },
}

impl CliError {
    pub(crate) fn exit_code(&self) -> i32 {
        match self {
            CliError::RetrievalIo { .. } => 2,
            _ => 1,
        }
    }
}
