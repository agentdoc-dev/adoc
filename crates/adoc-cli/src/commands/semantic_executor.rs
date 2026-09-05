use std::fs;
use std::path::{Path, PathBuf};

use crate::commands::artifact_paths::{ensure_distinct_paths, remove_stale, write_atomic};
use adoc_core::{
    DiagnosticCode, HumanReviewExpectedBindings, SemanticExecutorError,
    build_semantic_context_from_document, complete_semantic_execution, fail_semantic_execution,
    validate_human_semantic_assessment, validate_semantic_assessment,
    validate_semantic_executor_request,
};

const MAX_ASSESSMENT_BYTES: u64 = 1024 * 1024;

pub(crate) fn semantic_context(input: PathBuf, out: PathBuf) -> i32 {
    if let Err(message) = ensure_distinct_paths(&[&input, &out]) {
        return fail(&message);
    }
    if let Err(message) = remove_stale(&out) {
        return fail(&message);
    }
    let bytes = match fs::read(&input) {
        Ok(bytes) => bytes,
        Err(error) => return fail(&format!("could not read {}: {error}", input.display())),
    };
    let context = match build_semantic_context_from_document(&bytes) {
        Ok(context) => context,
        Err(error) => return fail(&error.to_string()),
    };
    let json = match context.to_canonical_json() {
        Ok(json) => json,
        Err(error) => return fail(&error.to_string()),
    };
    if let Err(error) = fs::write(&out, &json) {
        return fail(&format!("could not write {}: {error}", out.display()));
    }
    print!("{json}");
    0
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn semantic_executor(
    request_path: PathBuf,
    assessment_path: PathBuf,
    failure_code: Option<String>,
    receipt_path: PathBuf,
    validated_assessment_path: PathBuf,
    validated_request_path: Option<PathBuf>,
    reviewing_principal_id: Option<String>,
    requesting_principal_id: Option<String>,
) -> i32 {
    let mut paths = vec![
        request_path.as_path(),
        assessment_path.as_path(),
        receipt_path.as_path(),
        validated_assessment_path.as_path(),
    ];
    if let Some(path) = &validated_request_path {
        paths.push(path);
    }
    if let Err(message) = ensure_distinct_paths(&paths) {
        return fail(&message);
    }
    let output_paths = [receipt_path.as_path(), validated_assessment_path.as_path()]
        .into_iter()
        .chain(validated_request_path.as_deref())
        .collect::<Vec<_>>();
    if let Err(message) = remove_outputs(&output_paths) {
        return fail(&message);
    }
    let request_bytes = match fs::read(&request_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            return fail(&format!(
                "could not read {}: {error}",
                request_path.display()
            ));
        }
    };
    let request = match validate_semantic_executor_request(&request_bytes) {
        Ok(request) => request,
        Err(error) => return fail(&error.to_string()),
    };
    let validated_request_bytes = match &validated_request_path {
        Some(_) => match request.to_digest_bytes() {
            Ok(bytes) => Some(bytes),
            Err(error) => return fail(&error.to_string()),
        },
        None => None,
    };
    let expected_human_review = match (reviewing_principal_id, requesting_principal_id) {
        (Some(reviewing_principal_id), Some(requesting_principal_id)) => {
            Some(HumanReviewExpectedBindings {
                reviewing_principal_id,
                requesting_principal_id,
            })
        }
        (None, None) => None,
        _ => return fail("both trusted human-review Principal IDs are required together"),
    };
    if let Some(code) = failure_code {
        return record_failure(
            &request,
            &code,
            "semantic executor invocation failed before candidate validation",
            &receipt_path,
            validated_request_path
                .as_deref()
                .zip(validated_request_bytes.as_deref()),
        );
    }
    let assessment_bytes = match bounded_assessment(&assessment_path) {
        Ok(bytes) => bytes,
        Err(message) => {
            return record_failure(
                &request,
                "assessment.semantic_schema_invalid",
                &message,
                &receipt_path,
                validated_request_path
                    .as_deref()
                    .zip(validated_request_bytes.as_deref()),
            );
        }
    };
    let validated = match expected_human_review.as_ref() {
        Some(expected) => {
            validate_human_semantic_assessment(&assessment_bytes, request.context(), expected)
        }
        None => validate_semantic_assessment(&assessment_bytes, request.context()),
    };
    let assessment = match validated {
        Ok(assessment) => assessment,
        Err(error) => {
            return record_failure(
                &request,
                error.diagnostic_code().as_str(),
                &error.to_string(),
                &receipt_path,
                validated_request_path
                    .as_deref()
                    .zip(validated_request_bytes.as_deref()),
            );
        }
    };
    let receipt =
        match complete_semantic_execution(&request, &assessment, expected_human_review.as_ref()) {
            Ok(receipt) => receipt,
            Err(error) => {
                let code = match &error {
                    SemanticExecutorError::IdentityMismatch => {
                        DiagnosticCode::AssessmentSemanticIdentityMismatch.as_str()
                    }
                    _ => DiagnosticCode::AssessmentSemanticSchemaInvalid.as_str(),
                };
                return record_failure(
                    &request,
                    code,
                    &error.to_string(),
                    &receipt_path,
                    validated_request_path
                        .as_deref()
                        .zip(validated_request_bytes.as_deref()),
                );
            }
        };
    let assessment_json = match assessment.to_canonical_json() {
        Ok(json) => json,
        Err(error) => return fail(&error.to_string()),
    };
    let receipt_json = match receipt.to_canonical_json() {
        Ok(json) => json,
        Err(error) => return fail(&error.to_string()),
    };
    if let Err(message) = write_atomic(&validated_assessment_path, assessment_json.as_bytes()) {
        return fail(&message);
    }
    if let Err(message) = publish_receipt(
        &receipt_path,
        receipt_json.as_bytes(),
        validated_request_path
            .as_deref()
            .zip(validated_request_bytes.as_deref()),
    ) {
        return fail(&cleanup_outputs(
            message,
            &[validated_assessment_path.as_path()],
        ));
    }
    print!("{receipt_json}");
    0
}

