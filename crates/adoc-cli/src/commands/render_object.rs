use std::io::Read;
use std::path::PathBuf;

use adoc_core::{MAX_RENDER_OBJECT_BYTES, run_render_object, run_render_object_into};

use super::artifact_paths::{ensure_distinct_paths, remove_stale, write_atomic};
use super::eprint_diagnostics;

fn fail(message: &str) -> i32 {
    eprintln!("error: {message}");
    2
}

/// Read at most the size bound plus one byte, so the core can refuse an oversized file.
fn read_bounded(path: &PathBuf) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| {
            file.take(MAX_RENDER_OBJECT_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
        })
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    Ok(bytes)
}

pub(crate) fn render_object(
    input: PathBuf,
    out: PathBuf,
    into: Option<PathBuf>,
    append: bool,
) -> i32 {
    let mut paths = vec![&input, &out];
    paths.extend(&into);
    if let Err(message) =
        ensure_distinct_paths(&paths.iter().map(|p| p.as_path()).collect::<Vec<_>>())
    {
        return fail(&message);
    }
    // A failed run never leaves a previous block for a consumer to mistake
    // for this run's result.
    if let Err(message) = remove_stale(&out) {
        return fail(&message);
    }
    let bytes = match read_bounded(&input) {
        Ok(bytes) => bytes,
        Err(message) => return fail(&message),
    };
    let result = match &into {
        None => run_render_object(&bytes),
        Some(file) => match read_bounded(file) {
            Ok(file_bytes) => run_render_object_into(&bytes, &file_bytes, append),
            Err(message) => return fail(&message),
        },
    };
    match result {
        Ok(text) => match write_atomic(&out, text.as_bytes()) {
            Ok(()) => 0,
            Err(message) => fail(&message),
        },
        Err(diagnostics) => {
            eprint_diagnostics(&diagnostics);
            2
        }
    }
}
