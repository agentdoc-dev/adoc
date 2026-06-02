use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de, ser::SerializeStruct};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub severity: Severity,
    pub message: String,
    pub span: Option<SourceSpan>,
    pub object_id: Option<String>,
    pub help: Option<String>,
}

/// Semantic identifier for a diagnostic.
///
/// Per ADR-0005, this is part of the public surface as of v0.x — promoted from
/// `pub(crate)` so external consumers (the CLI today, future LSP/web hosts
/// tomorrow) can pattern-match on it instead of comparing strings. The wire
/// format remains the dotted code string (`parse.raw_html`,
/// `io.unreadable_file`, `io.unreadable_directory`, etc.); the manual
/// `Serialize` impl below preserves byte-identical JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticCode {
    ParseRawHtml,
    ParseUnsafeLink,
    ParseUnclosedFence,
    ParseMalformedPageAnnotation,
    ParseNestedTypedBlock,
    ParseMalformedField,
    ParseMalformedOpenFence,
    SchemaUnknownKind,
    SchemaMissingField,
    SchemaDuplicateField,
    SchemaInvalidStatus,
    SchemaConstraintMissingSeverity,
    SchemaConstraintInvalidSeverity,
    SchemaProcedureMissingStatus,
    SchemaProcedureMissingBody,
    SchemaProcedureBodyMustStartWithOrderedList,
    ProcedureVerifiedMissingEvidence,
    SchemaExampleMissingLang,
    SchemaExampleInvalidLang,
    SchemaExampleInvalidSandbox,
    SchemaExampleVerifiedRequiresChecks,
    SchemaExampleVerifiedRequiresSandbox,
    ClaimVerifiedMissingEvidence,
    ClaimStatusCasing,
    LifecycleExpired,
    LifecycleInvalidExpiresAt,
    IdDuplicate,
    IdInvalid,
    RefBroken,
    IoUnreadableFile,
    IoUnreadableDirectory,
    IoUnsupportedSourceExtension,
    IoArtifactMissing,
    IoArtifactUnreadable,
    IoArtifactMalformed,
    SchemaUnsupportedVersion,
    IdDuplicateInArtifact,
    RetrievalObjectNotFound,
    SearchInvalidFilter,
    EmbedModelLoadFailed,
    EmbedComputeFailed,
    EmbedUnexpectedDimension,
    BuildEmbeddingsCached,
    BuildEmbeddingsCacheIgnored,
    BuildEmbeddingsSkipped,
    SearchArtifactMissing,
    SearchModelMismatch,
    SearchHashDrift,
    SearchDeterministicQuality,
    GraphObjectNotFound,
    PatchInvalidDocument,
    PatchValidationFailed,
    PatchBaseHashMismatch,
    PatchTargetAlreadyExists,
    PatchPlacementInvalid,
    SchemaImpactsInvalidPath,
    SchemaImpactsEmpty,
    SchemaPolicyMissingStatus,
    SchemaPolicyMissingOwner,
    SchemaPolicyMissingApprovedBy,
    SchemaPolicyMissingEffectiveAt,
    SchemaPolicyInvalidEffectiveAt,
    SchemaPolicyInvalidReviewInterval,
    SchemaPolicyMissingBody,
    SchemaPolicyFutureEffectiveAt,
    SchemaAgentInstructionMissingScope,
    SchemaAgentInstructionMissingTrust,
    SchemaAgentInstructionInvalidTrust,
    SchemaAgentInstructionMissingAllowedActions,
    SchemaAgentInstructionMissingForbiddenActions,
    SchemaAgentInstructionActionsNotDisjoint,
    CompatRawHtmlQuarantined,
    CompatUnsafeLinkDropped,
    CompatUnsafeImageSrcDropped,
    CompatUnknownExtension,
    ParseMalformedMarkdown,
    RetrievalNoKnowledgeObjectsConsiderMigration,
    SchemaContradictionMissingSeverity,
    SchemaContradictionInvalidSeverity,
    SchemaContradictionMissingStatus,
    SchemaContradictionInvalidStatus,
    SchemaContradictionMissingClaims,
    SchemaContradictionClaimsTooFew,
    SchemaContradictionClaimNotFound,
    SchemaSourceMissingKind,
    SchemaSourceInvalidKind,
    SchemaSourceMissingPathOrUrl,
    SchemaSourceConflictingPathAndUrl,
    SchemaSourceInvalidPath,
    SchemaSourceInvalidUrl,
    SchemaSourceKindTargetMismatch,
}

