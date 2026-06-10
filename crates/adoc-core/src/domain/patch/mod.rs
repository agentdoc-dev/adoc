use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, Severity};
use crate::domain::graph::{GraphIndex, GraphKnowledgeObjectNode, GraphRelationKind};
use crate::domain::identity::{OBJECT_ID_GRAMMAR_HELP, ObjectId};
use crate::domain::knowledge_object::EVIDENCE_REF_FIELD;
use crate::domain::knowledge_object::draft::{KnowledgeObjectDraft, validate_draft};
use crate::domain::obligation::ProofObligation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchDocument {
    pub(crate) target: String,
    pub(crate) intent: PatchIntent,
    pub(crate) reason: String,
    pub(crate) proposer: Option<PatchProposer>,
}

impl PatchDocument {
    pub(crate) fn operation(&self) -> PatchOperation {
        self.intent.operation()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchOperation {
    ReplaceBody,
    UpdateFields,
    CreateObject,
    Supersede,
    Revoke,
}

impl PatchOperation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReplaceBody => "replace_body",
            Self::UpdateFields => "update_fields",
            Self::CreateObject => "create_object",
            Self::Supersede => "supersede",
            Self::Revoke => "revoke",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PatchIntent {
    ReplaceBody {
        base_hash: String,
        body: String,
    },
    UpdateFields {
        base_hash: String,
        fields: BTreeMap<String, String>,
    },
    CreateObject {
        kind: String,
        status: Option<String>,
        body: String,
        fields: BTreeMap<String, String>,
        placement: PlacementHint,
    },
    Supersede {
        base_hash: String,
        supersedes: Vec<String>,
    },
    Revoke {
        base_hash: String,
    },
}

impl PatchIntent {
    pub(crate) fn operation(&self) -> PatchOperation {
        match self {
            Self::ReplaceBody { .. } => PatchOperation::ReplaceBody,
            Self::UpdateFields { .. } => PatchOperation::UpdateFields,
            Self::CreateObject { .. } => PatchOperation::CreateObject,
            Self::Supersede { .. } => PatchOperation::Supersede,
            Self::Revoke { .. } => PatchOperation::Revoke,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlacementHint {
    pub(crate) page_id: String,
    pub(crate) after: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PatchProposer {
    pub(crate) proposer_type: String,
    pub(crate) id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PatchDiff {
    pub field: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AffectedRelation {
    pub source: String,
    pub relation: GraphRelationKind,
    pub target: String,
    pub action: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PatchValidationReport {
    pub(crate) valid: bool,
    pub(crate) accepted_for_review: bool,
    pub(crate) target: Option<String>,
    pub(crate) operation: PatchOperation,
    pub(crate) diffs: Vec<PatchDiff>,
    pub(crate) affected_relations: Vec<AffectedRelation>,
    pub(crate) proof_obligations: Vec<ProofObligation>,
    pub(crate) required_follow_up: Vec<String>,
    pub(crate) diagnostics: Vec<Diagnostic>,
}

pub(crate) fn validate_patch(graph: &GraphIndex, patch: PatchDocument) -> PatchValidationReport {
    let mut validator = PatchValidator {
        graph,
        patch,
        diagnostics: Vec::new(),
        diffs: Vec::new(),
        affected_relations: Vec::new(),
        proof_obligations: Vec::new(),
        required_follow_up: Vec::new(),
    };
    validator.validate()
}

struct PatchValidator<'a> {
    graph: &'a GraphIndex,
    patch: PatchDocument,
    diagnostics: Vec<Diagnostic>,
    diffs: Vec<PatchDiff>,
    affected_relations: Vec<AffectedRelation>,
    proof_obligations: Vec<ProofObligation>,
    required_follow_up: Vec<String>,
}

impl PatchValidator<'_> {
    fn validate(&mut self) -> PatchValidationReport {
        self.validate_reason();

        let target = match ObjectId::new(self.patch.target.clone()) {
            Ok(target) => target,
            Err(_) => {
                self.diagnostics.push(invalid_object_id_diagnostic(
                    self.patch.target.clone(),
                    "patch target",
                ));
                return self.finish(None);
            }
        };

        match self.patch.intent.clone() {
            PatchIntent::ReplaceBody { base_hash, body } => {
                self.validate_replace_body(&target, &base_hash, &body)
            }
            PatchIntent::UpdateFields { base_hash, fields } => {
                self.validate_update_fields(&target, &base_hash, fields)
            }
            PatchIntent::CreateObject {
                kind,
                status,
                body,
                fields,
                placement,
            } => self.validate_create_object(
                &target,
                &kind,
                status.as_deref(),
                &body,
                fields,
                &placement,
            ),
            PatchIntent::Supersede {
                base_hash,
                supersedes,
            } => self.validate_supersede(&target, &base_hash, supersedes),
            PatchIntent::Revoke { base_hash } => self.validate_revoke(&target, &base_hash),
        }

        self.finish(Some(target.to_string()))
    }

    fn finish(&mut self, target: Option<String>) -> PatchValidationReport {
        let valid = !self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error);
        PatchValidationReport {
            valid,
            accepted_for_review: valid,
            target,
            operation: self.patch.operation(),
            diffs: std::mem::take(&mut self.diffs),
            affected_relations: std::mem::take(&mut self.affected_relations),
            proof_obligations: std::mem::take(&mut self.proof_obligations),
            required_follow_up: std::mem::take(&mut self.required_follow_up),
            diagnostics: std::mem::take(&mut self.diagnostics),
        }
    }

    fn validate_reason(&mut self) {
        if self.patch.reason.trim().is_empty() {
            self.diagnostics.push(
                Diagnostic::error(
                    DiagnosticCode::PatchInvalidDocument,
                    "patch reason must be a non-empty string",
                )
                .with_help("Add a short review reason explaining why this patch is proposed."),
            );
        }
    }

    fn validate_replace_body(&mut self, target: &ObjectId, base_hash: &str, body: &str) {
        let Some(object) = self.require_existing_target(target) else {
            return;
        };
        if !self.require_matching_base_hash(&object, base_hash) {
            return;
        }
        if body.trim().is_empty() {
            self.diagnostics.push(validation_error(
                target.as_str(),
                "replace_body requires a non-empty changes.body value",
            ));
            return;
        }
        self.diffs.push(value_diff("body", &object.body, body));
        self.add_verified_claim_obligation_if_needed(
            &object,
            "Verified claim body changes require evidence review before approval.",
        );
    }

    fn validate_update_fields(
        &mut self,
        target: &ObjectId,
        base_hash: &str,
        fields: BTreeMap<String, String>,
    ) {
        let Some(object) = self.require_existing_target(target) else {
            return;
        };
        if !self.require_matching_base_hash(&object, base_hash) {
            return;
        }
        for (key, value) in fields {
            if !is_valid_field_key(&key) {
                self.diagnostics.push(validation_error(
                    target.as_str(),
                    format!("field key `{key}` is invalid"),
                ));
                continue;
            }
            if is_relation_field(&key) {
                self.diagnostics.push(validation_error(
                    target.as_str(),
                    format!("field `{key}` is a relation field; use a relation operation"),
                ));
                continue;
            }
            if value.trim().is_empty() {
                self.diagnostics.push(validation_error(
                    target.as_str(),
                    format!("field `{key}` requires a non-empty value"),
                ));
                continue;
            }
            // V5.8 TB5: when the field being updated is `evidence_ref`, resolve
            // each comma-separated Object ID against the head graph artifact.
            // On a parse failure we emit an existing validation error and skip
            // the diff (hard error — the value is syntactically invalid).
            // On a resolution failure (not found / wrong kind) we emit the
            // schema diagnostic AND still record the diff: the patch is a
            // proposal and the diff is part of the record even when the ref is
            // broken; reviewers need to see what was proposed. This mirrors the
            // supersede path which records affected_relations even when targets
            // are stale, and keeps the diff stream consistent with what other
            // field updates produce.
            if key == EVIDENCE_REF_FIELD {
                let mut ref_parse_error = false;
                for raw_segment in value.split(',') {
                    let trimmed = raw_segment.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let ref_id = match ObjectId::new(trimmed.to_string()) {
                        Ok(id) => id,
                        Err(_) => {
                            self.diagnostics
                                .push(invalid_object_id_diagnostic(trimmed, "evidence_ref target"));
                            ref_parse_error = true;
                            continue;
                        }
                    };
                    match self.graph.object(&ref_id) {
                        None => {
                            self.diagnostics.push(
                                Diagnostic::error(
                                    DiagnosticCode::SchemaEvidenceTargetNotFound,
                                    format!(
                                        "patch for `{target}` references unknown object `{ref_id}` in `evidence_ref`; no object with that id exists in the graph artifact",
                                    ),
                                )
                                .with_object_id(target.as_str()),
                            );
                        }
                        Some(node) if node.kind != "source" => {
                            let actual_kind = node.kind.clone();
                            self.diagnostics.push(
                                Diagnostic::error(
                                    DiagnosticCode::SchemaEvidenceTargetNotASource,
                                    format!(
                                        "patch for `{target}` references `{ref_id}` in `evidence_ref`, but that object is a `{actual_kind}`, not a `source`",
                                    ),
                                )
                                .with_object_id(target.as_str()),
                            );
                        }
                        Some(_) => {} // exists and is a source — OK
                    }
                }
                // Only skip the diff if the value itself was syntactically
                // invalid (unparseable IDs). Resolution failures still produce
                // a diff because the proposal is recorded as-is.
                if ref_parse_error {
                    continue;
                }
            }
            let old = object.fields.get(&key).cloned();
            self.diffs
                .push(option_value_diff(format!("fields.{key}"), old, Some(value)));
        }
        self.add_verified_claim_obligation_if_needed(
            &object,
            "Verified claim field changes require evidence review before approval.",
        );
    }

    fn validate_create_object(
        &mut self,
        target: &ObjectId,
        kind: &str,
        status: Option<&str>,
        body: &str,
        fields: BTreeMap<String, String>,
        placement: &PlacementHint,
    ) {
        if self.graph.contains_object(target) {
            self.diagnostics.push(
                Diagnostic::error(
                    DiagnosticCode::PatchTargetAlreadyExists,
                    format!("create_object target `{target}` already exists in the graph artifact"),
                )
                .with_object_id(target.as_str())
                .with_help(DiagnosticCode::PatchTargetAlreadyExists.default_help()),
            );
            return;
        }
        let draft_validation = validate_draft(KnowledgeObjectDraft {
            id: target,
            kind,
            status,
            body,
            fields: &fields,
        });
        self.diagnostics.extend(draft_validation.diagnostics);
        for obligation in draft_validation.proof_obligations {
            self.add_proof_obligation(&obligation.object_id, &obligation.reason);
        }
        self.validate_placement(target, placement);

        self.diffs.push(PatchDiff {
            field: "object".to_string(),
            old: None,
            new: Some(serde_json::json!({
                "id": target.as_str(),
                "kind": kind,
                "status": status,
                "body": body,
                "fields": fields,
                "placement": {
                    "page_id": placement.page_id.clone(),
                    "after": placement.after.clone(),
                }
            })),
        });
    }

    fn validate_supersede(&mut self, target: &ObjectId, base_hash: &str, supersedes: Vec<String>) {
        let Some(object) = self.require_existing_target(target) else {
            return;
        };
        if !self.require_matching_base_hash(&object, base_hash) {
            return;
        }

        let mut seen = BTreeSet::new();
        let existing: BTreeSet<_> = object.relations.supersedes.iter().cloned().collect();
        for raw_target in supersedes {
            let relation_target = match ObjectId::new(raw_target.clone()) {
                Ok(id) => id,
                Err(_) => {
                    self.diagnostics.push(invalid_object_id_diagnostic(
                        raw_target,
                        "supersedes target",
                    ));
                    continue;
                }
            };
            if !seen.insert(relation_target.clone()) {
                self.diagnostics.push(validation_error(
                    target.as_str(),
                    format!("duplicate supersedes target `{relation_target}`"),
                ));
                continue;
            }
            if !self.graph.contains_object(&relation_target) {
                self.diagnostics
                    .push(missing_graph_object_diagnostic(relation_target.as_str()));
                continue;
            }
            if existing.contains(relation_target.as_str()) {
                self.diagnostics.push(validation_error(
                    target.as_str(),
                    format!("supersedes target `{relation_target}` already exists"),
                ));
                continue;
            }
            self.affected_relations.push(AffectedRelation {
                source: target.to_string(),
                relation: GraphRelationKind::Supersedes,
                target: relation_target.to_string(),
                action: "add".to_string(),
            });
            self.diffs.push(option_value_diff(
                "relations.supersedes".to_string(),
                None,
                Some(relation_target.to_string()),
            ));
        }
        self.add_verified_claim_obligation_if_needed(
            &object,
            "Verified claim supersession changes require evidence review before approval.",
        );
    }

    fn validate_revoke(&mut self, target: &ObjectId, base_hash: &str) {
        let Some(object) = self.require_existing_target(target) else {
            return;
        };
        if !self.require_matching_base_hash(&object, base_hash) {
            return;
        }
        self.diffs.push(option_value_diff(
            "status".to_string(),
            object.status.clone(),
            Some("revoked".to_string()),
        ));
        self.required_follow_up.push(format!(
            "Review source lifecycle fields for `{target}` before applying the revoke intent."
        ));
    }

    fn require_existing_target(&mut self, target: &ObjectId) -> Option<GraphKnowledgeObjectNode> {
        match self.graph.object(target) {
            Some(object) => Some(object.clone()),
            None => {
                self.diagnostics
                    .push(missing_graph_object_diagnostic(target.as_str()));
                None
            }
        }
    }

    fn require_matching_base_hash(
        &mut self,
        object: &GraphKnowledgeObjectNode,
        base_hash: &str,
    ) -> bool {
        if base_hash != object.content_hash {
            self.diagnostics.push(
                Diagnostic::error(
                    DiagnosticCode::PatchBaseHashMismatch,
                    format!(
                        "patch base_hash `{base_hash}` does not match current content_hash `{}` for `{}`",
                        object.content_hash, object.id
                    ),
                )
                .with_object_id(&object.id)
                .with_help(DiagnosticCode::PatchBaseHashMismatch.default_help()),
            );
            return false;
        }
        true
    }

    fn validate_placement(&mut self, target: &ObjectId, placement: &PlacementHint) {
        if !self.graph.page_exists(&placement.page_id) {
            self.diagnostics.push(
                Diagnostic::error(
                    DiagnosticCode::PatchPlacementInvalid,
                    format!("placement page_id `{}` does not exist", placement.page_id),
                )
                .with_object_id(target.as_str()),
            );
            return;
        }
        let Some(after) = placement.after.as_ref() else {
            return;
        };
        let after_id = match ObjectId::new(after.clone()) {
            Ok(id) => id,
            Err(_) => {
                self.diagnostics.push(invalid_object_id_diagnostic(
                    after,
                    "placement after target",
                ));
                return;
            }
        };
        match self.graph.object_page_id(&after_id) {
            Some(page_id) if page_id == placement.page_id => {}
            Some(page_id) => self.diagnostics.push(
                Diagnostic::error(
                    DiagnosticCode::PatchPlacementInvalid,
                    format!(
                        "placement after `{after}` is on page `{page_id}`, not `{}`",
                        placement.page_id
                    ),
                )
                .with_object_id(target.as_str()),
            ),
            None => self
                .diagnostics
                .push(missing_graph_object_diagnostic(after_id.as_str())),
        }
    }

    fn add_verified_claim_obligation_if_needed(
        &mut self,
        object: &GraphKnowledgeObjectNode,
        reason: &str,
    ) {
        if object.kind == "claim" && object.status.as_deref() == Some("verified") {
            self.add_proof_obligation(&object.id, reason);
        }
    }

    fn add_proof_obligation(&mut self, object_id: &str, reason: &str) {
        if self
            .proof_obligations
            .iter()
            .any(|obligation| obligation.object_id == object_id && obligation.reason == reason)
        {
            return;
        }
        self.proof_obligations.push(ProofObligation {
            object_id: object_id.to_string(),
            reason: reason.to_string(),
            required_evidence: vec![
                "owner".to_string(),
                "verified_at".to_string(),
                "source|test|reviewed_by".to_string(),
            ],
        });
        self.required_follow_up
            .push(format!("Resolve proof obligation for `{object_id}`."));
    }
}

fn value_diff(field: impl Into<String>, old: &str, new: &str) -> PatchDiff {
    option_value_diff(field.into(), Some(old.to_string()), Some(new.to_string()))
}

fn option_value_diff(field: String, old: Option<String>, new: Option<String>) -> PatchDiff {
    PatchDiff {
        field,
        old: old.map(serde_json::Value::String),
        new: new.map(serde_json::Value::String),
    }
}

fn is_valid_field_key(key: &str) -> bool {
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_lowercase()
        && chars.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

fn is_relation_field(key: &str) -> bool {
    GraphRelationKind::ALL
        .iter()
        .any(|relation| relation.as_str() == key)
}

fn validation_error(object_id: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(DiagnosticCode::PatchValidationFailed, message)
        .with_object_id(object_id)
        .with_help(DiagnosticCode::PatchValidationFailed.default_help())
}

fn invalid_object_id_diagnostic(id: impl Into<String>, label: &str) -> Diagnostic {
    let id = id.into();
    Diagnostic::error(
        DiagnosticCode::IdInvalid,
        format!("{label} Object ID `{id}` is invalid."),
    )
    .with_object_id(id)
    .with_help(OBJECT_ID_GRAMMAR_HELP)
}

fn missing_graph_object_diagnostic(id: impl Into<String>) -> Diagnostic {
    let id = id.into();
    Diagnostic::error(
        DiagnosticCode::GraphObjectNotFound,
        format!("Object ID `{id}` was not found in the graph artifact."),
    )
    .with_object_id(id)
    .with_help("Run `adoc build` if the source was changed after the graph artifact was generated.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::graph::{
        GraphArtifactDocument, GraphEdge, GraphKnowledgeObjectNode, GraphNode, GraphPageNode,
        GraphRelations, GraphSourceSpan,
    };

    fn graph(objects: Vec<GraphKnowledgeObjectNode>) -> GraphIndex {
        GraphIndex::from_document(GraphArtifactDocument {
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
        })
        .expect("graph indexes")
    }

    fn object(id: &str, status: &str) -> GraphKnowledgeObjectNode {
        GraphKnowledgeObjectNode {
            id: id.to_string(),
            kind: "claim".to_string(),
            content_hash: format!("sha256:{id}"),
            status: Some(status.to_string()),
            severity: None,
            trust: None,
            body: format!("{id} body."),
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
            evidence_quality: None,
        }
    }

    fn source_object(id: &str) -> GraphKnowledgeObjectNode {
        GraphKnowledgeObjectNode {
            id: id.to_string(),
            kind: "source".to_string(),
            content_hash: format!("sha256:{id}"),
            status: None,
            severity: None,
            trust: None,
            body: format!("{id} body."),
            page_id: "team.page".to_string(),
            source_span: GraphSourceSpan {
                path: "docs/team.adoc".to_string(),
                line: 2,
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
            evidence_quality: None,
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

    fn create_patch(
        kind: &str,
        status: Option<&str>,
        fields: BTreeMap<String, String>,
    ) -> PatchDocument {
        PatchDocument {
            target: "billing.new-credits".to_string(),
            intent: PatchIntent::CreateObject {
                kind: kind.to_string(),
                status: status.map(str::to_string),
                body: "Created body.".to_string(),
                fields,
                placement: PlacementHint {
                    page_id: "team.page".to_string(),
                    after: Some("billing.credits".to_string()),
                },
            },
            reason: "create object".to_string(),
            proposer: None,
        }
    }

    #[test]
    fn replace_body_requires_matching_base_hash_and_reports_diff() {
        let graph = graph(vec![object("billing.credits", "draft")]);
        let patch = patch(PatchIntent::ReplaceBody {
            base_hash: "sha256:billing.credits".to_string(),
            body: "New body.".to_string(),
        });

        let report = validate_patch(&graph, patch);

        assert!(report.valid);
        assert_eq!(report.diffs[0].field, "body");
        assert_eq!(report.diagnostics, Vec::new());
    }

    #[test]
    fn stale_base_hash_is_invalid() {
        let graph = graph(vec![object("billing.credits", "draft")]);
        let patch = patch(PatchIntent::ReplaceBody {
            base_hash: "sha256:stale".to_string(),
            body: "New body.".to_string(),
        });

        let report = validate_patch(&graph, patch);

        assert!(!report.valid);
        assert_eq!(
            report.diagnostics[0].code,
            DiagnosticCode::PatchBaseHashMismatch
        );
    }

    #[test]
    fn update_fields_rejects_relation_field_replacement() {
        let graph = graph(vec![object("billing.credits", "draft")]);
        let patch = patch(PatchIntent::UpdateFields {
            base_hash: "sha256:billing.credits".to_string(),
            fields: BTreeMap::from([("depends_on".to_string(), "billing.ledger".to_string())]),
        });

        let report = validate_patch(&graph, patch);

        assert!(!report.valid);
        assert_eq!(
            report.diagnostics[0].code,
            DiagnosticCode::PatchValidationFailed
        );
    }

    #[test]
    fn supersede_rejects_duplicate_relation_targets() {
        let graph = graph(vec![
            object("billing.credits", "draft"),
            object("billing.old-credits", "draft"),
        ]);
        let patch = patch(PatchIntent::Supersede {
            base_hash: "sha256:billing.credits".to_string(),
            supersedes: vec![
                "billing.old-credits".to_string(),
                "billing.old-credits".to_string(),
            ],
        });

        let report = validate_patch(&graph, patch);

        assert!(!report.valid);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("duplicate supersedes target"))
        );
    }

    #[test]
    fn create_object_requires_valid_placement() {
        let graph = graph(vec![object("billing.credits", "draft")]);
        let mut patch = PatchDocument {
            target: "billing.new-credits".to_string(),
            intent: PatchIntent::CreateObject {
                kind: "claim".to_string(),
                status: Some("draft".to_string()),
                body: "Created body.".to_string(),
                fields: BTreeMap::new(),
                placement: PlacementHint {
                    page_id: "team.missing".to_string(),
                    after: None,
                },
            },
            reason: "create new object".to_string(),
            proposer: None,
        };

        let report = validate_patch(&graph, patch.clone());
        assert!(!report.valid);
        assert_eq!(
            report.diagnostics[0].code,
            DiagnosticCode::PatchPlacementInvalid
        );

        patch.intent = PatchIntent::CreateObject {
            kind: "claim".to_string(),
            status: Some("draft".to_string()),
            body: "Created body.".to_string(),
            fields: BTreeMap::new(),
            placement: PlacementHint {
                page_id: "team.page".to_string(),
                after: Some("billing.credits".to_string()),
            },
        };
        let report = validate_patch(&graph, patch);
        assert!(report.valid);
    }

    #[test]
    fn revoke_is_status_diff_without_relation_change() {
        let graph = graph(vec![object("billing.credits", "draft")]);
        let patch = patch(PatchIntent::Revoke {
            base_hash: "sha256:billing.credits".to_string(),
        });

        let report = validate_patch(&graph, patch);

        assert!(report.valid);
        assert_eq!(report.diffs[0].field, "status");
        assert!(report.affected_relations.is_empty());
    }

    #[test]
    fn verified_claim_change_adds_proof_obligation() {
        let graph = graph(vec![object("billing.credits", "verified")]);
        let patch = patch(PatchIntent::ReplaceBody {
            base_hash: "sha256:billing.credits".to_string(),
            body: "New body.".to_string(),
        });

        let report = validate_patch(&graph, patch);

        assert!(report.valid);
        assert_eq!(report.proof_obligations.len(), 1);
        assert_eq!(report.proof_obligations[0].object_id, "billing.credits");
    }

    #[test]
    fn create_accepted_decision_without_decided_by_is_invalid() {
        let graph = graph(vec![object("billing.credits", "draft")]);
        let patch = create_patch("decision", Some("accepted"), BTreeMap::new());

        let report = validate_patch(&graph, patch);

        assert!(!report.valid);
        assert_eq!(
            report.diagnostics[0].code,
            DiagnosticCode::PatchValidationFailed
        );
        assert!(report.diagnostics[0].message.contains("fields.decided_by"));
    }

    #[test]
    fn create_accepted_decision_with_decided_by_is_valid() {
        let graph = graph(vec![object("billing.credits", "draft")]);
        let patch = create_patch(
            "decision",
            Some("accepted"),
            BTreeMap::from([("decided_by".to_string(), "architecture".to_string())]),
        );

        let report = validate_patch(&graph, patch);

        assert!(report.valid);
        assert!(report.diagnostics.is_empty());
    }

    #[test]
    fn create_verified_claim_missing_proof_data_is_valid_with_proof_obligation() {
        let graph = graph(vec![object("billing.credits", "draft")]);
        let patch = create_patch("claim", Some("verified"), BTreeMap::new());

        let report = validate_patch(&graph, patch);

        assert!(report.valid);
        assert_eq!(report.proof_obligations.len(), 1);
        assert_eq!(report.proof_obligations[0].object_id, "billing.new-credits");
        assert!(
            report.proof_obligations[0]
                .reason
                .contains("missing complete verification evidence")
        );
    }

    #[test]
    fn create_glossary_permits_status_field_but_rejects_changes_status() {
        let graph = graph(vec![object("billing.credits", "draft")]);
        let report = validate_patch(
            &graph,
            create_patch(
                "glossary",
                None,
                BTreeMap::from([("status".to_string(), "draft".to_string())]),
            ),
        );

        assert!(report.valid);

        let report = validate_patch(
            &graph,
            create_patch("glossary", Some("draft"), BTreeMap::new()),
        );

        assert!(!report.valid);
        assert_eq!(
            report.diagnostics[0].code,
            DiagnosticCode::PatchValidationFailed
        );
        assert!(report.diagnostics[0].message.contains("changes.status"));
    }

    // ── V5.8 TB5: evidence_ref resolution in update_fields ───────────────────

    #[test]
    fn update_field_evidence_ref_to_existing_source_is_valid_with_diff() {
        let graph = graph(vec![
            object("billing.credits", "draft"),
            source_object("billing.consume-use-case"),
        ]);
        let patch = patch(PatchIntent::UpdateFields {
            base_hash: "sha256:billing.credits".to_string(),
            fields: BTreeMap::from([(
                "evidence_ref".to_string(),
                "billing.consume-use-case".to_string(),
            )]),
        });

        let report = validate_patch(&graph, patch);

        assert!(
            report.valid,
            "expected valid; diagnostics: {:?}",
            report.diagnostics
        );
        assert!(
            report.diagnostics.is_empty(),
            "expected no diagnostics; got: {:?}",
            report.diagnostics
        );
        assert_eq!(report.diffs.len(), 1);
        assert_eq!(report.diffs[0].field, "fields.evidence_ref");
    }

    #[test]
    fn update_field_evidence_ref_to_missing_id_emits_target_not_found() {
        let graph = graph(vec![object("billing.credits", "draft")]);
        let patch = patch(PatchIntent::UpdateFields {
            base_hash: "sha256:billing.credits".to_string(),
            fields: BTreeMap::from([("evidence_ref".to_string(), "missing.thing".to_string())]),
        });

        let report = validate_patch(&graph, patch);

        assert!(!report.valid);
        assert_eq!(
            report
                .diagnostics
                .iter()
                .filter(|d| d.code == DiagnosticCode::SchemaEvidenceTargetNotFound)
                .count(),
            1,
            "expected exactly one SchemaEvidenceTargetNotFound; got: {:?}",
            report.diagnostics
        );
        // Diff is still recorded even on resolution failure (proposal as-is).
        assert_eq!(report.diffs.len(), 1);
        assert_eq!(report.diffs[0].field, "fields.evidence_ref");
    }

    #[test]
    fn update_field_evidence_ref_to_wrong_kind_emits_not_a_source() {
        // billing.credits is a claim; use it as a wrong-kind evidence_ref target.
        let graph = graph(vec![
            object("billing.credits", "draft"),
            object("billing.other-claim", "draft"),
        ]);
        // Target is billing.other-claim (kind=claim, not source).
        let patch_doc = patch(PatchIntent::UpdateFields {
            base_hash: "sha256:billing.credits".to_string(),
            fields: BTreeMap::from([(
                "evidence_ref".to_string(),
                "billing.other-claim".to_string(),
            )]),
        });

        let report = validate_patch(&graph, patch_doc);

        assert!(!report.valid);
        let not_a_source: Vec<_> = report
            .diagnostics
            .iter()
            .filter(|d| d.code == DiagnosticCode::SchemaEvidenceTargetNotASource)
            .collect();
        assert_eq!(
            not_a_source.len(),
            1,
            "expected exactly one SchemaEvidenceTargetNotASource; got: {:?}",
            report.diagnostics
        );
        assert!(
            not_a_source[0].message.contains("claim"),
            "message must name the actual kind; got: {}",
            not_a_source[0].message
        );
        // Diff is still recorded.
        assert_eq!(report.diffs.len(), 1);
        assert_eq!(report.diffs[0].field, "fields.evidence_ref");
    }

    #[test]
    fn update_field_non_evidence_ref_produces_no_new_diagnostics_regression() {
        // A non-evidence_ref update_fields patch must produce byte-identical
        // results to before TB5: no evidence diagnostics, diff recorded, valid.
        let graph = graph(vec![object("billing.credits", "draft")]);
        let patch = patch(PatchIntent::UpdateFields {
            base_hash: "sha256:billing.credits".to_string(),
            fields: BTreeMap::from([("owner".to_string(), "billing-team".to_string())]),
        });

        let report = validate_patch(&graph, patch);

        assert!(report.valid);
        assert!(
            report.diagnostics.is_empty(),
            "non-evidence field update must produce zero diagnostics; got: {:?}",
            report.diagnostics
        );
        assert_eq!(report.diffs.len(), 1);
        assert_eq!(report.diffs[0].field, "fields.owner");
        assert!(report.diffs[0].old.is_none());
        assert_eq!(
            report.diffs[0].new,
            Some(serde_json::Value::String("billing-team".to_string()))
        );
    }
}
