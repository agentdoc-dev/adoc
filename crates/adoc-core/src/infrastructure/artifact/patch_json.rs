use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use serde::Deserialize;

use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::patch::{
    PatchDocument, PatchIntent, PatchOperation, PatchProposer, PlacementHint,
};
use crate::domain::ports::artifact_reader::ArtifactReader;

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct PatchJsonArtifact;

pub(crate) const SUPPORTED_PATCH_SCHEMA_VERSION: &str = "adoc.patch.v0";

pub(crate) fn read_patch_document(path: &Path) -> Result<PatchDocument, Vec<Diagnostic>> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) => return Err(vec![read_error_diagnostic(path, error)]),
    };
    let value = match serde_json::from_str::<serde_json::Value>(&contents) {
        Ok(value) => value,
        Err(error) => {
            return Err(vec![
                Diagnostic::error(
                    DiagnosticCode::IoArtifactMalformed,
                    format!(
                        "Patch artifact '{}' is malformed JSON: {error}",
                        path.display()
                    ),
                )
                .with_help("Fix the JSON syntax before running patch validation."),
            ]);
        }
    };
    read_patch_document_value(value, &format!("Patch artifact '{}'", path.display()))
}

pub(crate) fn read_patch_document_value(
    value: serde_json::Value,
    label: &str,
) -> Result<PatchDocument, Vec<Diagnostic>> {
    let document = match serde_json::from_value::<PatchDocumentDto>(value) {
        Ok(document) => document,
        Err(error) => {
            return Err(vec![
                Diagnostic::error(
                    DiagnosticCode::PatchInvalidDocument,
                    format!("{label} is not a valid adoc.patch.v0 document: {error}"),
                )
                .with_help(DiagnosticCode::PatchInvalidDocument.default_help()),
            ]);
        }
    };
    if document.schema_version != SUPPORTED_PATCH_SCHEMA_VERSION {
        return Err(vec![
            Diagnostic::error(
                DiagnosticCode::PatchInvalidDocument,
                format!(
                    "{label} uses unsupported schema_version '{}'.",
                    document.schema_version,
                ),
            )
            .with_help(format!(
                "Expected schema_version '{}'.",
                SUPPORTED_PATCH_SCHEMA_VERSION
            )),
        ]);
    }

    document.into_domain()
}

impl ArtifactReader for PatchJsonArtifact {
    type Output = PatchDocument;