impl DiagnosticCode {
    fn all() -> &'static [Self] {
        &[
            DiagnosticCode::ParseRawHtml,
            DiagnosticCode::ParseUnsafeLink,
            DiagnosticCode::ParseUnclosedFence,
            DiagnosticCode::ParseMalformedPageAnnotation,
            DiagnosticCode::ParseNestedTypedBlock,
            DiagnosticCode::ParseMalformedField,
            DiagnosticCode::ParseMalformedOpenFence,
            DiagnosticCode::SchemaUnknownKind,
            DiagnosticCode::SchemaMissingField,
            DiagnosticCode::SchemaDuplicateField,
            DiagnosticCode::SchemaInvalidStatus,
            DiagnosticCode::SchemaConstraintMissingSeverity,
            DiagnosticCode::SchemaConstraintInvalidSeverity,
            DiagnosticCode::SchemaProcedureMissingStatus,
            DiagnosticCode::SchemaProcedureMissingBody,
            DiagnosticCode::SchemaProcedureBodyMustStartWithOrderedList,
            DiagnosticCode::ProcedureVerifiedMissingEvidence,
            DiagnosticCode::SchemaExampleMissingLang,
            DiagnosticCode::SchemaExampleInvalidLang,
            DiagnosticCode::SchemaExampleInvalidSandbox,
            DiagnosticCode::SchemaExampleVerifiedRequiresChecks,
            DiagnosticCode::SchemaExampleVerifiedRequiresSandbox,
            DiagnosticCode::ClaimVerifiedMissingEvidence,
            DiagnosticCode::ClaimStatusCasing,
            DiagnosticCode::LifecycleExpired,
            DiagnosticCode::LifecycleInvalidExpiresAt,
            DiagnosticCode::IdDuplicate,
            DiagnosticCode::IdInvalid,
            DiagnosticCode::RefBroken,
            DiagnosticCode::IoUnreadableFile,
            DiagnosticCode::IoUnreadableDirectory,
            DiagnosticCode::IoUnsupportedSourceExtension,
            DiagnosticCode::IoArtifactMissing,
            DiagnosticCode::IoArtifactUnreadable,
            DiagnosticCode::IoArtifactMalformed,
            DiagnosticCode::SchemaUnsupportedVersion,
            DiagnosticCode::IdDuplicateInArtifact,
            DiagnosticCode::RetrievalObjectNotFound,
            DiagnosticCode::SearchInvalidFilter,
            DiagnosticCode::EmbedModelLoadFailed,
            DiagnosticCode::EmbedComputeFailed,
            DiagnosticCode::EmbedUnexpectedDimension,
            DiagnosticCode::BuildEmbeddingsCached,
            DiagnosticCode::BuildEmbeddingsCacheIgnored,
            DiagnosticCode::BuildEmbeddingsSkipped,
            DiagnosticCode::SearchArtifactMissing,
            DiagnosticCode::SearchModelMismatch,
            DiagnosticCode::SearchHashDrift,
            DiagnosticCode::SearchDeterministicQuality,
            DiagnosticCode::GraphObjectNotFound,
            DiagnosticCode::PatchInvalidDocument,
            DiagnosticCode::PatchValidationFailed,
            DiagnosticCode::PatchBaseHashMismatch,
            DiagnosticCode::PatchTargetAlreadyExists,
            DiagnosticCode::PatchPlacementInvalid,
            DiagnosticCode::SchemaImpactsInvalidPath,
            DiagnosticCode::SchemaImpactsEmpty,
            DiagnosticCode::SchemaPolicyMissingStatus,
            DiagnosticCode::SchemaPolicyMissingOwner,
            DiagnosticCode::SchemaPolicyMissingApprovedBy,
            DiagnosticCode::SchemaPolicyMissingEffectiveAt,
            DiagnosticCode::SchemaPolicyInvalidEffectiveAt,
            DiagnosticCode::SchemaPolicyInvalidReviewInterval,
            DiagnosticCode::SchemaPolicyMissingBody,
            DiagnosticCode::SchemaPolicyFutureEffectiveAt,
            DiagnosticCode::SchemaAgentInstructionMissingScope,
            DiagnosticCode::SchemaAgentInstructionMissingTrust,
            DiagnosticCode::SchemaAgentInstructionInvalidTrust,
            DiagnosticCode::SchemaAgentInstructionMissingAllowedActions,
            DiagnosticCode::SchemaAgentInstructionMissingForbiddenActions,
            DiagnosticCode::SchemaAgentInstructionActionsNotDisjoint,
            DiagnosticCode::CompatRawHtmlQuarantined,
            DiagnosticCode::CompatUnsafeLinkDropped,
            DiagnosticCode::CompatUnsafeImageSrcDropped,
            DiagnosticCode::CompatUnknownExtension,
            DiagnosticCode::ParseMalformedMarkdown,
            DiagnosticCode::RetrievalNoKnowledgeObjectsConsiderMigration,
            DiagnosticCode::SchemaContradictionMissingSeverity,
            DiagnosticCode::SchemaContradictionInvalidSeverity,
            DiagnosticCode::SchemaContradictionMissingStatus,
            DiagnosticCode::SchemaContradictionInvalidStatus,
            DiagnosticCode::SchemaContradictionMissingClaims,
            DiagnosticCode::SchemaContradictionClaimsTooFew,
            DiagnosticCode::SchemaContradictionClaimNotFound,
            DiagnosticCode::SchemaSourceMissingKind,
            DiagnosticCode::SchemaSourceInvalidKind,
            DiagnosticCode::SchemaSourceMissingPathOrUrl,
            DiagnosticCode::SchemaSourceConflictingPathAndUrl,
            DiagnosticCode::SchemaSourceInvalidPath,
            DiagnosticCode::SchemaSourceInvalidUrl,
            DiagnosticCode::SchemaSourceKindTargetMismatch,
        ]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            DiagnosticCode::ParseRawHtml => "parse.raw_html",
            DiagnosticCode::ParseUnsafeLink => "parse.unsafe_link",
            DiagnosticCode::ParseUnclosedFence => "parse.unclosed_fence",
            DiagnosticCode::ParseMalformedPageAnnotation => "parse.malformed_page_annotation",
            DiagnosticCode::ParseNestedTypedBlock => "parse.nested_typed_block",
            DiagnosticCode::ParseMalformedField => "parse.malformed_field",
            DiagnosticCode::ParseMalformedOpenFence => "parse.malformed_open_fence",
            DiagnosticCode::SchemaUnknownKind => "schema.unknown_kind",
            DiagnosticCode::SchemaMissingField => "schema.missing_field",
            DiagnosticCode::SchemaDuplicateField => "schema.duplicate_field",
            DiagnosticCode::SchemaInvalidStatus => "schema.invalid_status",
            DiagnosticCode::SchemaConstraintMissingSeverity => "schema.constraint_missing_severity",
            DiagnosticCode::SchemaConstraintInvalidSeverity => "schema.constraint_invalid_severity",
            DiagnosticCode::SchemaProcedureMissingStatus => "schema.procedure_missing_status",
            DiagnosticCode::SchemaProcedureMissingBody => "schema.procedure_missing_body",
            DiagnosticCode::SchemaProcedureBodyMustStartWithOrderedList => {
                "schema.procedure_body_must_start_with_ordered_list"
            }
            DiagnosticCode::ProcedureVerifiedMissingEvidence => {
                "procedure.verified_missing_evidence"
            }
            DiagnosticCode::SchemaExampleMissingLang => "schema.example_missing_lang",
            DiagnosticCode::SchemaExampleInvalidLang => "schema.example_invalid_lang",
            DiagnosticCode::SchemaExampleInvalidSandbox => "schema.example_invalid_sandbox",
            DiagnosticCode::SchemaExampleVerifiedRequiresChecks => {
                "schema.example_verified_requires_checks"
            }
            DiagnosticCode::SchemaExampleVerifiedRequiresSandbox => {
                "schema.example_verified_requires_sandbox"
            }
            DiagnosticCode::ClaimVerifiedMissingEvidence => "claim.verified_missing_evidence",
            DiagnosticCode::ClaimStatusCasing => "claim.status_casing",
            DiagnosticCode::LifecycleExpired => "lifecycle.expired",
            DiagnosticCode::LifecycleInvalidExpiresAt => "lifecycle.invalid_expires_at",
            DiagnosticCode::IdDuplicate => "id.duplicate",
            DiagnosticCode::IdInvalid => "id.invalid",
            DiagnosticCode::RefBroken => "ref.broken",
            DiagnosticCode::IoUnreadableFile => "io.unreadable_file",
            DiagnosticCode::IoUnreadableDirectory => "io.unreadable_directory",
            DiagnosticCode::IoUnsupportedSourceExtension => "io.unsupported_source_extension",
            DiagnosticCode::IoArtifactMissing => "io.artifact_missing",
            DiagnosticCode::IoArtifactUnreadable => "io.artifact_unreadable",
            DiagnosticCode::IoArtifactMalformed => "io.artifact_malformed",
            DiagnosticCode::SchemaUnsupportedVersion => "schema.unsupported_version",
            DiagnosticCode::IdDuplicateInArtifact => "id.duplicate_in_artifact",
            DiagnosticCode::RetrievalObjectNotFound => "retrieval.object_not_found",
            DiagnosticCode::SearchInvalidFilter => "search.invalid_filter",
            DiagnosticCode::EmbedModelLoadFailed => "embed.model_load_failed",
            DiagnosticCode::EmbedComputeFailed => "embed.compute_failed",
            DiagnosticCode::EmbedUnexpectedDimension => "embed.unexpected_dim",
            DiagnosticCode::BuildEmbeddingsCached => "build.embeddings_cached",
            DiagnosticCode::BuildEmbeddingsCacheIgnored => "build.embeddings_cache_ignored",
            DiagnosticCode::BuildEmbeddingsSkipped => "build.embeddings_skipped",
            DiagnosticCode::SearchArtifactMissing => "search.artifact_missing",
            DiagnosticCode::SearchModelMismatch => "search.model_mismatch",
            DiagnosticCode::SearchHashDrift => "search.hash_drift",
            DiagnosticCode::SearchDeterministicQuality => "search.deterministic_quality",
            DiagnosticCode::GraphObjectNotFound => "graph.object_not_found",
            DiagnosticCode::PatchInvalidDocument => "patch.invalid_document",
            DiagnosticCode::PatchValidationFailed => "patch.validation_failed",
            DiagnosticCode::PatchBaseHashMismatch => "patch.base_hash_mismatch",
            DiagnosticCode::PatchTargetAlreadyExists => "patch.target_already_exists",
            DiagnosticCode::PatchPlacementInvalid => "patch.placement_invalid",
            DiagnosticCode::SchemaImpactsInvalidPath => "schema.impacts_invalid_path",
            DiagnosticCode::SchemaImpactsEmpty => "schema.impacts_empty",
            DiagnosticCode::SchemaPolicyMissingStatus => "schema.policy_missing_status",
            DiagnosticCode::SchemaPolicyMissingOwner => "schema.policy_missing_owner",
            DiagnosticCode::SchemaPolicyMissingApprovedBy => "schema.policy_missing_approved_by",
            DiagnosticCode::SchemaPolicyMissingEffectiveAt => "schema.policy_missing_effective_at",
            DiagnosticCode::SchemaPolicyInvalidEffectiveAt => "schema.policy_invalid_effective_at",
            DiagnosticCode::SchemaPolicyInvalidReviewInterval => {
                "schema.policy_invalid_review_interval"
            }
            DiagnosticCode::SchemaPolicyMissingBody => "schema.policy_missing_body",
            DiagnosticCode::SchemaPolicyFutureEffectiveAt => "schema.policy_future_effective_at",
            DiagnosticCode::SchemaAgentInstructionMissingScope => {
                "schema.agent_instruction_missing_scope"
            }
            DiagnosticCode::SchemaAgentInstructionMissingTrust => {
                "schema.agent_instruction_missing_trust"
            }
            DiagnosticCode::SchemaAgentInstructionInvalidTrust => {
                "schema.agent_instruction_invalid_trust"
            }
            DiagnosticCode::SchemaAgentInstructionMissingAllowedActions => {
                "schema.agent_instruction_missing_allowed_actions"
            }
            DiagnosticCode::SchemaAgentInstructionMissingForbiddenActions => {
                "schema.agent_instruction_missing_forbidden_actions"
            }
            DiagnosticCode::SchemaAgentInstructionActionsNotDisjoint => {
                "schema.agent_instruction_actions_not_disjoint"
            }
            DiagnosticCode::CompatRawHtmlQuarantined => "compat.raw_html_quarantined",
            DiagnosticCode::CompatUnsafeLinkDropped => "compat.unsafe_link_dropped",
            DiagnosticCode::CompatUnsafeImageSrcDropped => "compat.unsafe_image_src_dropped",
            DiagnosticCode::CompatUnknownExtension => "compat.unknown_extension",
            DiagnosticCode::ParseMalformedMarkdown => "parse.malformed_markdown",
            DiagnosticCode::RetrievalNoKnowledgeObjectsConsiderMigration => {
                "retrieval.no_knowledge_objects_consider_migration"
            }
            DiagnosticCode::SchemaContradictionMissingSeverity => {
                "schema.contradiction_missing_severity"
            }
            DiagnosticCode::SchemaContradictionInvalidSeverity => {
                "schema.contradiction_invalid_severity"
            }
            DiagnosticCode::SchemaContradictionMissingStatus => {
                "schema.contradiction_missing_status"
            }
            DiagnosticCode::SchemaContradictionInvalidStatus => {
                "schema.contradiction_invalid_status"
            }
            DiagnosticCode::SchemaContradictionMissingClaims => {
                "schema.contradiction_missing_claims"
            }
            DiagnosticCode::SchemaContradictionClaimsTooFew => {
                "schema.contradiction_claims_too_few"
            }
            DiagnosticCode::SchemaContradictionClaimNotFound => {
                "schema.contradiction_claim_not_found"
            }
            DiagnosticCode::SchemaSourceMissingKind => "schema.source_missing_kind",
            DiagnosticCode::SchemaSourceInvalidKind => "schema.source_invalid_kind",
            DiagnosticCode::SchemaSourceMissingPathOrUrl => "schema.source_missing_path_or_url",
            DiagnosticCode::SchemaSourceConflictingPathAndUrl => {
                "schema.source_conflicting_path_and_url"
            }
            DiagnosticCode::SchemaSourceInvalidPath => "schema.source_invalid_path",
            DiagnosticCode::SchemaSourceInvalidUrl => "schema.source_invalid_url",
            DiagnosticCode::SchemaSourceKindTargetMismatch => "schema.source_kind_target_mismatch",
        }
    }

    pub fn default_help(self) -> &'static str {
        match self {
            DiagnosticCode::ParseRawHtml => {
                "Remove raw HTML or replace it with supported Markdown/ADoc syntax."
            }
            DiagnosticCode::ParseUnsafeLink => {
                "Use a safe link scheme such as https, http, mailto, or a relative path."
            }
            DiagnosticCode::ParseUnclosedFence => {
                "Close the typed block with a matching fence before the end of the file."
            }
            DiagnosticCode::ParseMalformedPageAnnotation => {
                "Use a page annotation in the form `@doc(object.id)` with a valid Object ID."
            }
            DiagnosticCode::ParseNestedTypedBlock => {
                "Move nested typed blocks out of the current block body or field value."
            }
            DiagnosticCode::ParseMalformedField => {
                "Write typed block fields as `key: value` lines before the block body."
            }
            DiagnosticCode::ParseMalformedOpenFence => {
                "Open typed blocks with `::kind object.id`, using a supported kind and valid Object ID."
            }
            DiagnosticCode::SchemaUnknownKind => {
                "Use a supported object kind or update the schema before compiling."
            }
            DiagnosticCode::SchemaMissingField => "Add the required field with a non-empty value.",
            DiagnosticCode::SchemaDuplicateField => {
                "Keep only one value for each field inside the object."
            }
            DiagnosticCode::SchemaInvalidStatus => {
                "Use one of the allowed status values for this object kind."
            }
            DiagnosticCode::SchemaConstraintMissingSeverity => {
                "Add a `severity` field to the constraint: one of low, medium, high, critical."
            }
            DiagnosticCode::SchemaConstraintInvalidSeverity => {
                "Use a valid constraint severity: one of low, medium, high, critical."
            }
            DiagnosticCode::SchemaProcedureMissingStatus => {
                "Add a `status` field to the procedure: one of draft, verified, deprecated."
            }
            DiagnosticCode::SchemaProcedureMissingBody => {
                "Add a non-empty body to the procedure describing its ordered steps."
            }
            DiagnosticCode::SchemaProcedureBodyMustStartWithOrderedList => {
                "Begin the procedure body with an ordered list; write the steps as `1. ...`, `2. ...`."
            }
            DiagnosticCode::ProcedureVerifiedMissingEvidence => {
                "Add at least one evidence field (`source`, `human_review`, or `reviewed_by`) before marking the procedure as verified."
            }
            DiagnosticCode::SchemaExampleMissingLang => {
                "An example requires either `lang` or `format`."
            }
            DiagnosticCode::SchemaExampleInvalidLang => {
                "Valid `lang` is a lowercase token matching [a-z][a-z0-9_+-]* (e.g. `ts`, `python`, `c++`)."
            }
            DiagnosticCode::SchemaExampleInvalidSandbox => {
                "Valid `sandbox` is a lowercase token matching [a-z][a-z0-9_+:-]* (e.g. `node-test`, `docker:node-test`)."
            }
            DiagnosticCode::SchemaExampleVerifiedRequiresChecks => {
                "A verified example requires both `checks` and `sandbox`."
            }
            DiagnosticCode::SchemaExampleVerifiedRequiresSandbox => {
                "A verified example requires both `checks` and `sandbox`."
            }
            DiagnosticCode::ClaimVerifiedMissingEvidence => {
                "Add evidence entries before marking the claim as verified."
            }
            DiagnosticCode::ClaimStatusCasing => "Use the canonical lowercase claim status value.",
            DiagnosticCode::LifecycleExpired => {
                "Update `expires_at` or remove it if this Knowledge Object is still valid."
            }
            DiagnosticCode::LifecycleInvalidExpiresAt => {
                "Use `YYYY-MM-DD` for `expires_at`, or remove the field."
            }
            DiagnosticCode::IdDuplicate => {
                "Give each object a unique ID across the compiled workspace."
            }
            DiagnosticCode::IdInvalid => {
                "Use a valid Object ID with lowercase segments separated by dots."
            }
            DiagnosticCode::RefBroken => {
                "Update the reference to an existing object ID or remove the reference."
            }
            DiagnosticCode::IoUnreadableFile => {
                "Check that the source path exists and can be read by the current user."
            }
            DiagnosticCode::IoUnreadableDirectory => {
                "Check that the source directory exists and can be read by the current user."
            }
            DiagnosticCode::IoUnsupportedSourceExtension => {
                "Use supported source files with the `.adoc` or `.md` extension."
            }
            DiagnosticCode::IoArtifactMissing => {
                "Build docs.graph.json before loading the retrieval artifact."
            }
            DiagnosticCode::IoArtifactUnreadable => {
                "Check that docs.graph.json exists and can be read by the current user."
            }
            DiagnosticCode::IoArtifactMalformed => {
                "Rebuild docs.graph.json from valid source documents."
            }
            DiagnosticCode::SchemaUnsupportedVersion => {
                "Regenerate the artifact with a schema version supported by this binary."
            }
            DiagnosticCode::IdDuplicateInArtifact => {
                "Rebuild the artifact after removing duplicate object IDs from the source."
            }
            DiagnosticCode::RetrievalObjectNotFound => {
                "Use an object ID present in the loaded retrieval artifact."
            }
            DiagnosticCode::SearchInvalidFilter => {
                "Change or remove the filter so it matches at least one object field in the artifact."
            }
            DiagnosticCode::EmbedModelLoadFailed => {
                "Check network access for the first model download, verify the local model cache is readable, ensure the binary was built with the `embeddings` feature, or rerun `adoc build --no-embeddings`."
            }
            DiagnosticCode::EmbedComputeFailed => {
                "Retry `adoc build`; if the error repeats, rebuild with `--no-embeddings` while investigating the embedding provider."
            }
            DiagnosticCode::EmbedUnexpectedDimension => {
                "Use an embedding provider that returns exactly one vector per input and the configured vector dimension."
            }
            DiagnosticCode::BuildEmbeddingsCached => {
                "No action is required; this reports search artifact embedding cache reuse."
            }
            DiagnosticCode::BuildEmbeddingsCacheIgnored => {
                "No action is required; the search cache will be recomputed for the current embedding model."
            }
            DiagnosticCode::BuildEmbeddingsSkipped => {
                "Run `adoc build` without `--no-embeddings` to emit docs.search.json."
            }
            DiagnosticCode::SearchArtifactMissing => {
                "Run `adoc build` to generate dist/docs.search.json for hybrid or semantic search."
            }
            DiagnosticCode::SearchModelMismatch => {
                "Rebuild dist/docs.search.json with the active embedding provider, or switch providers to match the artifact's model header."
            }
            DiagnosticCode::SearchHashDrift => {
                "Re-run `adoc build` to regenerate dist/docs.search.json from the current graph artifact."
            }
            DiagnosticCode::SearchDeterministicQuality => {
                "Use a semantic embedding provider for quality-sensitive retrieval; deterministic embeddings are repeatable but non-semantic."
            }
            DiagnosticCode::GraphObjectNotFound => {
                "Use an object ID present in the loaded graph artifact, or rebuild docs.graph.json."
            }
            DiagnosticCode::PatchInvalidDocument => {
                "Use the adoc.patch.v0 schema with exactly one supported operation and its required fields."
            }
            DiagnosticCode::PatchValidationFailed => {
                "Adjust the patch intent so it satisfies AgentDoc patch validation rules."
            }
            DiagnosticCode::PatchBaseHashMismatch => {
                "Rebuild docs.graph.json or regenerate the patch against the current target content_hash."
            }
            DiagnosticCode::PatchTargetAlreadyExists => {
                "Use create_object only for a new Object ID, or choose an update operation for an existing object."
            }
            DiagnosticCode::PatchPlacementInvalid => {
                "Use an existing page_id and, when after is supplied, an object already on that page."
            }
            DiagnosticCode::SchemaImpactsInvalidPath => {
                "Use a repo-relative path under the project root; remove leading `/`, `..` segments, and blank entries."
            }
            DiagnosticCode::SchemaImpactsEmpty => {
                "Remove the `impacts:` field entirely instead of leaving it empty; impacts must list at least one path."
            }
            DiagnosticCode::SchemaPolicyMissingStatus => {
                "Add a `status` field to the policy: one of proposed, active, archived, revoked."
            }
            DiagnosticCode::SchemaPolicyMissingOwner => {
                "Add a non-empty `owner` field to the policy."
            }
            DiagnosticCode::SchemaPolicyMissingApprovedBy => {
                "Add an `approved_by` field listing at least one approver (scalar or `[a, b]` list)."
            }
            DiagnosticCode::SchemaPolicyMissingEffectiveAt => {
                "Add an `effective_at` field in `YYYY-MM-DD` format."
            }
            DiagnosticCode::SchemaPolicyInvalidEffectiveAt => {
                "Use a valid `YYYY-MM-DD` date for `effective_at`."
            }
            DiagnosticCode::SchemaPolicyInvalidReviewInterval => {
                "Use a valid review interval in `[0-9]+d` form for `review_interval` (e.g. `90d`)."
            }
            DiagnosticCode::SchemaPolicyMissingBody => {
                "Add a non-empty body to the policy describing its rules."
            }
            DiagnosticCode::SchemaPolicyFutureEffectiveAt => {
                "An `active` policy must have an `effective_at` date on or before today."
            }
            DiagnosticCode::SchemaAgentInstructionMissingScope => {
                "Add a `scope` field to the agent_instruction with a non-empty glob pattern (e.g. `docs/auth/*`)."
            }
            DiagnosticCode::SchemaAgentInstructionMissingTrust => {
                "Add a `trust` field to the agent_instruction: one of informal, team, authoritative, regulated, system."
            }
            DiagnosticCode::SchemaAgentInstructionInvalidTrust => {
                "Use a valid trust level: one of informal, team, authoritative, regulated, system."
            }
            DiagnosticCode::SchemaAgentInstructionMissingAllowedActions => {
                "Add an `allowed_actions` field listing at least one action (scalar or `[a, b]` list)."
            }
            DiagnosticCode::SchemaAgentInstructionMissingForbiddenActions => {
                "Add a `forbidden_actions` field listing at least one action (scalar or `[a, b]` list)."
            }
            DiagnosticCode::SchemaAgentInstructionActionsNotDisjoint => {
                "Remove the overlapping actions so `allowed_actions` and `forbidden_actions` are disjoint."
            }
            DiagnosticCode::CompatRawHtmlQuarantined => {
                "Replace raw HTML with Markdown syntax, or migrate the page to .adoc for strict validation."
            }
            DiagnosticCode::CompatUnsafeLinkDropped => {
                "Use a safe link scheme such as https, http, mailto, or a relative path; the unsafe href was dropped from the rendered HTML."
            }
            DiagnosticCode::CompatUnsafeImageSrcDropped => {
                "Use a safe image scheme such as https, http, or a relative path; the unsafe src was dropped from the rendered HTML."
            }
            DiagnosticCode::CompatUnknownExtension => {
                "Remove the unsupported Markdown construct or migrate the page to .adoc; the source text was rendered as an escaped code block."
            }
            DiagnosticCode::ParseMalformedMarkdown => {
                "Check that Markdown blocks (tables, lists, fenced code) are well-formed; the parser tolerated the imbalance and rendered best-effort output."
            }
            DiagnosticCode::RetrievalNoKnowledgeObjectsConsiderMigration => {
                "Migrate .md files to .adoc with typed Knowledge Objects, or wait for `adoc migrate` (V4.5+); Markdown source contributes prose blocks but no citable Knowledge Objects."
            }
            DiagnosticCode::SchemaContradictionMissingSeverity => {
                "Add a `severity` field to the contradiction: one of low, medium, high, critical."
            }
            DiagnosticCode::SchemaContradictionInvalidSeverity => {
                "Use a valid contradiction severity: one of low, medium, high, critical."
            }
            DiagnosticCode::SchemaContradictionMissingStatus => {
                "Add a `status` field to the contradiction: one of unresolved, resolved, dismissed."
            }
            DiagnosticCode::SchemaContradictionInvalidStatus => {
                "Use a valid contradiction status: one of unresolved, resolved, dismissed."
            }
            DiagnosticCode::SchemaContradictionMissingClaims => {
                "Add a `claims` field listing at least two claim IDs (scalar or `[a.b, c.d]` list)."
            }
            DiagnosticCode::SchemaContradictionClaimsTooFew => {
                "List at least two distinct claim IDs in `claims` using `[a.b, c.d]` list form."
            }
            DiagnosticCode::SchemaContradictionClaimNotFound => {
                "Ensure every claim ID in `claims` refers to an existing `claim` object in the workspace."
            }
            DiagnosticCode::SchemaSourceMissingKind => {
                "Add a `kind` field to the source: one of source_code, test, commit, pull_request, issue, design_doc, human_review, external_url, api_schema, runtime_metric, incident, support_ticket, audit_record, policy_reference, dataset, experiment."
            }
            DiagnosticCode::SchemaSourceInvalidKind => {
                "Use a valid evidence kind: one of source_code, test, commit, pull_request, issue, design_doc, human_review, external_url, api_schema, runtime_metric, incident, support_ticket, audit_record, policy_reference, dataset, experiment."
            }
            DiagnosticCode::SchemaSourceMissingPathOrUrl => {
                "Add either a `path` (repo-relative) or `url` (absolute URL) field to the source object."
            }
            DiagnosticCode::SchemaSourceConflictingPathAndUrl => {
                "Provide only one of `path` or `url` on a source object, not both."
            }
            DiagnosticCode::SchemaSourceInvalidPath => {
                "Use a repo-relative path (e.g. `src/main.rs`); avoid leading `/`, `..` segments, backslashes, and Windows drive letters."
            }
            DiagnosticCode::SchemaSourceInvalidUrl => {
                "Use a well-formed absolute URL with an allowed scheme (http, https, or mailto)."
            }
            DiagnosticCode::SchemaSourceKindTargetMismatch => {
                "The evidence kind restricts target to path-only or url-only. Adjust the `kind`, `path`, or `url` field accordingly."
            }
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        Self::all()
            .iter()
            .copied()
            .find(|code| code.as_str() == value)
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for DiagnosticCode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for DiagnosticCode {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        DiagnosticCode::from_str(&value)
            .ok_or_else(|| de::Error::unknown_variant(&value, DIAGNOSTIC_CODE_VARIANTS))
    }
}

