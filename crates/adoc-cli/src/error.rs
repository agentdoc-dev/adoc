use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub(crate) enum CliError {
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

    #[error("build did not produce artifacts")]
    BuildMissingArtifacts,

    #[error("invalid build usage: build requires <path> --out <directory>")]
    InvalidBuildUsage,

    #[error(
        "invalid explain usage: explain requires <object-id> [--artifact <path>] [--format text]"
    )]
    InvalidExplainUsage,

    #[error("unsupported explain format: {format}")]
    UnsupportedExplainFormat { format: String },

    #[error("missing command")]
    MissingCommand,

    #[error("unknown or invalid command: {command}")]
    UnknownCommand { command: String },
}

impl CliError {
    pub(crate) fn exit_code(&self) -> i32 {
        match self {
            CliError::InvalidExplainUsage | CliError::UnsupportedExplainFormat { .. } => 1,
            CliError::InvalidBuildUsage
            | CliError::MissingCommand
            | CliError::UnknownCommand { .. } => 2,
            _ => 1,
        }
    }
}
