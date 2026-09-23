//! Exact-snapshot preparation composes the authoritative Validation Runtime.
use super::validation_runtime::{
    ValidationReceipt, ValidationResult, ValidationRuntimeInput, run_validation_runtime,
};
use crate::domain::{
    diagnostic::Diagnostic,
    hashing::sha256_prefixed,
    migration::{MIGRATION_RECEIPT_SCHEMA_VERSION, MigrationError, MigrationRequest},
    ports::snapshot_workspace::{GitRef, SnapshotSelector, SnapshotWorkspaceProvider},
};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Validator-only output; deliberately no Deserialize or public field construction.
#[derive(Debug, Serialize)]
pub struct MigrationReceipt {
    schema_version: &'static str,
    phase: &'static str,
    request: MigrationRequest,
    request_digest: String,
    validation_receipt: ValidationReceipt,
    diagnostics: Vec<Diagnostic>,
}
impl MigrationReceipt {
    pub fn result(&self) -> ValidationResult {
        self.validation_receipt.result()
    }
    pub fn to_canonical_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self).map(|json| json + "\n")
    }
}
/// The composition root privately resolves committed config; validation runs here.
pub(crate) fn prepare_with_provider(
    bytes: &[u8],
    provider: &impl SnapshotWorkspaceProvider,
    resolve: impl FnOnce(&Path) -> Result<MigrationValidationTarget, MigrationError>,
    runtime_version: String,
    runtime_binary_digest: String,
) -> Result<MigrationReceipt, MigrationError> {
    let request = MigrationRequest::parse(bytes)?;
    let snapshot = provider
        .checkout(&SnapshotSelector::GitRef(GitRef::new(
            &request.revision.value,
        )))
        .map_err(|_| MigrationError::SnapshotUnavailable)?;
    let target = resolve(snapshot.path())?;
    let outcome = run_validation_runtime(target.runtime_input(
        snapshot.path(),
        request.date()?,
        runtime_version,
        runtime_binary_digest,
    )?)
    .map_err(|_| MigrationError::ValidationUnavailable)?;
    Ok(MigrationReceipt {
        schema_version: MIGRATION_RECEIPT_SCHEMA_VERSION,
        phase: "prepare",
        request,
        request_digest: sha256_prefixed(bytes),
        validation_receipt: outcome.receipt,
        diagnostics: outcome.diagnostics,
    })
}
#[derive(Debug)]
pub(crate) struct MigrationValidationTarget {
    pub root: PathBuf,
    pub project: Option<super::compile::LocalProjectContext>,
    pub config_path: Option<PathBuf>,
    pub config_bytes: String,
}
impl MigrationValidationTarget {
    fn runtime_input(
        &self,
        snapshot: &Path,
        evaluation_date: chrono::NaiveDate,
        runtime_version: String,
        runtime_binary_digest: String,
    ) -> Result<ValidationRuntimeInput, MigrationError> {
        Ok(ValidationRuntimeInput {
            root: self.root.clone(),
            project: self.project.clone(),
            anchor_root: snapshot.to_path_buf(),
            evaluation_date,
            runtime_version,
            runtime_binary_digest,
            config_path: self.config_path.clone(),
            source_invocation: None,
            context_artifact: None,
            semantic_context: None,
            semantic_context_expectations: None,
        })
    }
}

use crate::domain::migration::{
    GENERATED_INSPECTION_CONFIG, GENERATED_INSPECTION_EXTENSIONS, GENERATED_INSPECTION_PROFILE,
    MIGRATION_IMPORT_MAX_SOURCES, REPOSITORY_INSPECTION_RECEIPT_SCHEMA_VERSION,
    RepositoryInspectionRequest,
};
/// Upper bound on distinct diagnostic codes echoed by an inspection receipt.
const INSPECTION_MAX_DIAGNOSTIC_CODES: usize = 64;