fn bounded_assessment(path: &Path) -> Result<Vec<u8>, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("could not inspect {}: {error}", path.display()))?;
    if metadata.len() > MAX_ASSESSMENT_BYTES {
        return Err("semantic assessment exceeds 1 MiB".to_string());
    }
    fs::read(path).map_err(|error| format!("could not read {}: {error}", path.display()))
}

fn record_failure(
    request: &adoc_core::SemanticExecutorRequest,
    code: &str,
    message: &str,
    receipt_path: &Path,
    validated_request: Option<(&Path, &[u8])>,
) -> i32 {
    let receipt = match fail_semantic_execution(request, code)
        .and_then(|receipt| receipt.to_canonical_json())
    {
        Ok(json) => json,
        Err(error) => return fail(&error.to_string()),
    };
    if let Err(message) = publish_receipt(receipt_path, receipt.as_bytes(), validated_request) {
        return fail(&message);
    }
    eprintln!("error[{code}] {message}");
    print!("{receipt}");
    2
}

fn publish_receipt(
    receipt_path: &Path,
    receipt: &[u8],
    validated_request: Option<(&Path, &[u8])>,
) -> Result<(), String> {
    if let Some((path, bytes)) = validated_request {
        write_atomic(path, bytes)?;
    }
    if let Err(message) = write_atomic(receipt_path, receipt) {
        return Err(match validated_request {
            Some((path, _)) => cleanup_outputs(message, &[path]),
            None => message,
        });
    }
    Ok(())
}

fn cleanup_outputs(mut message: String, paths: &[&Path]) -> String {
    if let Err(cleanup_error) = remove_outputs(paths) {
        message.push_str(&format!("; {cleanup_error}"));
    }
    message
}

fn remove_outputs(paths: &[&Path]) -> Result<(), String> {
    let mut errors = Vec::new();
    for path in paths {
        if let Err(cleanup_error) = remove_stale(path) {
            errors.push(cleanup_error);
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

fn fail(message: &str) -> i32 {
    eprintln!("error: {message}");
    2
}
