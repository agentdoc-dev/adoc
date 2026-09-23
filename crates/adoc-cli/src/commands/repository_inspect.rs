use super::migration_import::read_bounded;
use adoc_core::{MIGRATION_REQUEST_MAX_BYTES, MigrationError};
use std::{io::Write, path::PathBuf};

/// Exit 0 with a receipt for every completed inspection (including invalid config,
/// no files or failed validation); exit 2 with a diagnostic code when unavailable.
pub(crate) fn repository_inspect(
    request: PathBuf,
    repository: PathBuf,
    runtime_binary_digest: String,
) -> i32 {
    let result = (|| {
        let request = read_bounded(
            &request,
            MIGRATION_REQUEST_MAX_BYTES,
            MigrationError::InvalidRequest,
        )?;
        let receipt = adoc_core::inspect_repository_from_git(
            &repository,
            &request,
            env!("CARGO_PKG_VERSION").into(),
            runtime_binary_digest,
        )?;
        std::io::stdout()
            .write_all(receipt.to_canonical_json()?.as_bytes())
            .map_err(|_| MigrationError::ValidationUnavailable)
    })();
    match result {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("error[{}] {error}", error.diagnostic_code());
            2
        }
    }
}