    fn read(&self, path: &Path) -> Result<Self::Output, Vec<Diagnostic>> {
        read_patch_document(path)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PatchDocumentDto {
    schema_version: String,
    op: PatchOperation,
    target: String,
    #[serde(default)]
    base_hash: Option<String>,
    changes: PatchChangesDto,
    reason: String,
    #[serde(default)]
    proposer: Option<PatchProposerDto>,
}

impl PatchDocumentDto {
    fn into_domain(self) -> Result<PatchDocument, Vec<Diagnostic>> {
        let intent = into_intent(self.op, self.base_hash, self.changes)?;
        Ok(PatchDocument {
            target: self.target,
            intent,
            reason: self.reason,
            proposer: self.proposer.map(PatchProposerDto::into_domain),
        })
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct PatchChangesDto {
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    fields: Option<BTreeMap<String, String>>,
    #[serde(default)]
    placement: Option<PlacementHintDto>,
    #[serde(default)]
    supersedes: Option<Vec<String>>,
}

impl PatchChangesDto {
    fn populated_fields(&self) -> Vec<&'static str> {
        let mut fields = Vec::new();
        if self.body.is_some() {
            fields.push("body");
        }
        if self.kind.is_some() {
            fields.push("kind");
        }
        if self.status.is_some() {
            fields.push("status");
        }
        if self.fields.is_some() {
            fields.push("fields");
        }
        if self.placement.is_some() {
            fields.push("placement");
        }
        if self.supersedes.is_some() {
            fields.push("supersedes");
        }
        fields
    }

    fn unexpected_fields(&self, allowed: &[&str], op: PatchOperation) -> Vec<Diagnostic> {
        self.populated_fields()
            .into_iter()
            .filter(|field| !allowed.contains(field))
            .map(|field| {
                invalid_patch_document(format!("{} does not accept changes.{field}", op.as_str()))
            })
            .collect()
    }
}

fn into_intent(
    op: PatchOperation,
    base_hash: Option<String>,
    changes: PatchChangesDto,
) -> Result<PatchIntent, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    match op {
        PatchOperation::ReplaceBody => {
            diagnostics.extend(changes.unexpected_fields(&["body"], op));
            let Some(base_hash) = required_base_hash(op, base_hash, &mut diagnostics) else {
                return Err(diagnostics);
            };
            let Some(body) = required_change(changes.body, op, "body", &mut diagnostics) else {
                return Err(diagnostics);
            };
            if diagnostics.is_empty() {
                Ok(PatchIntent::ReplaceBody { base_hash, body })
            } else {
                Err(diagnostics)
            }
        }
        PatchOperation::UpdateFields => {
            diagnostics.extend(changes.unexpected_fields(&["fields"], op));
            let Some(base_hash) = required_base_hash(op, base_hash, &mut diagnostics) else {
                return Err(diagnostics);
            };
            let fields = changes.fields.unwrap_or_default();
            if fields.is_empty() {
                diagnostics.push(required_change_diagnostic(op, "fields"));
            }
            if diagnostics.is_empty() {
                Ok(PatchIntent::UpdateFields { base_hash, fields })
            } else {
                Err(diagnostics)
            }
        }
        PatchOperation::CreateObject => {
            diagnostics.extend(
                changes.unexpected_fields(&["kind", "status", "body", "fields", "placement"], op),
            );
            if base_hash.is_some() {
                diagnostics.push(invalid_patch_document("create_object must omit base_hash"));
            }
            let Some(kind) = required_change(changes.kind, op, "kind", &mut diagnostics) else {
                return Err(diagnostics);
            };
            let Some(body) = required_change(changes.body, op, "body", &mut diagnostics) else {
                return Err(diagnostics);
            };
            // V6.4 TB3: placement is optional on the wire. A missing
            // placement is a check-time WARNING and an apply-time ERROR
            // (`patch.create_missing_placement`), both emitted by the
            // domain validator and the apply orchestration — not here.
            let placement = changes.placement.map(PlacementHintDto::into_domain);
            if diagnostics.is_empty() {
                Ok(PatchIntent::CreateObject {
                    kind,
                    status: changes.status,
                    body,
                    fields: changes.fields.unwrap_or_default(),
                    placement,
                })
            } else {
                Err(diagnostics)
            }
        }
        PatchOperation::Supersede => {
            diagnostics.extend(changes.unexpected_fields(&["supersedes"], op));
            let Some(base_hash) = required_base_hash(op, base_hash, &mut diagnostics) else {
                return Err(diagnostics);
            };
            let supersedes = changes.supersedes.unwrap_or_default();
            if supersedes.is_empty() {
                diagnostics.push(required_change_diagnostic(op, "supersedes"));
            }
            if diagnostics.is_empty() {
                Ok(PatchIntent::Supersede {
                    base_hash,
                    supersedes,
                })
            } else {
                Err(diagnostics)
            }
        }
        PatchOperation::Revoke => {
            diagnostics.extend(changes.unexpected_fields(&[], op));
            let Some(base_hash) = required_base_hash(op, base_hash, &mut diagnostics) else {
                return Err(diagnostics);
            };
            if diagnostics.is_empty() {
                Ok(PatchIntent::Revoke { base_hash })
            } else {
                Err(diagnostics)
            }
        }
    }
}

fn required_base_hash(
    op: PatchOperation,
    base_hash: Option<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<String> {
    match base_hash {
        Some(base_hash) if !base_hash.trim().is_empty() => Some(base_hash),
        _ => {
            diagnostics.push(invalid_patch_document(format!(
                "{} requires base_hash",
                op.as_str()
            )));
            None
        }
    }
}

fn required_change(
    value: Option<String>,
    op: PatchOperation,
    field: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<String> {
    match value {
        Some(value) => Some(value),
        None => {
            diagnostics.push(required_change_diagnostic(op, field));
            None
        }
    }
}

fn required_change_diagnostic(op: PatchOperation, field: &str) -> Diagnostic {
    invalid_patch_document(format!("{} requires changes.{field}", op.as_str()))
}

fn invalid_patch_document(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(DiagnosticCode::PatchInvalidDocument, message)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlacementHintDto {
    page_id: String,
    #[serde(default)]
    after: Option<String>,
}

impl PlacementHintDto {
    fn into_domain(self) -> PlacementHint {
        PlacementHint {
            page_id: self.page_id,
            after: self.after,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PatchProposerDto {
    #[serde(rename = "type")]
    proposer_type: String,
    id: String,
}

impl PatchProposerDto {
    fn into_domain(self) -> PatchProposer {
        PatchProposer {
            proposer_type: self.proposer_type,
            id: self.id,
        }
    }
}

fn read_error_diagnostic(path: &Path, error: io::Error) -> Diagnostic {
    let code = if error.kind() == io::ErrorKind::NotFound {
        DiagnosticCode::IoArtifactMissing
    } else {
        DiagnosticCode::IoArtifactUnreadable
    };
    Diagnostic::error(
        code,
        format!(
            "Unable to read patch artifact '{}': {error}",
            path.display()
        ),
    )
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn reads_valid_patch_document() {
        let artifact = tempfile::Builder::new()
            .prefix("adoc-patch-")
            .suffix(".json")
            .tempfile()
            .expect("temp artifact can be created");
        fs::write(
            artifact.path(),
            serde_json::json!({
                "schema_version": "adoc.patch.v0",
                "op": "replace_body",
                "target": "billing.credits",
                "base_hash": "sha256:content",
                "changes": { "body": "Updated body." },
                "reason": "review update",
                "proposer": { "type": "agent", "id": "test-agent" }
            })
            .to_string(),
        )
        .expect("artifact can be written");

        let patch = read_patch_document(artifact.path()).expect("patch loads");

        assert_eq!(patch.target, "billing.credits");
        assert_eq!(patch.operation(), PatchOperation::ReplaceBody);
        match patch.intent {
            PatchIntent::ReplaceBody { base_hash, body } => {
                assert_eq!(base_hash, "sha256:content");
                assert_eq!(body, "Updated body.");
            }
            other => panic!("expected replace_body intent, got {other:?}"),
        }
    }

    #[test]
    fn rejects_unknown_fields_as_invalid_patch_document() {
        let artifact = tempfile::Builder::new()
            .prefix("adoc-patch-")
            .suffix(".json")
            .tempfile()
            .expect("temp artifact can be created");
        fs::write(
            artifact.path(),
            serde_json::json!({
                "schema_version": "adoc.patch.v0",
                "op": "replace_body",
                "target": "billing.credits",
                "base_hash": "sha256:content",
                "changes": { "body": "Updated body.", "unexpected": true },
                "reason": "review update"
            })
            .to_string(),
        )
        .expect("artifact can be written");

        let diagnostics = read_patch_document(artifact.path()).expect_err("patch must fail");

        assert_eq!(diagnostics[0].code, DiagnosticCode::PatchInvalidDocument);
    }

    #[test]
    fn malformed_json_is_io_artifact_malformed() {
        let artifact = tempfile::Builder::new()
            .prefix("adoc-patch-")
            .suffix(".json")
            .tempfile()
            .expect("temp artifact can be created");
        fs::write(artifact.path(), "{").expect("artifact can be written");

        let diagnostics = read_patch_document(artifact.path()).expect_err("patch must fail");

        assert_eq!(diagnostics[0].code, DiagnosticCode::IoArtifactMalformed);
    }

    #[test]
    fn replace_body_rejects_unrelated_changes_field_during_lowering() {
        let diagnostics = read_patch_value(serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "replace_body",
            "target": "billing.credits",
            "base_hash": "sha256:content",
            "changes": { "body": "Updated body.", "status": "verified" },
            "reason": "review update"
        }))
        .expect_err("patch must fail during DTO lowering");

        assert_invalid_document_message(diagnostics, "replace_body does not accept changes.status");
    }

    #[test]
    fn update_fields_rejects_unrelated_changes_field_during_lowering() {
        let diagnostics = read_patch_value(serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "update_fields",
            "target": "billing.credits",
            "base_hash": "sha256:content",
            "changes": { "fields": { "owner": "team-billing" }, "body": "Updated body." },
            "reason": "review update"
        }))
        .expect_err("patch must fail during DTO lowering");

        assert_invalid_document_message(diagnostics, "update_fields does not accept changes.body");
    }

    #[test]
    fn create_object_rejects_unrelated_changes_field_during_lowering() {
        let diagnostics = read_patch_value(serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "create_object",
            "target": "billing.new-credits",
            "changes": {
                "kind": "claim",
                "status": "draft",
                "body": "Created body.",
                "placement": { "page_id": "team.page" },
                "supersedes": ["billing.old-credits"]
            },
            "reason": "create object"
        }))
        .expect_err("patch must fail during DTO lowering");

        assert_invalid_document_message(
            diagnostics,
            "create_object does not accept changes.supersedes",
        );
    }

    #[test]
    fn create_object_without_placement_parses_with_none_placement() {
        // V6.4 TB3: placement is optional on the wire; the missing-placement
        // policy (check WARNING / apply ERROR) is enforced downstream.
        let document = read_patch_value(serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "create_object",
            "target": "billing.new-credits",
            "changes": {
                "kind": "claim",
                "status": "draft",
                "body": "Created body."
            },
            "reason": "create object"
        }))
        .expect("placement-less create parses");

        match document.intent {
            PatchIntent::CreateObject { placement, .. } => {
                assert!(placement.is_none(), "placement lowers to None");
            }
            other => panic!("expected CreateObject intent, got {other:?}"),
        }
    }

