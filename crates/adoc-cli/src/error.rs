#[derive(Debug, thiserror::Error)]
pub(crate) enum CliError {
    #[error(transparent)]
    Local(#[from] adoc_local::LocalError),

    #[error("error[cli.stdout_io] could not write command output: {source}")]
    StdoutIo {
        #[source]
        source: std::io::Error,
    },

    #[error("error[cli.stdin_io] could not read stdin: {source}")]
    StdinIo {
        #[source]
        source: std::io::Error,
    },
}

impl CliError {
    pub(crate) fn exit_code(&self) -> i32 {
        match self {
            CliError::Local(error) => error.exit_code(),
            CliError::StdoutIo { .. } | CliError::StdinIo { .. } => 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn io_error() -> std::io::Error {
        std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe closed")
    }

    #[test]
    fn stdout_io_pins_wire_code_and_exit_code() {
        let error = CliError::StdoutIo { source: io_error() };
        assert!(
            error
                .to_string()
                .starts_with("error[cli.stdout_io] could not write command output:"),
            "got: {error}"
        );
        assert_eq!(error.exit_code(), 2);
    }

    #[test]
    fn stdin_io_pins_wire_code_and_exit_code() {
        let error = CliError::StdinIo { source: io_error() };
        assert!(
            error
                .to_string()
                .starts_with("error[cli.stdin_io] could not read stdin:"),
            "got: {error}"
        );
        assert_eq!(error.exit_code(), 2);
    }
}
