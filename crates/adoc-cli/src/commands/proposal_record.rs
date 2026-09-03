use std::fs;
use std::path::{Path, PathBuf};

use adoc_core::{
    PROPOSAL_SCHEMA_VERSION, ProposalBindings, ProposalDispositionInput, ProposalPatchInput,
    build_proposal_record_with_dispositions,
};
use serde::Deserialize;

use crate::commands::artifact_paths::{ensure_distinct_paths, remove_stale, write_atomic};

/// CLI-private producer input: bindings, patch files, optional supersedes, and
/// receipted no-change dispositions. It carries identifiers and exact-head
/// placement only — never branch names or titles — so any producer yields the
/// same canonical record bytes.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposalRecordInput {
    bindings: ProposalBindings,
    #[serde(default)]
    supersedes: Option<String>,
    patches: Vec<PatchEntry>,
    #[serde(default)]
    dispositions: Vec<ProposalDispositionInput>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PatchEntry {
    finding_id: String,
    placement_path: String,
    page_id: String,
    patch_path: PathBuf,
}

pub(crate) fn proposal_record(input: PathBuf, out: PathBuf) -> i32 {
    if let Err(message) = ensure_distinct_paths(&[&input, &out]) {
        return fail(&message);
    }
    // --out holds this command's artifact and nothing else: an existing file
    // that is not a proposal record (a patch file, say) is refused, never
    // cleared, so a mistyped --out cannot destroy an input on any failure
    // path — including the ones before the patch paths are known. With
    // ownership settled, the stale clear is unconditional and a failed run
    // never leaves a previous record behind.
    if out.exists() && !is_proposal_record(&out) {
        return fail(&format!(
            "{} is not a proposal record; refusing to overwrite it",
            out.display()
        ));
    }
    let bytes = match fs::read(&input) {
        Ok(bytes) => bytes,
        Err(error) => {
            if let Err(message) = remove_stale(&out) {
                return fail(&message);
            }
            return fail(&format!("could not read {}: {error}", input.display()));
        }
    };
    let parsed: ProposalRecordInput = match serde_json::from_slice(&bytes) {
        Ok(parsed) => parsed,
        Err(error) => {
            if let Err(message) = remove_stale(&out) {
                return fail(&message);
            }
            return fail(&format!(
                "{} is not a valid proposal-record input: {error}",
                input.display()
            ));
        }
    };
    let base = input.parent().unwrap_or(Path::new("."));
    let patch_paths = parsed
        .patches
        .iter()
        .map(|entry| base.join(&entry.patch_path))
        .collect::<Vec<_>>();
    for path in &patch_paths {
        if let Err(message) = ensure_distinct_paths(&[path, &out]) {
            return fail(&message);
        }
    }
    if let Err(message) = remove_stale(&out) {
        return fail(&message);
    }
    let mut patches = Vec::with_capacity(parsed.patches.len());
    // Patch-vs-patch collisions are left to the domain, which names them
    // proposal_record.patch_invalid.
    for (entry, path) in parsed.patches.into_iter().zip(patch_paths) {
        let patch_bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) => return fail(&format!("could not read {}: {error}", path.display())),
        };
        patches.push(ProposalPatchInput {
            finding_id: entry.finding_id,
            placement_path: entry.placement_path,
            page_id: entry.page_id,
            patch_bytes,
        });
    }
    let record = match build_proposal_record_with_dispositions(
        parsed.bindings,
        patches,
        parsed.dispositions,
        parsed.supersedes,
    ) {
        Ok(record) => record,
        Err(error) => {
            return fail(&format!("[{}] {error}", error.diagnostic_code().as_str()));
        }
    };
    let json = match record.to_canonical_json() {
        Ok(json) => json,
        Err(error) => return fail(&format!("[{}] {error}", error.diagnostic_code().as_str())),
    };
    if let Err(message) = write_atomic(&out, json.as_bytes()) {
        return fail(&message);
    }
    print!("{json}");
    0
}

/// Ownership check for `--out`: only the record header is read, so a record
/// left by any earlier run — whatever its validity — is this command's to
/// clear, while nothing else ever is.
fn is_proposal_record(path: &Path) -> bool {
    #[derive(Deserialize)]
    struct Header {
        schema_version: String,
    }
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Header>(&bytes).ok())
        .is_some_and(|header| header.schema_version == PROPOSAL_SCHEMA_VERSION)
}

fn fail(message: &str) -> i32 {
    eprintln!("error: {message}");
    2
}
