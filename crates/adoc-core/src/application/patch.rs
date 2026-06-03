use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use serde::Serialize;

use crate::domain::diagnostic::{Diagnostic, Severity};
use crate::domain::graph::{GraphArtifactDocument, GraphIndex};
use crate::domain::obligation::ProofObligation;
use crate::domain::patch::{
    AffectedRelation, PatchDiff, PatchDocument, PatchValidationReport, validate_patch,
};
use crate::domain::ports::artifact_reader::ArtifactReader;

pub const PATCH_CHECK_SCHEMA_VERSION: &str = "adoc.patch.check.v0";

/// Error returned by the V3.7 patch parsing helpers
/// ([`crate::parse_patch_from_path`] and [`crate::parse_patch_from_value`])
/// when the supplied patch source cannot be turned into a [`PatchDocument`].
///
/// V3.7 surface (introduced for `adoc review --patch`): a failed parse must
/// be distinguishable from a successful parse that fails validation. The
/// latter is reported inside [`PatchCheckResult::diagnostics`]; the former
/// reaches callers as this typed error so the orchestration layer can map it
/// into a higher-level review error rather than a misleading `valid: false`
/// envelope without a target object.
///
/// This enum lives in `application/patch.rs` (a pure value type) while the
/// constructors live in `lib.rs` — the application layer is forbidden from
/// importing `infrastructure/` directly per ADR-0006 and the
/// `patch_application_layer_does_not_reference_infrastructure` boundary
/// test in `crates/adoc-core/tests/public_surface.rs`.
#[non_exhaustive]
#[derive(Debug)]
pub enum PatchParseError {
    /// Reading or parsing the patch file at `path` failed. `diagnostics`
    /// carry the file/line/column context produced by the infrastructure
    /// reader (typically `IoArtifactMalformed` or `PatchInvalidDocument`).
    Read {
        path: PathBuf,
        diagnostics: Vec<Diagnostic>,
    },
    /// Parsing an inline `serde_json::Value` into a [`PatchDocument`] failed.
    /// `diagnostics` carry the structural reason (typically
    /// `PatchInvalidDocument`).
    Inline { diagnostics: Vec<Diagnostic> },
}

impl fmt::Display for PatchParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, diagnostics } => write!(
                f,
                "could not parse patch document at {} ({} diagnostics)",
                path.display(),
                diagnostics.len()
            ),
            Self::Inline { diagnostics } => write!(
                f,
                "could not parse inline patch document ({} diagnostics)",
                diagnostics.len()
            ),
        }
    }
}

impl Error for PatchParseError {}