const DIAGNOSTIC_CODE_VARIANTS: &[&str] = &[
    "parse.raw_html",
    "parse.unsafe_link",
    "parse.unclosed_fence",
    "parse.malformed_page_annotation",
    "parse.nested_typed_block",
    "parse.malformed_field",
    "parse.malformed_open_fence",
    "schema.unknown_kind",
    "schema.missing_field",
    "schema.duplicate_field",
    "schema.invalid_status",
    "schema.constraint_missing_severity",
    "schema.constraint_invalid_severity",
    "schema.procedure_missing_status",
    "schema.procedure_missing_body",
    "schema.procedure_body_must_start_with_ordered_list",
    "procedure.verified_missing_evidence",
    "schema.example_missing_lang",
    "schema.example_invalid_lang",
    "schema.example_invalid_sandbox",
    "schema.example_verified_requires_checks",
    "schema.example_verified_requires_sandbox",
    "claim.verified_missing_evidence",
    "claim.status_casing",
    "lifecycle.expired",
    "lifecycle.invalid_expires_at",
    "id.duplicate",
    "id.invalid",
    "ref.broken",
    "io.unreadable_file",
    "io.unreadable_directory",
    "io.unsupported_source_extension",
    "io.artifact_missing",
    "io.artifact_unreadable",
    "io.artifact_malformed",
    "schema.unsupported_version",
    "id.duplicate_in_artifact",
    "retrieval.object_not_found",
    "search.invalid_filter",
    "embed.model_load_failed",
    "embed.compute_failed",
    "embed.unexpected_dim",
    "build.embeddings_cached",
    "build.embeddings_cache_ignored",
    "build.embeddings_skipped",
    "search.artifact_missing",
    "search.model_mismatch",
    "search.hash_drift",
    "search.deterministic_quality",
    "graph.object_not_found",
    "patch.invalid_document",
    "patch.validation_failed",
    "patch.base_hash_mismatch",
    "patch.target_already_exists",
    "patch.placement_invalid",
    "schema.impacts_invalid_path",
    "schema.impacts_empty",
    "schema.policy_missing_status",
    "schema.policy_missing_owner",
    "schema.policy_missing_approved_by",
    "schema.policy_missing_effective_at",
    "schema.policy_invalid_effective_at",
    "schema.policy_invalid_review_interval",
    "schema.policy_missing_body",
    "schema.policy_future_effective_at",
    "schema.agent_instruction_missing_scope",
    "schema.agent_instruction_missing_trust",
    "schema.agent_instruction_invalid_trust",
    "schema.agent_instruction_missing_allowed_actions",
    "schema.agent_instruction_missing_forbidden_actions",
    "schema.agent_instruction_actions_not_disjoint",
    "compat.raw_html_quarantined",
    "compat.unsafe_link_dropped",
    "compat.unsafe_image_src_dropped",
    "compat.unknown_extension",
    "parse.malformed_markdown",
    "retrieval.no_knowledge_objects_consider_migration",
    "schema.contradiction_missing_severity",
    "schema.contradiction_invalid_severity",
    "schema.contradiction_missing_status",
    "schema.contradiction_invalid_status",
    "schema.contradiction_missing_claims",
    "schema.contradiction_claims_too_few",
    "schema.contradiction_claim_not_found",
    "schema.source_missing_kind",
    "schema.source_invalid_kind",
    "schema.source_missing_path_or_url",
    "schema.source_conflicting_path_and_url",
    "schema.source_invalid_path",
    "schema.source_invalid_url",
    "schema.source_kind_target_mismatch",
];

