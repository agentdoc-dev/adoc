//! AgentDoc Validation Runtime receipts (E1.7; SEMANTICS §S6/§S10;
//! MILESTONES §E1.7; PRD §6.7).
//!
//! All AgentDoc-domain validation runs through this pinned runtime; it
//! returns the registered `adoc.validation_receipt.v0` envelope binding the
//! exact runtime identity, every consumed input digest, the consumed
//! contract versions, the closed `pass | fail` result, and a digest of the
//! typed diagnostics. Receipts are deterministic: stable ordering, no
//! wall-clock timestamps anywhere — lifecycle evaluation pins to an
//! explicit `evaluation_date` input, never "today".
//!
//! Validator-only construction (stop-ship, MILESTONES §E1.7): the fields of
//! [`ValidationReceipt`] are private and [`run_validation_runtime`] is the
//! only constructor path, so unvalidated JSON has no core representation
//! downstream code can consume. The guarantee rests on field privacy and
//! the deliberate ABSENCE of a `Deserialize` derive — never derive
//! `Deserialize` on [`ValidationReceipt`]: a consumer of receipt bytes must
//! parse into its own shape and verify digests, or forged receipts become
//! constructible from bytes while every existing test stays green (E1.3
//! precedent, `domain/reconciliation.rs`). Compile-time visibility proofs
//! live as `compile_fail` doctests on the type.
//!
//! The runtime binary digest is supplied by the invoking harness as an
//! explicit attested input — a binary cannot hash itself deterministically.
//! The T2 harness (`scripts/validation-runtime/run.sh`) verifies the
//! binary's sha256 against its recorded pin before invoking and passes the
//! verified digest through; the receipt records what the harness attested.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use serde::Serialize;
use thiserror::Error;

use crate::application::compile::{LocalProjectContext, compile_with_provider_anchored_for_date};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, Severity};
use crate::domain::graph::{GraphArtifactDocument, GraphNode};
use crate::domain::hashing::sha256_prefixed;
use crate::domain::ports::source_provider::SourceProvider;
use crate::infrastructure::artifact::graph_json::read_graph_artifact_document;
use crate::infrastructure::source::evidence_fs::FsEvidenceFileReader;
use crate::infrastructure::source::fs::FsSourceProvider;

/// The registered contract id every serialized receipt carries.
pub(crate) const VALIDATION_RECEIPT_SCHEMA_VERSION: &str = "adoc.validation_receipt.v0";

/// The graph contract whose semantics this runtime validates; recorded under
/// `contract_versions` in every receipt.
const GRAPH_CONTRACT_KEY: &str = "graph";

/// A validation request against a resolved source root. Resolution (config
/// discovery, docs-path selection, path policy) stays in the Local Workflow
/// Layer; the runtime receives exactly what `adoc check` would compile.
#[derive(Debug, Clone)]
pub struct ValidationRuntimeInput {
    /// Source file or directory to validate (the resolved check root).
    pub root: PathBuf,
    /// Project context when the root belongs to a configured project.
    pub project: Option<LocalProjectContext>,
    /// Evidence Anchor resolution root (ADR-0048), as check threads it.
    pub anchor_root: PathBuf,
    /// Explicit lifecycle evaluation date. Mandatory: receipts never read
    /// the wall clock.
    pub evaluation_date: NaiveDate,
    /// Release version of the invoking runtime surface (the `adoc` binary).
    pub runtime_version: String,
    /// Harness-attested sha256 digest of the invoking binary
    /// (`sha256:<64 hex>`); see the module docs.
    pub runtime_binary_digest: String,
    /// Discovered project config file, digested as validation context.
    pub config_path: Option<PathBuf>,
    /// Graph artifact to validate against the recompiled source
    /// (E1.7.T4). Consumed exact-match: any `schema_version` other than
    /// `adoc.graph.v6` is rejected with `schema.unsupported_version`, and
    /// a schema-valid artifact whose governed-meaning content hashes do
    /// not correspond to the source fails with
    /// `validation.context_artifact_drift` — JSON Schema stays preflight
    /// only (ADR-0015).
    pub context_artifact: Option<PathBuf>,
}

/// A validation run: the digest-bound receipt plus the ordinary typed
/// diagnostics for human/agent presentation. `diagnostics_digest` inside the
/// receipt is computed over exactly this array's canonical serialization.
#[derive(Debug, Clone)]
pub struct ValidationRuntimeOutcome {
    pub receipt: ValidationReceipt,
    pub diagnostics: Vec<Diagnostic>,
}

