use std::io::{Read, Write};

use adoc_core::{MAX_PORTABLE_PROJECTION_BYTES, run_portable_projection};

pub(crate) fn portable_project() -> i32 {
    let mut bytes = Vec::new();
    if std::io::stdin()
        .take(MAX_PORTABLE_PROJECTION_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .is_err()
    {
        eprintln!("error[projection.unavailable] could not read portable projection input");
        return 2;
    }
    match run_portable_projection(&bytes) {
        Ok(output) => {
            let Ok(bytes) = serde_json::to_vec(&output) else {
                return 2;
            };
            if std::io::stdout().lock().write_all(&bytes).is_err() {
                return 2;
            }
            0
        }
        Err(diagnostic) => {
            eprintln!("error[{}] {}", diagnostic.code.as_str(), diagnostic.message);
            2
        }
    }
}