impl Diagnostic {
    /// Constructs the not-found diagnostic emitted when the `why` lookup's
    /// requested object id is absent from the loaded artifact.
    pub fn not_found(object_id: impl Into<String>) -> Self {
        let id = object_id.into();
        Self {
            code: DiagnosticCode::RetrievalObjectNotFound,
            severity: Severity::Error,
            message: format!("Object ID `{id}` was not found in the graph artifact."),
            span: None,
            object_id: Some(id),
            help: Some(
                "Run `adoc build` if the source was changed after the artifact was generated."
                    .to_string(),
            ),
        }
    }

    pub(crate) fn error(code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: Severity::Error,
            message: message.into(),
            span: None,
            object_id: None,
            help: Some(code.default_help().to_string()),
        }
    }

    pub(crate) fn warning(code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: Severity::Warning,
            message: message.into(),
            span: None,
            object_id: None,
            help: Some(code.default_help().to_string()),
        }
    }

    pub(crate) fn info(code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: Severity::Info,
            message: message.into(),
            span: None,
            object_id: None,
            help: Some(code.default_help().to_string()),
        }
    }

    pub fn with_span(mut self, span: SourceSpan) -> Self {
        self.span = Some(span);
        self
    }

    pub(crate) fn with_object_id(mut self, object_id: impl Into<String>) -> Self {
        self.object_id = Some(object_id.into());
        self
    }

    pub(crate) fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }
}