    #[test]
    fn supersede_rejects_unrelated_changes_field_during_lowering() {
        let diagnostics = read_patch_value(serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "supersede",
            "target": "billing.credits",
            "base_hash": "sha256:content",
            "changes": { "supersedes": ["billing.old-credits"], "body": "Updated body." },
            "reason": "supersede old object"
        }))
        .expect_err("patch must fail during DTO lowering");

        assert_invalid_document_message(diagnostics, "supersede does not accept changes.body");
    }

    #[test]
    fn revoke_rejects_unrelated_changes_field_during_lowering() {
        let diagnostics = read_patch_value(serde_json::json!({
            "schema_version": "adoc.patch.v0",
            "op": "revoke",
            "target": "billing.credits",
            "base_hash": "sha256:content",
            "changes": { "fields": {} },
            "reason": "revoke object"
        }))
        .expect_err("patch must fail during DTO lowering");

        assert_invalid_document_message(diagnostics, "revoke does not accept changes.fields");
    }

    fn read_patch_value(value: serde_json::Value) -> Result<PatchDocument, Vec<Diagnostic>> {
        let artifact = tempfile::Builder::new()
            .prefix("adoc-patch-")
            .suffix(".json")
            .tempfile()
            .expect("temp artifact can be created");
        fs::write(artifact.path(), value.to_string()).expect("artifact can be written");

        read_patch_document(artifact.path())
    }

    fn assert_invalid_document_message(diagnostics: Vec<Diagnostic>, message: &str) {
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic.code
                == DiagnosticCode::PatchInvalidDocument
                && diagnostic.message.contains(message)),
            "expected patch.invalid_document containing `{message}`, got {diagnostics:?}"
        );
    }
}
