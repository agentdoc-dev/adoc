//! `api.verified_missing_schema_evidence` (V6.5.1, PRD §15.4).
//!
//! A verified `api` is verified by its schema source, not by human assertion:
//! it must carry at least one inline `source:` (SourceCode) evidence entry or
//! an `evidence_ref` resolving to a `source` object whose kind is
//! `api_schema` or `source_code`. This is a workspace rule (not a per-page
//! rule) because ref targets may live on other pages — the same reason
//! [`super::evidence_ref_resolves::EvidenceRefResolves`] is one.

use std::collections::HashMap;

use crate::domain::ast::{BlockAst, WorkspaceAst};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::identity::ObjectId;
use crate::domain::knowledge_object::KnowledgeObject;
use crate::domain::rules::WorkspaceRule;
use crate::domain::value_objects::evidence_kind::EvidenceKind;

pub(crate) struct ApiVerifiedEvidence;

impl WorkspaceRule for ApiVerifiedEvidence {
    fn check(&self, workspace: &WorkspaceAst, sink: &mut Vec<Diagnostic>) {
        // Map source object id -> its evidence kind, one workspace pass.
        let mut source_kinds: HashMap<&ObjectId, EvidenceKind> = HashMap::new();
        for page in &workspace.pages {
            for block in &page.blocks {
                let BlockAst::KnowledgeObject(ko) = block else {
                    continue;
                };
                if let KnowledgeObject::Source(source) = ko.as_ref() {
                    source_kinds.insert(source.id(), source.kind());
                }
            }
        }

        for page in &workspace.pages {
            for block in &page.blocks {
                let BlockAst::KnowledgeObject(ko) = block else {
                    continue;
                };
                let KnowledgeObject::Api(api) = ko.as_ref() else {
                    continue;
                };
                let Some(verification) = api.verification() else {
                    continue; // non-verified apis carry no evidence obligation
                };

                let has_inline_schema_evidence = verification
                    .evidence()
                    .iter()
                    .any(|entry| entry.kind() == Some(EvidenceKind::SourceCode));

                let has_schema_ref = api.evidence_refs().iter().any(|entry| {
                    entry
                        .target_id()
                        .and_then(|ref_id| source_kinds.get(ref_id))
                        .is_some_and(|kind| {
                            matches!(kind, EvidenceKind::ApiSchema | EvidenceKind::SourceCode)
                        })
                });

                if !has_inline_schema_evidence && !has_schema_ref {
                    sink.push(
                        Diagnostic::error(
                            DiagnosticCode::ApiVerifiedMissingSchemaEvidence,
                            format!(
                                "verified api `{}` requires schema evidence: an inline `source:` entry or an `evidence_ref` to an `api_schema`/`source_code` source",
                                api.id()
                            ),
                        )
                        .with_span(api.span().clone())
                        .with_object_id(api.id().as_str())
                        .with_help(DiagnosticCode::ApiVerifiedMissingSchemaEvidence.default_help()),
                    );
                }
            }
        }
    }
}