/// A [`Diagnostic`] whose severity is always [`Severity::Warning`].
///
/// ADR-0023 requires every Compatibility-Mode validation rule to emit only
/// warnings — never errors. Encoded as a newtype around `Diagnostic` whose
/// only constructor is [`CompatDiagnostic::warning`]; the registry boundary
/// calls [`CompatDiagnostic::into_diagnostic`] to unwrap once and forwards
/// the inner value to the orchestrator's diagnostic stream.
///
/// Adding an `error`/`info` constructor here, or impl'ing `From<Diagnostic>`,
/// would erase the invariant and is intentionally absent. Compat rules
/// implement [`crate::domain::rules::CompatRule`] (not `ValidationRule`) so
/// their sink type carries this newtype at compile time.
#[derive(Debug, Clone)]
pub(crate) struct CompatDiagnostic(Diagnostic);

impl CompatDiagnostic {
    /// Construct a warning-severity compat diagnostic. The only way to make
    /// one of these — `error`/`info` constructors do not exist.
    pub(crate) fn warning(code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self(Diagnostic::warning(code, message))
    }

    pub(crate) fn with_span(self, span: SourceSpan) -> Self {
        Self(self.0.with_span(span))
    }

    #[allow(dead_code)]
    pub(crate) fn with_object_id(self, object_id: impl Into<String>) -> Self {
        Self(self.0.with_object_id(object_id))
    }

