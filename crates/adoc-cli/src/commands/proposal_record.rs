use std::fs;
use std::path::{Path, PathBuf};

use adoc_core::{ProposalBindings, ProposalPatchInput, build_proposal_record};
use serde::Deserialize;

use crate::commands::artifact_paths::{ensure_distinct_paths, remove_stale};

/// CLI-private producer input: the bindings plus one entry per patch file.
/// It carries identifiers and exact-head placement only — never branch names
/// or titles — so any producer yields the same canonical record bytes.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposalRecordInput {
    bindings: ProposalBindings,
    #[serde(default)]
    supersedes: Option<String>,
    patches: Vec<PatchEntry>,
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
    // Until the input parses, --out is only known to be distinct from
    // --input; that is enough to clear it, so a failed run never leaves a
    // previous record behind (the patch paths are not known yet, so nothing
    // else can be touched).
    let fail_clearing_out = |message: String| match remove_stale(&out) {
        Ok(()) => fail(&message),
        Err(removal) => fail(&format!("{message}; {removal}")),
    };
    let bytes = match fs::read(&input) {
        Ok(bytes) => bytes,
        Err(error) => {
            return fail_clearing_out(format!("could not read {}: {error}", input.display()));
        }
    };
    let parsed: ProposalRecordInput = match serde_json::from_slice(&bytes) {
        Ok(parsed) => parsed,
        Err(error) => {
            return fail_clearing_out(format!(
                "{} is not a valid proposal-record input: {error}",
                input.display()
            ));
        }
    };
    let base = input.parent().unwrap_or(Path::new("."));
    // Every patch file is an input too: check each against --out before
    // the stale-output clear can destroy one of them. Patch-vs-patch
    // collisions are left to the domain, which names them
    // proposal_record.patch_invalid.
    let patch_paths: Vec<PathBuf> = parsed
        .patches
        .iter()
        .map(|entry| base.join(&entry.patch_path))
        .collect();
    for path in &patch_paths {
        if let Err(message) = ensure_distinct_paths(&[&out, path]) {
            return fail(&message);
        }
    }
    if let Err(message) = remove_stale(&out) {
        return fail(&message);
    }
    let mut patches = Vec::with_capacity(parsed.patches.len());
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
    let record = match build_proposal_record(parsed.bindings, patches, parsed.supersedes) {
        Ok(record) => record,
        Err(error) => {
            return fail(&format!("[{}] {error}", error.diagnostic_code().as_str()));
        }
    };
    let json = match record.to_canonical_json() {
        Ok(json) => json,
        Err(error) => return fail(&format!("[{}] {error}", error.diagnostic_code().as_str())),
    };
    if let Err(error) = fs::write(&out, &json) {
        return fail(&format!("could not write {}: {error}", out.display()));
    }
    print!("{json}");
    0
}

fn fail(message: &str) -> i32 {
    eprintln!("error: {message}");
    2
}
