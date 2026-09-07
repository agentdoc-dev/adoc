use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

use adoc_core::{
    Diagnostic, DiagnosticCode, ManagedRetrievalQuery, RetrievalEnvelope, SearchMode, Severity,
    run_managed_retrieval,
};

use crate::cli::{ManagedRetrievalCommand, ManagedSearchMode};

use super::write_json_or_report;

pub(crate) fn managed_retrieve(
    input: PathBuf,
    manifest_out: Option<PathBuf>,
    operation: ManagedRetrievalCommand,
) -> i32 {
    const MAX_INPUT_BYTES: u64 = 64 * 1024 * 1024;
    let mut bytes = Vec::new();
    if File::open(input)
        .and_then(|file| file.take(MAX_INPUT_BYTES + 1).read_to_end(&mut bytes))
        .is_err()
        || bytes.len() as u64 > MAX_INPUT_BYTES
    {
        return refuse();
    }
    let query = match operation {
        ManagedRetrievalCommand::Search { query, mode, top } => ManagedRetrievalQuery::Search {
            text: query,
            mode: match mode {
                ManagedSearchMode::Lexical => SearchMode::Lexical,
                ManagedSearchMode::Semantic => SearchMode::Semantic,
                ManagedSearchMode::Hybrid => SearchMode::Hybrid,
            },
            top,
        },
        ManagedRetrievalCommand::Why { object_id } => ManagedRetrievalQuery::Why { object_id },
    };
    let outcome = run_managed_retrieval(&bytes, query);
    if let Some(path) = manifest_out {
        // Never replace an input or a retained manifest. Cloud supplies a fresh
        // private directory for every invocation and consumes both outputs.
        let manifest = match serde_json::to_vec(&outcome.contributing_bindings) {
            Ok(manifest) => manifest,
            Err(_) => return refuse(),
        };
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = match options.open(&path) {
            Ok(file) => file,
            Err(_) => return refuse(),
        };
        if file.write_all(&manifest).is_err() {
            let _ = std::fs::remove_file(path);
            return refuse();
        }
    }
    write_json_or_report(&outcome.envelope, outcome.exit_code)
}

fn refuse() -> i32 {
    write_json_or_report(
        &RetrievalEnvelope::new(
            Vec::new(),
            vec![Diagnostic {
                code: DiagnosticCode::RetrievalVisibilityUnavailable,
                severity: Severity::Error,
                message: "Managed retrieval input or output is unavailable.".into(),
                span: None,
                object_id: None,
                help: Some(
                    "Supply a bounded authorized managed input and a fresh private output path."
                        .into(),
                ),
            }],
        ),
        2,
    )
}
