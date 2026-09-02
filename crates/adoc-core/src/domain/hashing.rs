use std::{collections::BTreeMap, fmt::Write};

use serde_json::Value;
use sha2::{Digest, Sha256};

pub(crate) fn canonicalize_object_keys(value: Value) -> Value {
    match value {
        Value::Object(fields) => Value::Object(
            fields
                .into_iter()
                .map(|(key, value)| (key, canonicalize_object_keys(value)))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        Value::Array(values) => {
            Value::Array(values.into_iter().map(canonicalize_object_keys).collect())
        }
        value => value,
    }
}

pub(crate) fn sha256_prefixed(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity("sha256:".len() + digest.len() * 2);
    output.push_str("sha256:");
    for byte in digest {
        let _ = write!(output, "{byte:02x}");
    }
    output
}