/// Read-only exact-commit inspection receipt. Grants no import, source registration
/// or promotion authority; carries codes and digests only, never source bytes.
#[derive(Debug, Serialize)]
pub struct RepositoryInspectionReceipt {
    schema_version: &'static str,
    request: RepositoryInspectionRequest,
    request_digest: String,
    runtime_version: String,
    runtime_binary_digest: String,
    config_state: &'static str,
    profile: &'static str,
    finding: &'static str,
    validation_result: &'static str,
    eligible_file_count: Option<usize>,
    parsed_item_count: Option<usize>,
    config_digest: Option<String>,
    generated_config: Option<&'static str>,
    generated_config_digest: Option<String>,
    manifest_digest: Option<String>,
    diagnostic_codes: Vec<String>,
}
impl RepositoryInspectionReceipt {
    pub fn to_canonical_json(&self) -> Result<String, MigrationError> {
        bounded_json(self)
    }
}

/// Inspect an exact snapshot with the same config resolver, safe source loader and
/// validator as migration prepare/import. Refusals (unsafe source, limits, missing
/// snapshot) return errors, never partial counts.
pub(crate) fn inspect_with_provider(
    bytes: &[u8],
    provider: &impl SnapshotWorkspaceProvider,
    resolve: impl FnOnce(&Path) -> Result<MigrationValidationTarget, MigrationError>,
    runtime_version: String,
    runtime_binary_digest: String,
) -> Result<RepositoryInspectionReceipt, MigrationError> {
    use crate::infrastructure::source::fs::FsSourceProvider;
    use std::io::Read;
    let request = RepositoryInspectionRequest::parse(bytes)?;
    request.require_runtime(&runtime_version, &runtime_binary_digest)?;
    let evaluation_date = request.date()?;
    let snapshot = provider
        .checkout(&SnapshotSelector::GitRef(GitRef::new(request.revision())))
        .map_err(|_| MigrationError::SnapshotUnavailable)?;
    let root = snapshot.path();
    let config_path = root.join("agentdoc.config.yaml");
    let committed = match std::fs::symlink_metadata(&config_path) {
        Ok(metadata) if metadata.is_file() => {
            let mut config = Vec::new();
            std::fs::File::open(&config_path)
                .and_then(|file| {
                    file.take(MIGRATION_IMPORT_MAX_BYTES as u64 + 1)
                        .read_to_end(&mut config)
                })
                .map_err(|_| MigrationError::SnapshotUnavailable)?;
            if config.len() > MIGRATION_IMPORT_MAX_BYTES {
                return Err(MigrationError::OutputLimit);
            }
            Some(config)
        }
        Ok(_) => return Err(MigrationError::UnsafeSource),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => return Err(MigrationError::SnapshotUnavailable),
    };
    let mut receipt = RepositoryInspectionReceipt {
        schema_version: REPOSITORY_INSPECTION_RECEIPT_SCHEMA_VERSION,
        request,
        request_digest: sha256_prefixed(bytes),
        runtime_version: runtime_version.clone(),
        runtime_binary_digest: runtime_binary_digest.clone(),
        config_state: "absent",
        profile: GENERATED_INSPECTION_PROFILE,
        finding: "files_without_config",
        validation_result: "not_run",
        eligible_file_count: None,
        parsed_item_count: None,
        config_digest: None,
        generated_config: None,
        generated_config_digest: None,
        manifest_digest: None,
        diagnostic_codes: Vec::new(),
    };
    let target = match committed {
        Some(config) => {
            receipt.profile = "committed";
            receipt.config_digest = Some(sha256_prefixed(&config));
            match resolve(root) {
                // Invalid committed config is reported, never replaced by the default.
                Err(_) => {
                    receipt.config_state = "invalid";
                    receipt.finding = "invalid_config";
                    return Ok(receipt);
                }
                Ok(target) => {
                    receipt.config_state = "valid";
                    receipt.finding = "configured";
                    target
                }
            }
        }
        None => {
            receipt.generated_config = Some(GENERATED_INSPECTION_CONFIG);
            receipt.generated_config_digest =
                Some(sha256_prefixed(GENERATED_INSPECTION_CONFIG.as_bytes()));
            MigrationValidationTarget {
                root: root.to_path_buf(),
                project: Some(super::compile::LocalProjectContext {
                    project_root: root.to_path_buf(),
                    docs_root: root.to_path_buf(),
                }),
                config_path: None,
                config_bytes: GENERATED_INSPECTION_CONFIG.into(),
            }
        }
    };
    let project = target
        .project
        .as_ref()
        .ok_or(MigrationError::UnsafeSource)?;
    let mut raw = FsSourceProvider::for_project(
        target.root.clone(),
        project.project_root.clone(),
        project.docs_root.clone(),
    )
    .load_raw_migration_sources_with_extensions(
        MIGRATION_IMPORT_MAX_BYTES,
        // The generated profile is `.adoc` only; `.md` is never read or counted.
        if receipt.config_state == "absent" {
            GENERATED_INSPECTION_EXTENSIONS
        } else {
            crate::domain::source::SOURCE_EXTENSIONS
        },
    )?;
    if raw.len() > MIGRATION_IMPORT_MAX_SOURCES {
        return Err(MigrationError::OutputLimit);
    }
    raw.sort_by(|left, right| left.path.cmp(&right.path));
    let manifest: Vec<_> = raw
        .iter()
        .map(|source| serde_json::json!({"path": source.path, "sha256": sha256_prefixed(&source.bytes)}))
        .collect();
    receipt.eligible_file_count = Some(raw.len());
    receipt.manifest_digest = Some(sha256_prefixed(
        &serde_json::to_vec(&manifest).map_err(|_| MigrationError::ValidationUnavailable)?,
    ));
    if raw.is_empty() {
        receipt.finding = "no_eligible_files";
        receipt.parsed_item_count = Some(0);
        return Ok(receipt);
    }
    let validated = super::validation_runtime::run_migration_snapshot_validation(
        target.runtime_input(
            root,
            evaluation_date,
            runtime_version,
            runtime_binary_digest,
        )?,
        &raw.iter()
            .map(|source| source.loaded.clone())
            .collect::<Vec<_>>(),
    )
    .map_err(|_| MigrationError::ValidationUnavailable)?;
    let codes: std::collections::BTreeSet<_> = validated
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str().to_owned())
        .collect();
    receipt.diagnostic_codes = codes
        .into_iter()
        .take(INSPECTION_MAX_DIAGNOSTIC_CODES)
        .collect();
    receipt.validation_result = match validated.receipt.result() {
        ValidationResult::Pass => "pass",
        ValidationResult::Fail => "fail",
    };
    receipt.parsed_item_count = validated.parsed_item_count;
    Ok(receipt)
}

