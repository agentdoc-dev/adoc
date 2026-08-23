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
use std::path::PathBuf;

use chrono::NaiveDate;
use serde::Serialize;
use thiserror::Error;

use crate::application::compile::{LocalProjectContext, compile_with_provider_anchored_for_date};
use crate::domain::diagnostic::{Diagnostic, Severity};
use crate::domain::hashing::sha256_prefixed;
use crate::domain::ports::source_provider::SourceProvider;
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
/// // Private fields: no literal construction outside the validator module.
/// let receipt = adoc_core::ValidationReceipt {};
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

    let reader = FsEvidenceFileReader::new(input.anchor_root.clone());
    let compiled =
        compile_with_provider_anchored_for_date(&provider, &reader, input.evaluation_date);
    let diagnostics = compiled.diagnostics;

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
        }
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
        let instance: serde_json::Value =
            serde_json::from_str(&outcome.receipt.to_canonical_json()).expect("receipt is json");

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
