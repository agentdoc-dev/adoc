use std::fs;
use std::path::{Path, PathBuf};

use adoc_core::{
    build_semantic_context_from_document, complete_semantic_execution, fail_semantic_execution,
    validate_semantic_assessment, validate_semantic_executor_request,
};

const MAX_ASSESSMENT_BYTES: u64 = 1024 * 1024;

pub(crate) fn semantic_context(input: PathBuf, out: PathBuf) -> i32 {
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

pub(crate) fn semantic_executor(
    request_path: PathBuf,
    assessment_path: PathBuf,
    failure_code: Option<String>,
    receipt_path: PathBuf,
    validated_assessment_path: PathBuf,
) -> i32 {
    remove_stale(&validated_assessment_path);
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
    if let Some(code) = failure_code {
        return record_failure(
            &request,
            &code,
            "semantic executor invocation failed before candidate validation",
            &receipt_path,
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
            );
        }
    };
    let assessment = match validate_semantic_assessment(&assessment_bytes, request.context()) {
        Ok(assessment) => assessment,
        Err(error) => {
            return record_failure(
                &request,
                error.diagnostic_code().as_str(),
                &error.to_string(),
                &receipt_path,
            );
        }
    };
    let receipt = match complete_semantic_execution(&request, &assessment_bytes, &assessment) {
        Ok(receipt) => receipt,
        Err(error) => {
            return record_failure(
                &request,
                "assessment.semantic_schema_invalid",
                &error.to_string(),
                &receipt_path,
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
    if let Err(error) = fs::write(&validated_assessment_path, assessment_json) {
        return fail(&format!(
            "could not write {}: {error}",
            validated_assessment_path.display()
        ));
    }
    if let Err(error) = fs::write(&receipt_path, &receipt_json) {
        remove_stale(&validated_assessment_path);
        return fail(&format!(
            "could not write {}: {error}",
            receipt_path.display()
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
) -> i32 {
    let receipt = match fail_semantic_execution(request, code)
        .and_then(|receipt| receipt.to_canonical_json())
    {
        Ok(json) => json,
        Err(error) => return fail(&error.to_string()),
    };
    if let Err(error) = fs::write(receipt_path, &receipt) {
        return fail(&format!(
            "could not write {}: {error}",
            receipt_path.display()
        ));
    }
    eprintln!("error[{code}] {message}");
    print!("{receipt}");
    2
}

fn remove_stale(path: &Path) {
    if path.is_file() {
        let _ = fs::remove_file(path);
    }
}

fn fail(message: &str) -> i32 {
    eprintln!("error: {message}");
    2
}