use crate::domain::migration::{
    MIGRATION_IMPORT_MAX_BYTES, MIGRATION_IMPORT_SCHEMA_VERSION,
    MIGRATION_VALIDATION_INVOCATION_SCHEMA_VERSION, MigrationImportJob,
};

/// Candidate input evidence only; no field grants activation or promotion authority.
#[derive(Debug, Serialize)]
pub struct MigrationImportBundle {
    schema_version: &'static str,
    request: MigrationRequest,
    request_digest: String,
    job_digest: String,
    config_bytes: String,
    graph_artifact_bytes: String,
    sources: Vec<MigrationImportedSource>,
}
#[derive(Debug, Serialize)]
struct MigrationImportedSource {
    path: String,
    source_bytes: String,
    source_record_bytes: String,
    source_binding_bytes: String,
    source_invocation_bytes: String,
    validation_receipt_bytes: String,
}
#[derive(Serialize)]
struct MigrationValidationInvocation<'a> {
    schema_version: &'static str,
    workspace_id: &'a str,
    source_record_id: &'a str,
    source_record_digest: String,
    source_binding_id: &'a str,
    source_binding_digest: String,
    source_acl_snapshot_id: &'a str,
    config_digest: String,
    evaluation_date: &'a str,
    source_path: &'a str,
    request_digest: &'a str,
}
impl MigrationImportBundle {
    pub fn to_canonical_json(&self) -> Result<String, MigrationError> {
        bounded_json(self)
    }
}
fn bounded_json(value: &impl Serialize) -> Result<String, MigrationError> {
    use std::io::Write;
    let mut output = MigrationOutput(Vec::new());
    serde_json::to_writer(&mut output, value).map_err(|_| MigrationError::OutputLimit)?;
    output
        .write_all(b"\n")
        .map_err(|_| MigrationError::OutputLimit)?;
    String::from_utf8(output.0).map_err(|_| MigrationError::ValidationUnavailable)
}
struct MigrationOutput(Vec<u8>);
impl std::io::Write for MigrationOutput {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > MIGRATION_IMPORT_MAX_BYTES.saturating_sub(self.0.len()) {
            return Err(std::io::Error::other("migration output limit"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub(crate) fn import_with_provider(
    request_bytes: &[u8],
    job_bytes: &[u8],
    provider: &impl SnapshotWorkspaceProvider,
    resolve: impl FnOnce(&Path) -> Result<MigrationValidationTarget, MigrationError>,
    runtime_version: String,
    runtime_binary_digest: String,
) -> Result<MigrationImportBundle, MigrationError> {
    let request = MigrationRequest::parse(request_bytes)?;
    let job = MigrationImportJob::parse(job_bytes, &request)?;
    let snapshot = provider
        .checkout(&SnapshotSelector::GitRef(GitRef::new(
            &request.revision.value,
        )))
        .map_err(|_| MigrationError::SnapshotUnavailable)?;
    let target = resolve(snapshot.path())?;
    let input = target.runtime_input(
        snapshot.path(),
        request.date()?,
        runtime_version,
        runtime_binary_digest,
    )?;
    let validated =
        run_validation_runtime(input.clone()).map_err(|_| MigrationError::ValidationUnavailable)?;
    build_import_bundle(
        request_bytes,
        job_bytes,
        request,
        job,
        target,
        input,
        validated,
    )
}

fn build_import_bundle(
    request_bytes: &[u8],
    job_bytes: &[u8],
    request: MigrationRequest,
    job: MigrationImportJob,
    target: MigrationValidationTarget,
    input: ValidationRuntimeInput,
    validated: super::validation_runtime::ValidationRuntimeOutcome,
) -> Result<MigrationImportBundle, MigrationError> {
    use std::collections::BTreeMap;
    if validated.receipt.result() != ValidationResult::Pass {
        return Err(MigrationError::ValidationFailed);
    }
    let graph = validated
        .graph_artifact
        .ok_or(MigrationError::ValidationFailed)?;
    let metadata: BTreeMap<_, _> = job
        .sources
        .iter()
        .map(|source| (source.path.as_str(), source))
        .collect();
    if metadata.len() != validated.source_files.len() {
        return Err(MigrationError::InvalidJob);
    }
    let request_digest = sha256_prefixed(request_bytes);
    let mut bundle_bytes = graph.len().saturating_add(target.config_bytes.len());
    if bundle_bytes > MIGRATION_IMPORT_MAX_BYTES {
        return Err(MigrationError::OutputLimit);
    }
    let mut sources = Vec::new();
    for source in &validated.source_files {
        let path = source
            .logical_path
            .to_str()
            .ok_or(MigrationError::UnsafeSource)?;
        let metadata = metadata.get(path).ok_or(MigrationError::InvalidJob)?;
        let (source_record_bytes, source_binding_bytes) = source_evidence(
            &request,
            &job,
            metadata,
            source.text.as_bytes(),
            "text/plain",
        )?;
        let invocation = MigrationValidationInvocation {
            schema_version: MIGRATION_VALIDATION_INVOCATION_SCHEMA_VERSION,
            workspace_id: &request.workspace_id,
            source_record_id: &metadata.source_record_id,
            source_record_digest: sha256_prefixed(source_record_bytes.as_bytes()),
            source_binding_id: &metadata.source_binding_id,
            source_binding_digest: sha256_prefixed(source_binding_bytes.as_bytes()),
            source_acl_snapshot_id: &job.source_acl_scope.snapshot_id,
            config_digest: sha256_prefixed(target.config_bytes.as_bytes()),
            evaluation_date: &request.evaluation_date,
            source_path: path,
            request_digest: &request_digest,
        };
        let source_invocation_bytes = serde_json::to_string(&invocation)
            .map_err(|_| MigrationError::ValidationUnavailable)?;
        // ponytail: one full validation per source (maximum 512); reuse a validated
        // compiler snapshot if larger imports require measured throughput improvements.
        let validation = super::validation_runtime::run_migration_validation(
            input.clone(),
            &validated.source_files,
            source_invocation_bytes.as_bytes(),
            graph.as_bytes(),
        )
        .map_err(|_| MigrationError::ValidationUnavailable)?;
        if validation.receipt.result() != ValidationResult::Pass {
            return Err(MigrationError::ValidationFailed);
        }
        let validation_receipt_bytes = validation.receipt.to_canonical_json();
        for bytes in [
            &source.text,
            &source_record_bytes,
            &source_binding_bytes,
            &source_invocation_bytes,
            &validation_receipt_bytes,
        ] {
            bundle_bytes = bundle_bytes.saturating_add(bytes.len());
        }
        if bundle_bytes > MIGRATION_IMPORT_MAX_BYTES {
            return Err(MigrationError::OutputLimit);
        }
        sources.push(MigrationImportedSource {
            path: path.into(),
            source_bytes: source.text.clone(),
            source_record_bytes,
            source_binding_bytes,
            source_invocation_bytes,
            validation_receipt_bytes,
        });
    }
    Ok(MigrationImportBundle {
        schema_version: MIGRATION_IMPORT_SCHEMA_VERSION,
        request,
        request_digest,
        job_digest: sha256_prefixed(job_bytes),
        config_bytes: target.config_bytes,
        graph_artifact_bytes: graph,
        sources,
    })
}

fn source_evidence(
    request: &MigrationRequest,
    job: &MigrationImportJob,
    metadata: &crate::domain::migration::MigrationImportSource,
    bytes: &[u8],
    media_type: &str,
) -> Result<(String, String), MigrationError> {
    use crate::domain::source_provenance::{
        SourceBindingCoordinates, SourceBindingInput, build_source_binding,
    };
    use crate::domain::source_record::{
        RetentionClass, SourceArtifact, SourceRecordInput, build_source_record,
    };
    let path = metadata.path.as_str();
    let observed_at = chrono::DateTime::parse_from_rfc3339(&job.observed_at)
        .map_err(|_| MigrationError::InvalidJob)?
        .with_timezone(&chrono::Utc);
    let record = build_source_record(SourceRecordInput {
        source_record_id: metadata.source_record_id.clone(),
        workspace_id: request.workspace_id.clone(),
        connector_id: job.connector_id.clone(),
        source: SourceArtifact {
            provider: "git".into(),
            kind: "file".into(),
            external_id: path.into(),
            external_version: request.revision.value.clone(),
        },
        source_acl_scope: job.source_acl_scope.clone(),
        observed_at,
        media_type: media_type.into(),
        retention_class: RetentionClass::ExactCandidateInput,
        exact_bytes: bytes,
    })
    .map_err(|_| MigrationError::InvalidJob)?;
    let binding = build_source_binding(SourceBindingInput {
        source_binding_id: metadata.source_binding_id.clone(),
        workspace_id: request.workspace_id.clone(),
        source_record_id: metadata.source_record_id.clone(),
        coordinates: SourceBindingCoordinates {
            connector: "git".into(),
            source: path.into(),
            revision: Some(request.revision.value.clone()),
            path: path.into(),
            anchor: "document".into(),
            source_revision_digest: record.content_digest().into(),
        },
    })
    .map_err(|_| MigrationError::InvalidJob)?;
    let source_record_bytes = record
        .to_canonical_json()
        .map_err(|_| MigrationError::ValidationUnavailable)?;
    let source_binding_bytes = binding
        .to_canonical_json()
        .map_err(|_| MigrationError::ValidationUnavailable)?;
    Ok((source_record_bytes, source_binding_bytes))
}

use crate::domain::migration_qualification::{
    self, MIGRATION_LIFECYCLE_MAPPING_VERSION, MIGRATION_QUALIFICATION_POLICY_VERSION,
    MIGRATION_QUALIFICATION_RECEIPT_SCHEMA_VERSION, MIGRATION_QUALIFICATION_SCHEMA_VERSION,
    QualificationFreshness, QualifiedMigrationObject,
};

/// Actual-runtime evidence only. Construction is private; eligibility never grants authority.
#[derive(Debug, Serialize)]
pub struct MigrationQualification {
    schema_version: &'static str,
    request: MigrationRequest,
    request_digest: String,
    job_digest: String,
    qualification_policy_version: &'static str,
    config_bytes: String,
    #[serde(flatten)]
    outcome: QualificationOutcome,
}
#[derive(Debug, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
enum QualificationOutcome {
    Evaluated {
        candidate_bundle_bytes: String,
        qualification_receipt_bytes: String,
    },
    FlaggedSourceEvidence {
        validation_receipt_bytes: String,
        diagnostics_bytes: String,
        sources: Vec<FlaggedSourceEvidence>,
    },
}
#[derive(Debug, Serialize)]
struct FlaggedSourceEvidence {
    path: String,
    source_bytes_base64: String,
    source_record_bytes: String,
    source_binding_bytes: String,
}
#[derive(Debug, Serialize)]
struct QualificationReceipt {
    schema_version: &'static str,
    request_digest: String,
    job_digest: String,
    candidate_bundle_digest: String,
    graph_artifact_digest: String,
    config_digest: String,
    evaluation_date: String,
    qualification_policy_version: &'static str,
    lifecycle_mapping_version: &'static str,
    objects: Vec<QualifiedMigrationObject>,
}
impl MigrationQualification {
    pub fn result(&self) -> ValidationResult {
        match self.outcome {
            QualificationOutcome::Evaluated { .. } => ValidationResult::Pass,
            QualificationOutcome::FlaggedSourceEvidence { .. } => ValidationResult::Fail,
        }
    }
    pub fn to_canonical_json(&self) -> Result<String, MigrationError> {
        bounded_json(self)
    }
}

pub(crate) fn qualify_with_provider(
    request_bytes: &[u8],
    job_bytes: &[u8],
    policy_version: &str,
    provider: &impl SnapshotWorkspaceProvider,
    resolve: impl FnOnce(&Path) -> Result<MigrationValidationTarget, MigrationError>,
    runtime_version: String,
    runtime_binary_digest: String,
) -> Result<MigrationQualification, MigrationError> {
    use crate::domain::graph::GraphIndex;
    use crate::infrastructure::{
        artifact::graph_json::parse_graph_artifact_document, source::fs::FsSourceProvider,
    };
    use base64::{Engine, engine::general_purpose::STANDARD};
    migration_qualification::require_policy(policy_version)?;
    let request = MigrationRequest::parse(request_bytes)?;
    let job = MigrationImportJob::parse(job_bytes, &request)?;
    let snapshot = provider
        .checkout(&SnapshotSelector::GitRef(GitRef::new(
            &request.revision.value,
        )))
        .map_err(|_| MigrationError::SnapshotUnavailable)?;
    let target = resolve(snapshot.path())?;
    let project = target
        .project
        .as_ref()
        .ok_or(MigrationError::UnsafeSource)?;
    let raw = FsSourceProvider::for_project(
        target.root.clone(),
        project.project_root.clone(),
        project.docs_root.clone(),
    )
    .load_raw_migration_sources(MIGRATION_IMPORT_MAX_BYTES)?;
    let metadata: std::collections::BTreeMap<_, _> = job
        .sources
        .iter()
        .map(|source| (source.path.as_str(), source))
        .collect();
    if metadata.len() != raw.len()
        || raw
            .iter()
            .any(|source| !metadata.contains_key(source.path.as_str()))
    {
        return Err(MigrationError::InvalidJob);
    }
    let input = target.runtime_input(
        snapshot.path(),
        request.date()?,
        runtime_version,
        runtime_binary_digest,
    )?;
    let validated = super::validation_runtime::run_migration_snapshot_validation(
        input.clone(),
        &raw.iter()
            .map(|source| source.loaded.clone())
            .collect::<Vec<_>>(),
    )
    .map_err(|_| MigrationError::ValidationUnavailable)?;
    let request_digest = sha256_prefixed(request_bytes);
    let job_digest = sha256_prefixed(job_bytes);
    let config_bytes = target.config_bytes.clone();
    let outcome = if validated.receipt.result() == ValidationResult::Pass {
        let graph = validated
            .graph_artifact
            .as_ref()
            .ok_or(MigrationError::ValidationFailed)?;
        let graph_digest = sha256_prefixed(graph.as_bytes());
        let document = parse_graph_artifact_document(Path::new("graph_artifact"), graph.as_bytes())
            .map_err(|_| MigrationError::ValidationUnavailable)?;
        let session = super::graph::GraphSession::new(
            GraphIndex::from_document(document)
                .map_err(|_| MigrationError::ValidationUnavailable)?,
        );
        let mut freshness = QualificationFreshness::default();
        for signal in super::signals::evaluate_stale_for_date(&session, None, request.date()?) {
            match signal.category {
                super::signals::StaleCategory::Stale => {
                    freshness.stale.insert(signal.id);
                }
                super::signals::StaleCategory::ReviewOverdue => {
                    freshness.review_overdue.insert(signal.id);
                }
                super::signals::StaleCategory::ExpiringSoon => {}
            }
        }
        let objects = migration_qualification::evaluate(
            &session.objects().collect::<Vec<_>>(),
            &job.sources,
            &freshness,
            &validated.diagnostics,
            policy_version,
        )?;
        let bundle = build_import_bundle(
            request_bytes,
            job_bytes,
            request.clone(),
            job,
            target,
            input,
            validated,
        )?;
        let candidate_bundle_bytes = bundle.to_canonical_json()?;
        let receipt = QualificationReceipt {
            schema_version: MIGRATION_QUALIFICATION_RECEIPT_SCHEMA_VERSION,
            request_digest: request_digest.clone(),
            job_digest: job_digest.clone(),
            candidate_bundle_digest: sha256_prefixed(candidate_bundle_bytes.as_bytes()),
            graph_artifact_digest: graph_digest,
            config_digest: sha256_prefixed(config_bytes.as_bytes()),
            evaluation_date: request.evaluation_date.clone(),
            qualification_policy_version: MIGRATION_QUALIFICATION_POLICY_VERSION,
            lifecycle_mapping_version: MIGRATION_LIFECYCLE_MAPPING_VERSION,
            objects,
        };
        QualificationOutcome::Evaluated {
            candidate_bundle_bytes,
            qualification_receipt_bytes: bounded_json(&receipt)?,
        }
    } else {
        let mut sources = Vec::new();
        for source in raw {
            let metadata = metadata
                .get(source.path.as_str())
                .ok_or(MigrationError::InvalidJob)?;
            let (source_record_bytes, source_binding_bytes) = source_evidence(
                &request,
                &job,
                metadata,
                &source.bytes,
                "application/octet-stream",
            )?;
            sources.push(FlaggedSourceEvidence {
                path: source.path,
                source_bytes_base64: STANDARD.encode(source.bytes),
                source_record_bytes,
                source_binding_bytes,
            });
        }
        QualificationOutcome::FlaggedSourceEvidence {
            validation_receipt_bytes: validated.receipt.to_canonical_json(),
            diagnostics_bytes: serde_json::to_string(&validated.diagnostics)
                .map_err(|_| MigrationError::ValidationUnavailable)?,
            sources,
        }
    };
    Ok(MigrationQualification {
        schema_version: MIGRATION_QUALIFICATION_SCHEMA_VERSION,
        request,
        request_digest,
        job_digest,
        qualification_policy_version: MIGRATION_QUALIFICATION_POLICY_VERSION,
        config_bytes,
        outcome,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn migration_output_limits_serialized_bytes_including_json_escaping() {
        let mut output = MigrationOutput(Vec::new());
        let text = "\"".repeat(MIGRATION_IMPORT_MAX_BYTES / 2);
        assert!(serde_json::to_writer(&mut output, &text).is_err());
        assert!(output.0.len() <= MIGRATION_IMPORT_MAX_BYTES);
        let mut output = MigrationOutput(vec![b' '; MIGRATION_IMPORT_MAX_BYTES - 1]);
        assert!(output.write_all(b"x").is_ok());
        assert!(output.write_all(b"x").is_err());
        assert_eq!(output.0.len(), MIGRATION_IMPORT_MAX_BYTES);
    }
}