impl PatchParseError {
    /// Surfaces the underlying [`Diagnostic`] list to callers that want to
    /// embed parse failures into a downstream envelope (e.g. as the
    /// `diagnostics` field of a synthetic `PatchCheckResult::failure`).
    pub fn diagnostics(&self) -> &[Diagnostic] {
        match self {
            Self::Read { diagnostics, .. } | Self::Inline { diagnostics } => diagnostics,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PatchInput {
    pub graph_artifact_path: PathBuf,
    pub patch_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct PatchJsonInput {
    pub graph_artifact_path: PathBuf,
    pub patch: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PatchCheckResult {
    pub schema_version: &'static str,
    pub valid: bool,
    pub accepted_for_review: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    pub operation: String,
    pub diffs: Vec<PatchDiff>,
    pub affected_relations: Vec<AffectedRelation>,
    pub proof_obligations: Vec<ProofObligation>,
    pub required_follow_up: Vec<String>,
    pub diagnostics: Vec<Diagnostic>,
}

impl PatchCheckResult {
    fn from_report(report: PatchValidationReport, mut load_diagnostics: Vec<Diagnostic>) -> Self {
        load_diagnostics.extend(report.diagnostics);
        let valid = report.valid && !diagnostics_have_errors(&load_diagnostics);
        Self {
            schema_version: PATCH_CHECK_SCHEMA_VERSION,
            valid,
            accepted_for_review: valid && report.accepted_for_review,
            target: report.target,
            operation: report.operation.as_str().to_string(),
            diffs: report.diffs,
            affected_relations: report.affected_relations,
            proof_obligations: report.proof_obligations,
            required_follow_up: report.required_follow_up,
            diagnostics: load_diagnostics,
        }
    }

    pub(crate) fn failure(diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            schema_version: PATCH_CHECK_SCHEMA_VERSION,
            valid: false,
            accepted_for_review: false,
            target: None,
            operation: String::new(),
            diffs: Vec::new(),
            affected_relations: Vec::new(),
            proof_obligations: Vec::new(),
            required_follow_up: Vec::new(),
            diagnostics,
        }
    }
}

pub(crate) fn check_patch_with_readers<G, P>(
    input: PatchInput,
    graph_reader: &G,
    patch_reader: &P,
) -> PatchCheckResult
where
    G: ArtifactReader<Output = GraphArtifactDocument>,
    P: ArtifactReader<Output = PatchDocument>,
{
    let graph_document = match graph_reader.read(&input.graph_artifact_path) {
        Ok(document) => document,
        Err(diagnostics) => return PatchCheckResult::failure(diagnostics),
    };
    let patch_document = match patch_reader.read(&input.patch_path) {
        Ok(document) => document,
        Err(diagnostics) => return PatchCheckResult::failure(diagnostics),
    };

    check_patch_documents(graph_document, patch_document)
}

pub(crate) fn check_patch_documents(
    graph_document: GraphArtifactDocument,
    patch_document: PatchDocument,
) -> PatchCheckResult {
    let document_diagnostics = graph_document.diagnostics.clone();
    if diagnostics_have_errors(&document_diagnostics) {
        return PatchCheckResult::failure(document_diagnostics);
    }

    let graph = match GraphIndex::from_document(graph_document) {
        Ok(graph) => graph,
        Err(mut graph_diagnostics) => {
            let mut diagnostics = document_diagnostics;
            diagnostics.append(&mut graph_diagnostics);
            return PatchCheckResult::failure(diagnostics);
        }
    };

    let report = validate_patch(&graph, patch_document);
    PatchCheckResult::from_report(report, document_diagnostics)
}

fn diagnostics_have_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::Path;

    use crate::domain::diagnostic::{DiagnosticCode, Severity};
    use crate::domain::graph::{
        GraphEdge, GraphKnowledgeObjectNode, GraphNode, GraphPageNode, GraphRelations,
        GraphSourceSpan,
    };
    use crate::domain::patch::{PatchDocument, PatchIntent, PlacementHint};

    use super::*;

    #[derive(Clone)]
    struct StubGraphReader {
        result: Result<GraphArtifactDocument, Vec<Diagnostic>>,
    }

    impl ArtifactReader for StubGraphReader {
        type Output = GraphArtifactDocument;

        fn read(&self, _path: &Path) -> Result<Self::Output, Vec<Diagnostic>> {
            self.result.clone()
        }
    }

    #[derive(Clone)]
    struct StubPatchReader {
        result: Result<PatchDocument, Vec<Diagnostic>>,
    }

    impl ArtifactReader for StubPatchReader {
        type Output = PatchDocument;

        fn read(&self, _path: &Path) -> Result<Self::Output, Vec<Diagnostic>> {
            self.result.clone()
        }
    }

    fn graph_document(objects: Vec<GraphKnowledgeObjectNode>) -> GraphArtifactDocument {
        GraphArtifactDocument {
            schema_version: "adoc.graph.v3".to_string(),
            nodes: std::iter::once(GraphNode::Page(GraphPageNode {
                id: "team.page".to_string(),
                order: 0,
                title: Some("Team".to_string()),
                source_path: "docs/team.adoc".to_string(),
            }))
            .chain(objects.into_iter().map(GraphNode::KnowledgeObject))
            .collect(),
            edges: Vec::<GraphEdge>::new(),
            diagnostics: Vec::new(),
        }
    }

    fn object(id: &str) -> GraphKnowledgeObjectNode {
        GraphKnowledgeObjectNode {
            id: id.to_string(),
            kind: "claim".to_string(),
            content_hash: format!("sha256:{id}"),
            status: Some("draft".to_string()),
            body: "Body.".to_string(),
            page_id: "team.page".to_string(),
            source_span: GraphSourceSpan {
                path: "docs/team.adoc".to_string(),
                line: 1,
                column: 1,
            },
            fields: BTreeMap::new(),
            relations: GraphRelations::default(),
            impacts: Vec::new(),
            approved_by: Vec::new(),
            allowed_actions: Vec::new(),
            forbidden_actions: Vec::new(),
            contradiction_claims: Vec::new(),
            evidence: Vec::new(),
            effective_status: None,
            effective_reason: None,
        }
    }

    fn patch(intent: PatchIntent) -> PatchDocument {
        PatchDocument {
            target: "billing.credits".to_string(),
            intent,
            reason: "review update".to_string(),
            proposer: None,
        }
    }

    fn run(graph: GraphArtifactDocument, patch: PatchDocument) -> PatchCheckResult {
        check_patch_with_readers(
            PatchInput {
                graph_artifact_path: PathBuf::from("graph.json"),
                patch_path: PathBuf::from("patch.json"),
            },
            &StubGraphReader { result: Ok(graph) },
            &StubPatchReader { result: Ok(patch) },
        )
    }

    #[test]
    fn valid_patch_returns_check_envelope() {
        let graph = graph_document(vec![object("billing.credits")]);
        let patch = patch(PatchIntent::ReplaceBody {
            base_hash: "sha256:billing.credits".to_string(),
            body: "Updated body.".to_string(),
        });

        let result = run(graph, patch);

        assert!(result.valid);
        assert!(result.accepted_for_review);
        assert_eq!(result.schema_version, PATCH_CHECK_SCHEMA_VERSION);
        assert_eq!(result.operation, "replace_body");
        assert_eq!(result.target.as_deref(), Some("billing.credits"));
        assert_eq!(result.diffs[0].field, "body");
    }

    #[test]
    fn stale_base_hash_is_exit_mappable_validation_failure() {
        let graph = graph_document(vec![object("billing.credits")]);
        let patch = patch(PatchIntent::ReplaceBody {
            base_hash: "sha256:stale".to_string(),
            body: "Updated body.".to_string(),
        });

        let result = run(graph, patch);

        assert!(!result.valid);
        assert_eq!(
            result.diagnostics[0].code,
            DiagnosticCode::PatchBaseHashMismatch
        );
    }

    #[test]
    fn target_not_found_is_reported() {
        let graph = graph_document(Vec::new());
        let patch = patch(PatchIntent::ReplaceBody {
            base_hash: "sha256:billing.credits".to_string(),
            body: "Updated body.".to_string(),
        });

        let result = run(graph, patch);

        assert!(!result.valid);
        assert_eq!(
            result.diagnostics[0].code,
            DiagnosticCode::GraphObjectNotFound
        );
    }

    #[test]
    fn create_target_already_exists_is_rejected() {
        let graph = graph_document(vec![object("billing.credits")]);
        let patch = PatchDocument {
            target: "billing.credits".to_string(),
            intent: PatchIntent::CreateObject {
                kind: "claim".to_string(),
                status: Some("draft".to_string()),
                body: "Created body.".to_string(),
                fields: BTreeMap::new(),
                placement: PlacementHint {
                    page_id: "team.page".to_string(),
                    after: None,
                },
            },
            reason: "create object".to_string(),
            proposer: None,
        };

        let result = run(graph, patch);

        assert!(!result.valid);
        assert_eq!(
            result.diagnostics[0].code,
            DiagnosticCode::PatchTargetAlreadyExists
        );
    }

    #[test]
    fn graph_artifact_diagnostics_block_patch_validation() {
        let mut graph = graph_document(vec![object("billing.credits")]);
        graph.diagnostics.push(Diagnostic {
            code: DiagnosticCode::ParseRawHtml,
            severity: Severity::Error,
            message: "source error".to_string(),
            span: None,
            object_id: None,
            help: None,
        });
        let patch = patch(PatchIntent::ReplaceBody {
            base_hash: "sha256:billing.credits".to_string(),
            body: "Updated body.".to_string(),
        });

        let result = run(graph, patch);

        assert!(!result.valid);
        assert_eq!(result.diagnostics[0].code, DiagnosticCode::ParseRawHtml);
    }

    #[test]
    fn graph_index_validation_rejects_stub_reader_missing_content_hash() {
        let mut object = object("billing.credits");
        object.content_hash.clear();
        let graph = graph_document(vec![object]);
        let patch = patch(PatchIntent::ReplaceBody {
            base_hash: "sha256:billing.credits".to_string(),
            body: "Updated body.".to_string(),
        });

        let result = run(graph, patch);

        assert!(!result.valid);
        assert_eq!(
            result.diagnostics[0].code,
            DiagnosticCode::IoArtifactMalformed
        );
        assert_eq!(
            result.diagnostics[0].object_id.as_deref(),
            Some("billing.credits")
        );
    }

    #[test]
    fn patch_parse_error_display_mentions_diagnostics_count_and_path() {
        let err = PatchParseError::Read {
            path: PathBuf::from("/tmp/x.json"),
            diagnostics: vec![
                Diagnostic::error(DiagnosticCode::IoArtifactUnreadable, "missing file"),
                Diagnostic::error(DiagnosticCode::IoArtifactMalformed, "bad json"),
            ],
        };
        let msg = err.to_string();
        assert!(msg.contains("/tmp/x.json"));
        assert!(msg.contains("2 diagnostics"));
    }

    #[test]
    fn patch_parse_error_inline_display_mentions_diagnostics_count() {
        let err = PatchParseError::Inline {
            diagnostics: vec![Diagnostic::error(
                DiagnosticCode::PatchInvalidDocument,
                "bad shape",
            )],
        };
        let msg = err.to_string();
        assert!(msg.contains("inline patch"));
        assert!(msg.contains("1 diagnostics"));
    }

    #[test]
    fn patch_parse_error_diagnostics_accessor_returns_underlying_list() {
        let err = PatchParseError::Inline {
            diagnostics: vec![Diagnostic::error(DiagnosticCode::PatchInvalidDocument, "x")],
        };
        assert_eq!(err.diagnostics().len(), 1);
    }

    #[test]
    fn patch_reader_errors_surface_without_graph_validation() {
        let result = check_patch_with_readers(
            PatchInput {
                graph_artifact_path: PathBuf::from("graph.json"),
                patch_path: PathBuf::from("patch.json"),
            },
            &StubGraphReader {
                result: Ok(graph_document(vec![object("billing.credits")])),
            },
            &StubPatchReader {
                result: Err(vec![Diagnostic::error(
                    DiagnosticCode::PatchInvalidDocument,
                    "unsupported patch schema",
                )]),
            },
        );

        assert!(!result.valid);
        assert_eq!(
            result.diagnostics[0].code,
            DiagnosticCode::PatchInvalidDocument
        );
    }
}