    #[allow(dead_code)]
    pub(crate) fn with_help(self, help: impl Into<String>) -> Self {
        Self(self.0.with_help(help))
    }

    /// Unwrap to the inner [`Diagnostic`]. Called by the compat registry at
    /// the seam between compat-only rules and the unified diagnostic stream.
    pub(crate) fn into_diagnostic(self) -> Diagnostic {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl fmt::Display for Severity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => formatter.write_str("error"),
            Severity::Warning => formatter.write_str("warning"),
            Severity::Info => formatter.write_str("info"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SourceSpan {
    pub file: PathBuf,
    pub start: SourcePosition,
    pub end: SourcePosition,
}

impl SourceSpan {
    pub(crate) fn render_location(&self) -> String {
        format!(
            "{}:{}:{}",
            self.file.display(),
            self.start.line,
            self.start.column
        )
    }
}

impl Serialize for SourceSpan {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("SourceSpan", 3)?;
        state.serialize_field("file", &self.file.display().to_string())?;
        state.serialize_field("start", &self.start)?;
        state.serialize_field("end", &self.end)?;
        state.end()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourcePosition {
    pub line: u32,
    pub column: u32,
    pub offset: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span_with_file(file: PathBuf) -> SourceSpan {
        SourceSpan {
            file,
            start: SourcePosition {
                line: 1,
                column: 1,
                offset: 0,
            },
            end: SourcePosition {
                line: 1,
                column: 5,
                offset: 4,
            },
        }
    }

    #[test]
    fn source_span_serializes_file_as_display_string() {
        let value =
            serde_json::to_value(span_with_file(PathBuf::from("docs/sample.adoc"))).unwrap();

        assert_eq!(value["file"], "docs/sample.adoc");
        assert_eq!(value["start"]["line"], 1);
        assert_eq!(value["end"]["column"], 5);
    }

    #[test]
    fn source_span_renders_start_location() {
        let span = span_with_file(PathBuf::from("docs/sample.adoc"));

        assert_eq!(span.render_location(), "docs/sample.adoc:1:1");
    }

    #[test]
    fn with_help_sets_help_field() {
        let diagnostic =
            Diagnostic::error(DiagnosticCode::IdInvalid, "bad id").with_help("fix your id");
        assert_eq!(diagnostic.help.as_deref(), Some("fix your id"));
    }

    #[test]
    fn compat_diagnostic_is_always_warning_severity() {
        // ADR-0023 invariant: every Compat Validation Rule emits warnings
        // only. The type system enforces this — `CompatDiagnostic::warning`
        // is the only constructor — so this test only pins that the unwrap
        // hands back the warning severity. There is no `CompatDiagnostic::error`
        // to call; absence is the invariant.
        let compat = CompatDiagnostic::warning(DiagnosticCode::CompatRawHtmlQuarantined, "msg");
        assert_eq!(compat.into_diagnostic().severity, Severity::Warning);
    }

    #[test]
    fn constructors_attach_default_help() {
        let error = Diagnostic::error(DiagnosticCode::ParseRawHtml, "raw html is not allowed");
        let warning = Diagnostic::warning(DiagnosticCode::ClaimStatusCasing, "status casing");

        assert_eq!(
            error.help.as_deref(),
            Some(DiagnosticCode::ParseRawHtml.default_help())
        );
        assert_eq!(
            warning.help.as_deref(),
            Some(DiagnosticCode::ClaimStatusCasing.default_help())
        );
    }

    #[test]
    fn default_help_is_complete_for_every_wire_code() {
        let codes = DiagnosticCode::all();
        let wire_codes: Vec<&str> = codes.iter().map(|code| code.as_str()).collect();

        assert_eq!(wire_codes, DIAGNOSTIC_CODE_VARIANTS);

        for code in codes {
            let help = code.default_help();
            assert!(
                !help.trim().is_empty(),
                "{} should have non-empty default help",
                code
            );
            assert_eq!(DiagnosticCode::from_str(code.as_str()), Some(*code));
        }
    }

    #[test]
    fn search_invalid_filter_code_roundtrips_through_wire_string() {
        let value = serde_json::to_value(DiagnosticCode::SearchInvalidFilter)
            .expect("diagnostic code serializes");

        assert_eq!(value, "search.invalid_filter");
        assert_eq!(
            serde_json::from_value::<DiagnosticCode>(value).expect("diagnostic code deserializes"),
            DiagnosticCode::SearchInvalidFilter
        );
        assert!(
            DiagnosticCode::SearchInvalidFilter
                .default_help()
                .contains("filter")
        );
    }

    #[test]
    fn search_artifact_missing_code_roundtrips_through_wire_string() {
        let value = serde_json::to_value(DiagnosticCode::SearchArtifactMissing)
            .expect("diagnostic code serializes");
        assert_eq!(value, "search.artifact_missing");
        assert_eq!(
            serde_json::from_value::<DiagnosticCode>(value).expect("diagnostic code deserializes"),
            DiagnosticCode::SearchArtifactMissing
        );
        assert!(
            !DiagnosticCode::SearchArtifactMissing
                .default_help()
                .is_empty()
        );
    }

    #[test]
    fn search_model_mismatch_code_roundtrips_through_wire_string() {
        let value = serde_json::to_value(DiagnosticCode::SearchModelMismatch)
            .expect("diagnostic code serializes");
        assert_eq!(value, "search.model_mismatch");
        assert_eq!(
            serde_json::from_value::<DiagnosticCode>(value).expect("diagnostic code deserializes"),
            DiagnosticCode::SearchModelMismatch
        );
        assert!(
            !DiagnosticCode::SearchModelMismatch
                .default_help()
                .is_empty()
        );
    }

    #[test]
    fn search_hash_drift_code_roundtrips_through_wire_string() {
        let value = serde_json::to_value(DiagnosticCode::SearchHashDrift)
            .expect("diagnostic code serializes");
        assert_eq!(value, "search.hash_drift");
        assert_eq!(
            serde_json::from_value::<DiagnosticCode>(value).expect("diagnostic code deserializes"),
            DiagnosticCode::SearchHashDrift
        );
        assert!(!DiagnosticCode::SearchHashDrift.default_help().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn source_span_serializes_non_utf8_file_as_display_string() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let file = PathBuf::from(OsString::from_vec(vec![b'd', b'o', b'c', b's', b'/', 0xff]));
        let value = serde_json::to_value(span_with_file(file)).unwrap();

        assert!(
            value["file"].is_string(),
            "display-based serialization must not fail for non-UTF-8 paths"
        );
    }
}
