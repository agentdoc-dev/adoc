#[derive(Debug, thiserror::Error)]
pub(crate) enum CliError {
    #[error(transparent)]
    Local(#[from] adoc_local::LocalError),

    #[error("error[retrieval.io] could not write retrieval output: {source}")]
    RetrievalIo {
        #[source]
        source: std::io::Error,
    },
}

impl CliError {
    pub(crate) fn exit_code(&self) -> i32 {
        match self {
            CliError::Local(error) => error.exit_code(),
            CliError::RetrievalIo { .. } => 2,
        }
    }
}