/// Digest-bound `adoc.validation_receipt.v0` (SEMANTICS §S6).
///
/// Constructible only by [`run_validation_runtime`]; serialized with
/// [`ValidationReceipt::to_canonical_json`]. Unvalidated JSON has no core
/// representation:
///
/// ```compile_fail
/// // Private fields (E0451): no literal construction outside the validator
/// // module. All eight fields are named so only field privacy can fail this
/// // — a missing-fields E0063 could otherwise mask fields regressing to pub.
/// let receipt = adoc_core::ValidationReceipt {
///     schema_version: todo!(),
///     runtime: todo!(),
///     contract_versions: todo!(),
///     evaluation_date: todo!(),
///     inputs: todo!(),
///     context: todo!(),
///     result: todo!(),
///     diagnostics_digest: todo!(),
/// };
/// ```
///
/// ```compile_fail
/// // No Deserialize impl: receipt bytes cannot become the typed envelope.
/// fn requires_deserialize<T: serde::de::DeserializeOwned>() {}
/// requires_deserialize::<adoc_core::ValidationReceipt>();
/// ```
#[derive(Debug, Clone, Serialize)]
pub struct ValidationReceipt {
    schema_version: String,
    runtime: RuntimeIdentity,
    contract_versions: BTreeMap<String, String>,
    evaluation_date: String,
    inputs: Vec<DigestEntry>,
    context: Vec<NamedDigestEntry>,
    result: ValidationResult,
    diagnostics_digest: String,
}

#[derive(Debug, Clone, Serialize)]
struct RuntimeIdentity {
    version: String,
    binary_digest: String,
}

/// One consumed source input: Logical Source Path plus content digest.
#[derive(Debug, Clone, Serialize)]
struct DigestEntry {
    path: String,
    digest: String,
}

/// One named validation-context input (`config`, `context_artifact`).
#[derive(Debug, Clone, Serialize)]
struct NamedDigestEntry {
    name: String,
    digest: String,
}

/// Closed result vocabulary (CONTRACT-REGISTRY.md): `pass | fail`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationResult {
    Pass,
    Fail,
}

impl ValidationReceipt {
    /// The canonical serialized form: pretty JSON in declared field order
    /// with a trailing newline. Byte-identical across invocations, machines,
    /// and invocation surfaces for the same inputs (E1.7 exit gate).
    pub fn to_canonical_json(&self) -> String {
        let mut serialized = serde_json::to_string_pretty(self)
            .unwrap_or_else(|_| unreachable!("receipt fields serialize infallibly"));
        serialized.push('\n');
        serialized
    }

    pub fn result(&self) -> ValidationResult {
        self.result
    }
}

#[derive(Debug, Error)]
pub enum ValidationRuntimeError {
    #[error(
        "runtime binary digest '{digest}' is not 'sha256:' plus 64 lowercase hex characters; \
         the invoking harness must pass the digest it verified against its recorded pin"
    )]
    InvalidRuntimeBinaryDigest { digest: String },
    #[error("validation context '{path}' is unreadable: {message}")]
    ContextUnreadable { path: PathBuf, message: String },
}

