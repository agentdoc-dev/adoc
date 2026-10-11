use std::io::Read;
use std::path::PathBuf;

use adoc_core::{MAX_RENDER_OBJECT_BYTES, run_render_object};

use super::artifact_paths::{ensure_distinct_paths, remove_stale, write_atomic};
use super::eprint_diagnostics;

fn fail(message: &str) -> i32 {
    eprintln!("error: {message}");
    2
}

pub(crate) fn render_object(input: PathBuf, out: PathBuf) -> i32 {
    if let Err(message) = ensure_distinct_paths(&[&input, &out]) {
        return fail(&message);
    }
    // A failed run never leaves a previous block for a consumer to mistake
    // for this run's result.
    if let Err(message) = remove_stale(&out) {
        return fail(&message);
    }
    let mut bytes = Vec::new();
    if let Err(error) = std::fs::File::open(&input).and_then(|file| {
        file.take(MAX_RENDER_OBJECT_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
    }) {
        return fail(&format!("could not read {}: {error}", input.display()));
    }
    match run_render_object(&bytes) {
        Ok(block) => match write_atomic(&out, block.as_bytes()) {
            Ok(()) => 0,
            Err(message) => fail(&message),
        },
        Err(diagnostics) => {
            eprint_diagnostics(&diagnostics);
            2
        }
    }
}