/// Run AgentDoc-domain validation and construct the digest-bound receipt —
/// the ONLY constructor path for [`ValidationReceipt`].
pub fn run_validation_runtime(
    input: ValidationRuntimeInput,
) -> Result<ValidationRuntimeOutcome, ValidationRuntimeError> {
    require_sha256_digest(&input.runtime_binary_digest)?;

    let provider = match &input.project {
        Some(project) => FsSourceProvider::for_project(
            input.root.clone(),
            project.project_root.clone(),
            project.docs_root.clone(),
        ),
        None => FsSourceProvider::new(input.root.clone()),
    };

    let inputs = source_input_digests(&provider);
    let mut context = Vec::new();
    if let Some(config_path) = &input.config_path {
        context.push(NamedDigestEntry {
            name: "config".to_string(),
            digest: file_digest(config_path)?,
        });
    }

    if let Some(artifact_path) = &input.context_artifact {
        context.push(NamedDigestEntry {
            name: "context_artifact".to_string(),
            digest: file_digest(artifact_path)?,
        });
    }

    let reader = FsEvidenceFileReader::new(input.anchor_root.clone());
    let compiled =
        compile_with_provider_anchored_for_date(&provider, &reader, input.evaluation_date);
    let mut diagnostics = compiled.diagnostics;
    if let Some(artifact_path) = &input.context_artifact {
        diagnostics.extend(validate_context_artifact(
            artifact_path,
            compiled
                .artifacts
                .as_ref()
                .map(|artifacts| artifacts.graph_json.as_str()),
        ));
    }

    let result = if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
    {
        ValidationResult::Fail
    } else {
        ValidationResult::Pass
    };

    let diagnostics_digest = sha256_prefixed(
        &serde_json::to_vec(&diagnostics)
            .unwrap_or_else(|_| unreachable!("diagnostics serialize infallibly")),
    );

    let receipt = ValidationReceipt {
        schema_version: VALIDATION_RECEIPT_SCHEMA_VERSION.to_string(),
        runtime: RuntimeIdentity {
            version: input.runtime_version,
            binary_digest: input.runtime_binary_digest,
        },
        contract_versions: BTreeMap::from([(
            GRAPH_CONTRACT_KEY.to_string(),
            crate::infrastructure::artifact::graph_json::SUPPORTED_GRAPH_SCHEMA_VERSION.to_string(),
        )]),
        evaluation_date: input.evaluation_date.format("%Y-%m-%d").to_string(),
        inputs,
        context,
        result,
        diagnostics_digest,
    };

    Ok(ValidationRuntimeOutcome {
        receipt,
        diagnostics,
    })
}

/// Digest every source the provider yields, keyed by Logical Source Path
/// (portable, project-relative) in lexicographic order. Load failures carry
/// no digest — the compile run reports them as typed diagnostics and the
/// result fails closed.
fn source_input_digests<P: SourceProvider>(provider: &P) -> Vec<DigestEntry> {
    let mut digests = BTreeMap::new();
    for source in provider.load_sources().into_iter().flatten() {
        digests.insert(
            source.logical_path.to_string_lossy().into_owned(),
            sha256_prefixed(source.text.as_bytes()),
        );
    }
    digests
        .into_iter()
        .map(|(path, digest)| DigestEntry { path, digest })
        .collect()
}

fn file_digest(path: &PathBuf) -> Result<String, ValidationRuntimeError> {
    let bytes = fs::read(path).map_err(|error| ValidationRuntimeError::ContextUnreadable {
        path: path.clone(),
        message: error.to_string(),
    })?;
    Ok(sha256_prefixed(&bytes))
}

/// E1.7.T4: domain validation of a context artifact against the recompiled
/// source. `read_graph_artifact_document` enforces the exact-match version
/// gate (`schema.unsupported_version` for v5 and v99 alike) and JSON shape;
/// the governed-meaning comparison below is what JSON Schema can never
/// see: every Knowledge Object's `content_hash` must equal the recompiled
/// source's, and the object sets must coincide. When the source itself has
/// errors the recompiled artifact does not exist and the run already
/// fails, so only the version/shape gate applies.
fn validate_context_artifact(
    artifact_path: &Path,
    recompiled_graph_json: Option<&str>,
) -> Vec<Diagnostic> {
    let document = match read_graph_artifact_document(artifact_path) {
        Ok(document) => document,
        Err(diagnostics) => return diagnostics,
    };
    let Some(recompiled_graph_json) = recompiled_graph_json else {
        return Vec::new();
    };
    let recompiled: GraphArtifactDocument = match serde_json::from_str(recompiled_graph_json) {
        Ok(recompiled) => recompiled,
        // Compile-pipeline invariant: its own graph_json always parses.
        Err(_) => unreachable!("compile output graph_json is well-formed"),
    };

    let artifact_hashes = knowledge_object_hashes(&document);
    let recompiled_hashes = knowledge_object_hashes(&recompiled);
    let mut diagnostics = Vec::new();
    for (id, artifact_hash) in &artifact_hashes {
        match recompiled_hashes.get(id) {
            None => diagnostics.push(drift_diagnostic(
                artifact_path,
                id,
                "records a Knowledge Object the compiled source does not contain",
            )),
            Some(recompiled_hash) if recompiled_hash != artifact_hash => {
                diagnostics.push(drift_diagnostic(
                    artifact_path,
                    id,
                    "records a content_hash that does not match the recompiled source",
                ));
            }
            Some(_) => {}
        }
    }
    for id in recompiled_hashes.keys() {
        if !artifact_hashes.contains_key(id) {
            diagnostics.push(drift_diagnostic(
                artifact_path,
                id,
                "does not record a Knowledge Object the compiled source contains",
            ));
        }
    }
    diagnostics
}

fn knowledge_object_hashes(document: &GraphArtifactDocument) -> BTreeMap<String, String> {
    document
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::KnowledgeObject(object) => {
                Some((object.id.clone(), object.content_hash.clone()))
            }
            _ => None,
        })
        .collect()
}

fn drift_diagnostic(artifact_path: &Path, object_id: &str, detail: &str) -> Diagnostic {
    Diagnostic::error(
        DiagnosticCode::ValidationContextArtifactDrift,
        format!(
            "context artifact '{}' {detail} for `{object_id}`",
            artifact_path.display()
        ),
    )
    .with_object_id(object_id)
}

fn require_sha256_digest(digest: &str) -> Result<(), ValidationRuntimeError> {
    let hex = digest.strip_prefix("sha256:").unwrap_or_default();
    if hex.len() == 64
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Ok(());
    }
    Err(ValidationRuntimeError::InvalidRuntimeBinaryDigest {
        digest: digest.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    const TEST_DIGEST: &str =
        "sha256:ca1bf018dc0b72ee1197d9d521d96d227cd3e54cc81528ea5f45776c99d95f4d";

    fn write(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent dir");
        }
        fs::write(path, contents).expect("fixture write");
    }

    fn standalone_input(root: &Path) -> ValidationRuntimeInput {
        ValidationRuntimeInput {
            root: root.to_path_buf(),
            project: None,
            anchor_root: root.to_path_buf(),
            evaluation_date: NaiveDate::from_ymd_opt(2026, 1, 1).expect("valid date"),
            runtime_version: "0.4.0".to_string(),
            runtime_binary_digest: TEST_DIGEST.to_string(),
            config_path: None,
            context_artifact: None,
        }
    }

    /// Compile the workspace and return its graph artifact JSON — the
    /// honest artifact a build of the same source would publish.
    fn compiled_graph_json(root: &Path) -> String {
        let compiled = crate::compile_workspace_for_date(
            crate::CompileInput {
                root: root.to_path_buf(),
            },
            NaiveDate::from_ymd_opt(2026, 1, 1).expect("valid date"),
        );
        compiled
            .artifacts
            .expect("clean fixture compiles artifacts")
            .graph_json
    }

    fn graph_v6_schema_accepts(instance_json: &str) -> bool {
        let schema_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/agent/v0/schema/graph-artifact.v6.json");
        let schema: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(schema_path).expect("schema is readable"))
                .expect("schema is json");
        let instance: serde_json::Value =
            serde_json::from_str(instance_json).expect("artifact is json");
        jsonschema::validator_for(&schema)
            .expect("schema compiles")
            .is_valid(&instance)
    }

    fn valid_source() -> &'static str {
        "# Billing @doc(team.billing)\n\n::claim billing.ready\nstatus: draft\n--\nBilling docs are ready.\n::\n"
    }

    /// E1.7.T1 parity discipline (ADR-0015): the serialized receipt
    /// validates against the published JSON Schema — the schema documents
    /// the envelope, it never constructs it.
    #[test]
    fn serialized_receipt_validates_against_published_schema() {
        let workspace = tempfile::tempdir().expect("workspace");
        write(&workspace.path().join("docs/index.adoc"), valid_source());

        let outcome = run_validation_runtime(standalone_input(&workspace.path().join("docs")))
            .expect("validation runs");
        assert_receipt_matches_published_schema(&outcome.receipt.to_canonical_json());
    }

    /// Parity discipline (ADR-0015) helper shared by the pass- and
    /// fail-receipt tests: the serialized receipt validates against the
    /// published `adoc.validation_receipt.v0` JSON Schema.
    fn assert_receipt_matches_published_schema(receipt_json: &str) {
        let instance: serde_json::Value =
            serde_json::from_str(receipt_json).expect("receipt is json");
        let schema_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/agent/v0/schema/adoc.validation_receipt.v0.schema.json");
        let schema: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(schema_path).expect("schema is readable"))
                .expect("schema is json");
        let validator = jsonschema::validator_for(&schema).expect("schema compiles");
        let errors = validator
            .iter_errors(&instance)
            .map(|error| error.to_string())
            .collect::<Vec<_>>();
        assert!(
            errors.is_empty(),
            "adoc.validation_receipt.v0 schema validation failed:\n{}\ninstance:\n{instance:#}",
            errors.join("\n")
        );
    }

    /// Receipt determinism: same inputs, byte-identical canonical JSON, and
    /// the digest fields bind runtime identity, inputs, and diagnostics.
    #[test]
    fn receipt_is_deterministic_and_digest_bound() {
        let workspace = tempfile::tempdir().expect("workspace");
        write(&workspace.path().join("docs/index.adoc"), valid_source());
        let input = standalone_input(&workspace.path().join("docs"));

        let first = run_validation_runtime(input.clone()).expect("first run");
        let second = run_validation_runtime(input).expect("second run");
        assert_eq!(
            first.receipt.to_canonical_json(),
            second.receipt.to_canonical_json(),
            "receipts must be byte-identical across invocations"
        );

        let value: serde_json::Value =
            serde_json::from_str(&first.receipt.to_canonical_json()).expect("receipt is json");
        assert_eq!(value["schema_version"], "adoc.validation_receipt.v0");
        assert_eq!(value["runtime"]["binary_digest"], TEST_DIGEST);
        assert_eq!(value["contract_versions"]["graph"], "adoc.graph.v6");
        assert_eq!(value["result"], "pass");
        assert_eq!(value["inputs"][0]["path"], "index.adoc");
        assert_eq!(
            value["diagnostics_digest"],
            sha256_prefixed(&serde_json::to_vec(&first.diagnostics).expect("serializable")),
            "diagnostics_digest is the sha256 of the canonically serialized diagnostics array"
        );
    }

    /// Domain-invalid source (a dangling reference parses cleanly but
    /// violates domain rules): the runtime fails closed and the receipt
    /// records the failure with a digest over the typed diagnostics.
    #[test]
    fn domain_invalid_source_yields_fail_receipt() {
        let workspace = tempfile::tempdir().expect("workspace");
        write(
            &workspace.path().join("docs/index.adoc"),
            "# Billing @doc(team.billing)\n\n::claim billing.ready\nstatus: draft\ndepends_on: [billing.missing]\n--\nBilling docs are ready.\n::\n",
        );

        let outcome = run_validation_runtime(standalone_input(&workspace.path().join("docs")))
            .expect("validation runs");
        assert_eq!(outcome.receipt.result(), ValidationResult::Fail);
        assert!(
            outcome
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == Severity::Error),
            "expected typed error diagnostics, got: {:?}",
            outcome.diagnostics
        );
    }

    /// E1.7.T4 baseline: an honest artifact of the same source passes; the
    /// receipt records its digest as context and the consumed graph
    /// contract version.
    #[test]
    fn matching_context_artifact_passes_and_is_digest_bound() {
        let workspace = tempfile::tempdir().expect("workspace");
        let docs = workspace.path().join("docs");
        write(&docs.join("index.adoc"), valid_source());
        let artifact_path = workspace.path().join("dist/docs.graph.json");
        write(&artifact_path, &compiled_graph_json(&docs));

        let mut input = standalone_input(&docs);
        input.context_artifact = Some(artifact_path);
        let outcome = run_validation_runtime(input).expect("validation runs");

        assert_eq!(outcome.receipt.result(), ValidationResult::Pass);
        let value: serde_json::Value =
            serde_json::from_str(&outcome.receipt.to_canonical_json()).expect("receipt is json");
        assert_eq!(value["context"][0]["name"], "context_artifact");
        assert_eq!(value["contract_versions"]["graph"], "adoc.graph.v6");
    }

    /// E1.7.T4 stop-ship cut: a context artifact the published JSON Schema
    /// ACCEPTS but whose governed-meaning content_hash does not correspond
    /// to the source is rejected by the runtime with typed diagnostics,
    /// and the receipt records the failure. JSON Schema stays
    /// preflight/documentation only (ADR-0015): schema validity alone can
    /// never make a payload domain-valid, and a preflight that accepted
    /// this fixture would be overruled by this runtime.
    #[test]
    fn schema_valid_domain_invalid_context_artifact_fails_closed() {
        let workspace = tempfile::tempdir().expect("workspace");
        let docs = workspace.path().join("docs");
        write(&docs.join("index.adoc"), valid_source());
        let honest = compiled_graph_json(&docs);
        let honest_value: serde_json::Value = serde_json::from_str(&honest).expect("json");
        let real_hash = honest_value["nodes"]
            .as_array()
            .expect("nodes array")
            .iter()
            .find_map(|node| {
                (node["type"] == "knowledge_object").then(|| {
                    node["content_hash"]
                        .as_str()
                        .expect("content_hash string")
                        .to_string()
                })
            })
            .expect("fixture has a knowledge object");
        let forged_hash = format!("sha256:{}", "0".repeat(64));
        let forged = honest.replace(&real_hash, &forged_hash);
        assert!(
            graph_v6_schema_accepts(&forged),
            "the forged artifact must stay JSON-Schema-valid — the cut proves schema \
             validity is not domain validity"
        );
        let artifact_path = workspace.path().join("dist/docs.graph.json");
        write(&artifact_path, &forged);

        let mut input = standalone_input(&docs);
        input.context_artifact = Some(artifact_path);
        let outcome = run_validation_runtime(input).expect("validation runs");

        assert_eq!(outcome.receipt.result(), ValidationResult::Fail);
        assert!(
            outcome.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == DiagnosticCode::ValidationContextArtifactDrift
                    && diagnostic.severity == Severity::Error
            }),
            "expected validation.context_artifact_drift, got: {:?}",
            outcome.diagnostics
        );
        // The fail receipt is still a valid envelope of the published
        // receipt schema (parity discipline covers both results).
        let canonical = outcome.receipt.to_canonical_json();
        assert_receipt_matches_published_schema(&canonical);
        let receipt_value: serde_json::Value =
            serde_json::from_str(&canonical).expect("receipt is json");
        assert_eq!(receipt_value["result"], "fail");
    }

    /// E1.7.T4: unknown envelope versions reject exact-match — an older
    /// v5 and an unknown v99 alike — with `schema.unsupported_version`;
    /// the receipt records the failure and no drift comparison runs.
    #[test]
    fn unknown_context_artifact_version_rejects_exact_match() {
        let workspace = tempfile::tempdir().expect("workspace");
        let docs = workspace.path().join("docs");
        write(&docs.join("index.adoc"), valid_source());
        let honest = compiled_graph_json(&docs);

        for version in ["adoc.graph.v5", "adoc.graph.v99"] {
            let artifact_path = workspace.path().join("dist/docs.graph.json");
            write(&artifact_path, &honest.replace("adoc.graph.v6", version));

            let mut input = standalone_input(&docs);
            input.context_artifact = Some(artifact_path);
            let outcome = run_validation_runtime(input).expect("validation runs");

            assert_eq!(outcome.receipt.result(), ValidationResult::Fail);
            assert!(
                outcome
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code == DiagnosticCode::SchemaUnsupportedVersion),
                "{version}: expected schema.unsupported_version, got: {:?}",
                outcome.diagnostics
            );
            assert!(
                !outcome
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code
                        == DiagnosticCode::ValidationContextArtifactDrift),
                "{version}: version gate must reject before any drift comparison"
            );
        }
    }

    /// The harness-attested digest is validated at the boundary: anything
    /// but `sha256:` + 64 lowercase hex refuses before any validation runs.
    #[test]
    fn malformed_runtime_binary_digest_is_refused() {
        let workspace = tempfile::tempdir().expect("workspace");
        write(&workspace.path().join("docs/index.adoc"), valid_source());
        for bad in [
            "",
            "sha256:",
            "sha256:short",
            "sha256:CA1BF018DC0B72EE1197D9D521D96D227CD3E54CC81528EA5F45776C99D95F4D",
            "md5:ca1bf018dc0b72ee1197d9d521d96d227cd3e54cc81528ea5f45776c99d95f4d",
        ] {
            let mut input = standalone_input(&workspace.path().join("docs"));
            input.runtime_binary_digest = bad.to_string();
            assert!(
                matches!(
                    run_validation_runtime(input),
                    Err(ValidationRuntimeError::InvalidRuntimeBinaryDigest { .. })
                ),
                "digest {bad:?} must refuse"
            );
        }
    }
}
