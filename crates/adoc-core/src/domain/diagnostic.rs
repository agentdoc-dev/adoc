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
    PatchSourceDrift,
    PatchCreateMissingPlacement,
    PatchPlacementNotAdoc,
    McpPatchApplyDisabled,
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
    /// V5.10 TB1: an `active` policy's `effective_at + review_interval` is
    /// strictly before today — the review is overdue.
    SchemaPolicyReviewOverdue,
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
    SchemaContradictionClaimNotAClaim,
    SchemaSourceMissingKind,
    SchemaSourceInvalidKind,
    SchemaSourceMissingPathOrUrl,
    SchemaSourceConflictingPathAndUrl,
    SchemaSourceInvalidPath,
    SchemaSourceInvalidUrl,
    SchemaSourceKindTargetMismatch,
    /// V6.5.1: `api` Knowledge Object (PRD §13.7).
    SchemaApiMissingMethodOrInterfaceType,
    SchemaApiConflictingMethodAndInterfaceType,
    SchemaApiInvalidMethod,
    SchemaApiMissingPathOrSymbol,
    SchemaApiConflictingPathAndSymbol,
    SchemaApiInvalidPath,
    /// V6.5.1: a `verified` api has neither an inline `source_code` evidence
    /// entry nor an `evidence_ref` resolving to an `api_schema`/`source_code`
    /// source — an API contract is verified by its schema source.
    ApiVerifiedMissingSchemaEvidence,
    /// V6.5.2: `observation` Knowledge Object (PRD §13.9).
    SchemaObservationMissingStatus,
    SchemaObservationInvalidStatus,
    SchemaObservationInvalidSampleSize,
    SchemaObservationInvalidObservedAt,
    /// V6.5.3: `question` Knowledge Object (PRD §13.10).
    SchemaQuestionMissingStatus,
    /// V6.5.3: an `answered` question does not name the object that answered
    /// it via `resolved_by:`.
    SchemaQuestionAnsweredMissingResolvedBy,
    /// V6.5.3: the `resolved_by:` on a question names an Object ID that does
    /// not exist anywhere in the workspace.
    SchemaQuestionResolvedByNotFound,
    /// V6.5.3: the `resolved_by:` on a question names an Object ID that exists
    /// but is neither a `claim` nor a `decision`.
    SchemaQuestionResolvedByWrongKind,
    /// V6.5.3: a non-`answered` question carries a `resolved_by:` field —
    /// only answered questions name the object that answered them.
    SchemaQuestionUnexpectedResolvedBy,
    /// V6.5.4: `task` Knowledge Object (PRD §13.11).
    SchemaTaskMissingOwner,
    SchemaTaskMissingStatus,
    SchemaTaskInvalidStatus,
    SchemaTaskInvalidDue,
    /// V6.5.4: an `open` task's `due` date is strictly before today. WARNING
    /// severity — clock-dependent, so fixture dates use the wide-margin
    /// discipline (the `schema.policy_review_overdue` precedent).
    TaskOverdue,
    /// V5.8 TB2: the `evidence_ref:` on a claim names an Object ID that does
    /// not exist anywhere in the workspace.
    SchemaEvidenceTargetNotFound,
    /// V5.8 TB2: the `evidence_ref:` on a claim names an Object ID that exists
    /// but is not a `source` Knowledge Object.
    SchemaEvidenceTargetNotASource,
    /// V5.10 TB3: a `verified` claim's best inline evidence tier is Low-only.
    ///
    /// Emitted when a verified claim has at least one inline evidence entry but
    /// every inline entry maps to the `Low` evidence tier, and the claim has no
    /// `ObjectRef` evidence (which counts as ≥ Medium per ADR-0034).
    ClaimEvidenceQualityLow,
    /// V5.10 TB4: a `claim` is referenced by an `unresolved` contradiction
    /// but its authored `status` is not `"contradicted"`.
    ///
    /// This is a WARNING nudge only — the authored `status` is never mutated
    /// (ADR-0026). The effective `contradicted` state is projected at graph/HTML
    /// output time without touching the authored field.
    SchemaClaimContradictedByUnresolved,
    /// V6.3: a positional `adoc impacted-by` path argument is not a valid
    /// repo-relative path (absolute, escaping, or empty).
    ImpactedInvalidPath,
    /// V6.3: the `--ref` base could not be resolved in this repository.
    ImpactedRefUnresolvable,
    /// V6.3: git itself was unavailable or failed while deriving the
    /// changed-file set for `--ref`.
    ImpactedGitUnavailable,
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
            DiagnosticCode::PatchSourceDrift,
            DiagnosticCode::PatchCreateMissingPlacement,
            DiagnosticCode::PatchPlacementNotAdoc,
            DiagnosticCode::McpPatchApplyDisabled,
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
            DiagnosticCode::SchemaPolicyReviewOverdue,
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
            DiagnosticCode::SchemaContradictionClaimNotAClaim,
            DiagnosticCode::SchemaSourceMissingKind,
            DiagnosticCode::SchemaSourceInvalidKind,
            DiagnosticCode::SchemaSourceMissingPathOrUrl,
            DiagnosticCode::SchemaSourceConflictingPathAndUrl,
            DiagnosticCode::SchemaSourceInvalidPath,
            DiagnosticCode::SchemaSourceInvalidUrl,
            DiagnosticCode::SchemaSourceKindTargetMismatch,
            DiagnosticCode::SchemaApiMissingMethodOrInterfaceType,
            DiagnosticCode::SchemaApiConflictingMethodAndInterfaceType,
            DiagnosticCode::SchemaApiInvalidMethod,
            DiagnosticCode::SchemaApiMissingPathOrSymbol,
            DiagnosticCode::SchemaApiConflictingPathAndSymbol,
            DiagnosticCode::SchemaApiInvalidPath,
            DiagnosticCode::ApiVerifiedMissingSchemaEvidence,
            DiagnosticCode::SchemaObservationMissingStatus,
            DiagnosticCode::SchemaObservationInvalidStatus,
            DiagnosticCode::SchemaObservationInvalidSampleSize,
            DiagnosticCode::SchemaObservationInvalidObservedAt,
            DiagnosticCode::SchemaQuestionMissingStatus,
            DiagnosticCode::SchemaQuestionAnsweredMissingResolvedBy,
            DiagnosticCode::SchemaQuestionResolvedByNotFound,
            DiagnosticCode::SchemaQuestionResolvedByWrongKind,
            DiagnosticCode::SchemaQuestionUnexpectedResolvedBy,
            DiagnosticCode::SchemaTaskMissingOwner,
            DiagnosticCode::SchemaTaskMissingStatus,
            DiagnosticCode::SchemaTaskInvalidStatus,
            DiagnosticCode::SchemaTaskInvalidDue,
            DiagnosticCode::TaskOverdue,
            DiagnosticCode::SchemaEvidenceTargetNotFound,
            DiagnosticCode::SchemaEvidenceTargetNotASource,
            DiagnosticCode::ClaimEvidenceQualityLow,
            DiagnosticCode::SchemaClaimContradictedByUnresolved,
            DiagnosticCode::ImpactedInvalidPath,
            DiagnosticCode::ImpactedRefUnresolvable,
            DiagnosticCode::ImpactedGitUnavailable,
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
            DiagnosticCode::PatchSourceDrift => "patch.source_drift",
            DiagnosticCode::PatchCreateMissingPlacement => "patch.create_missing_placement",
            DiagnosticCode::PatchPlacementNotAdoc => "patch.placement_not_adoc",
            DiagnosticCode::McpPatchApplyDisabled => "mcp.patch_apply_disabled",
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
            DiagnosticCode::SchemaPolicyReviewOverdue => "schema.policy_review_overdue",
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
            DiagnosticCode::SchemaContradictionClaimNotAClaim => {
                "schema.contradiction_claim_not_a_claim"
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
            DiagnosticCode::SchemaApiMissingMethodOrInterfaceType => {
                "schema.api_missing_method_or_interface_type"
            }
            DiagnosticCode::SchemaApiConflictingMethodAndInterfaceType => {
                "schema.api_conflicting_method_and_interface_type"
            }
            DiagnosticCode::SchemaApiInvalidMethod => "schema.api_invalid_method",
            DiagnosticCode::SchemaApiMissingPathOrSymbol => "schema.api_missing_path_or_symbol",
            DiagnosticCode::SchemaApiConflictingPathAndSymbol => {
                "schema.api_conflicting_path_and_symbol"
            }
            DiagnosticCode::SchemaApiInvalidPath => "schema.api_invalid_path",
            DiagnosticCode::SchemaObservationMissingStatus => "schema.observation_missing_status",
            DiagnosticCode::SchemaObservationInvalidStatus => "schema.observation_invalid_status",
            DiagnosticCode::SchemaObservationInvalidSampleSize => {
                "schema.observation_invalid_sample_size"
            }
            DiagnosticCode::SchemaObservationInvalidObservedAt => {
                "schema.observation_invalid_observed_at"
            }
            DiagnosticCode::ApiVerifiedMissingSchemaEvidence => {
                "api.verified_missing_schema_evidence"
            }
            DiagnosticCode::SchemaQuestionMissingStatus => "schema.question_missing_status",
            DiagnosticCode::SchemaQuestionAnsweredMissingResolvedBy => {
                "schema.question_answered_missing_resolved_by"
            }
            DiagnosticCode::SchemaQuestionResolvedByNotFound => {
                "schema.question_resolved_by_not_found"
            }
            DiagnosticCode::SchemaQuestionResolvedByWrongKind => {
                "schema.question_resolved_by_wrong_kind"
            }
            DiagnosticCode::SchemaQuestionUnexpectedResolvedBy => {
                "schema.question_unexpected_resolved_by"
            }
            DiagnosticCode::SchemaTaskMissingOwner => "schema.task_missing_owner",
            DiagnosticCode::SchemaTaskMissingStatus => "schema.task_missing_status",
            DiagnosticCode::SchemaTaskInvalidStatus => "schema.task_invalid_status",
            DiagnosticCode::SchemaTaskInvalidDue => "schema.task_invalid_due",
            DiagnosticCode::TaskOverdue => "task.overdue",
            DiagnosticCode::SchemaEvidenceTargetNotFound => "schema.evidence_target_not_found",
            DiagnosticCode::SchemaEvidenceTargetNotASource => "schema.evidence_target_not_a_source",
            DiagnosticCode::ClaimEvidenceQualityLow => "claim.evidence_quality_low",
            DiagnosticCode::SchemaClaimContradictedByUnresolved => {
                "schema.claim_contradicted_by_unresolved"
            }
            DiagnosticCode::ImpactedInvalidPath => "impacted.invalid_path",
            DiagnosticCode::ImpactedRefUnresolvable => "impacted.ref_unresolvable",
            DiagnosticCode::ImpactedGitUnavailable => "impacted.git_unavailable",
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
            DiagnosticCode::PatchSourceDrift => {
                "Source changed since last build; run adoc build and re-propose the patch."
            }
            DiagnosticCode::PatchCreateMissingPlacement => {
                "Add changes.placement with a page_id (and optional after) so apply knows where to insert the new block."
            }
            DiagnosticCode::PatchPlacementNotAdoc => {
                "Place the new object on an .adoc page; .md pages cannot host typed blocks."
            }
            DiagnosticCode::McpPatchApplyDisabled => {
                "Set `mcp: { patch_apply: enabled }` in agentdoc.config.yaml to opt this project into MCP patch apply; adoc_patch_check remains available."
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
            DiagnosticCode::SchemaPolicyReviewOverdue => {
                "Re-review the policy and update `effective_at`, or adjust `review_interval`."
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
            DiagnosticCode::SchemaContradictionClaimNotAClaim => {
                "Every `claims` entry must reference a `claim` object; this ID resolves to a different kind."
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
            DiagnosticCode::SchemaApiMissingMethodOrInterfaceType => {
                "Add either a `method` (HTTP method, e.g. POST) or an `interface_type` (e.g. grpc, graphql) field to the api object."
            }
            DiagnosticCode::SchemaApiConflictingMethodAndInterfaceType => {
                "Provide only one of `method` or `interface_type` on an api object, not both."
            }
            DiagnosticCode::SchemaApiInvalidMethod => {
                "Use an uppercase HTTP method: one of GET, HEAD, POST, PUT, DELETE, CONNECT, OPTIONS, TRACE, PATCH."
            }
            DiagnosticCode::SchemaApiMissingPathOrSymbol => {
                "Add either a `path` (`/`-prefixed route template) or a `symbol` (code symbol) field to the api object."
            }
            DiagnosticCode::SchemaApiConflictingPathAndSymbol => {
                "Provide only one of `path` or `symbol` on an api object, not both."
            }
            DiagnosticCode::SchemaApiInvalidPath => {
                "Use a non-empty `/`-prefixed route template (e.g. `/api/billing/credits/consume`)."
            }
            DiagnosticCode::ApiVerifiedMissingSchemaEvidence => {
                "A verified api requires schema evidence: an inline `source:` entry or an `evidence_ref` to an `api_schema`/`source_code` source object."
            }
            DiagnosticCode::SchemaObservationMissingStatus => {
                "Add a `status` field to the observation. The only valid observation status is: observed."
            }
            DiagnosticCode::SchemaObservationInvalidStatus => {
                "The only valid observation status is: observed."
            }
            DiagnosticCode::SchemaObservationInvalidSampleSize => {
                "Use a positive integer for `sample_size` (e.g. `37`)."
            }
            DiagnosticCode::SchemaObservationInvalidObservedAt => {
                "Use a valid `YYYY-MM-DD` date for `observed_at`."
            }
            DiagnosticCode::SchemaQuestionMissingStatus => {
                "Questions require non-empty `status`. Valid question statuses are: open, answered."
            }
            DiagnosticCode::SchemaQuestionAnsweredMissingResolvedBy => {
                "An answered question must name the knowledge that answered it: add `resolved_by: <object-id>` referencing a `claim` or `decision`."
            }
            DiagnosticCode::SchemaQuestionResolvedByNotFound => {
                "Ensure the `resolved_by` ID refers to an existing object in the workspace."
            }
            DiagnosticCode::SchemaQuestionResolvedByWrongKind => {
                "`resolved_by` must reference a `claim` or `decision` object — the knowledge that answered the question."
            }
            DiagnosticCode::SchemaQuestionUnexpectedResolvedBy => {
                "Remove `resolved_by` or set `status: answered`."
            }
            DiagnosticCode::SchemaTaskMissingOwner => {
                "Tasks require a non-empty `owner` field; a task without an owner is a wish."
            }
            DiagnosticCode::SchemaTaskMissingStatus => {
                "Tasks require non-empty `status`. Valid task statuses are: open, done."
            }
            DiagnosticCode::SchemaTaskInvalidStatus => "Valid task statuses are: open, done.",
            DiagnosticCode::SchemaTaskInvalidDue => "Use a valid `YYYY-MM-DD` date for `due`.",
            DiagnosticCode::TaskOverdue => {
                "Complete the task and set `status: done`, or move its `due` date."
            }
            DiagnosticCode::SchemaEvidenceTargetNotFound => {
                "Ensure every `evidence_ref` ID refers to an existing `source` object in the workspace."
            }
            DiagnosticCode::SchemaEvidenceTargetNotASource => {
                "An `evidence_ref` must point to a `source` Knowledge Object; update the ID or change the referenced object's kind."
            }
            DiagnosticCode::ClaimEvidenceQualityLow => {
                "This verified claim relies only on low-quality evidence (external URL, issue, ticket, metric, dataset, or experiment). Add a test, source-code reference, API schema, audit record, or policy reference to strengthen verification."
            }
            DiagnosticCode::SchemaClaimContradictedByUnresolved => {
                "This claim is referenced by an unresolved contradiction. Consider setting `status: contradicted` on the claim to make its effective state explicit. The effective_status is already projected as `contradicted` in graph and HTML output regardless of the authored status."
            }
            DiagnosticCode::ImpactedInvalidPath => {
                "Pass repo-relative paths as emitted by `git diff --name-only`, e.g. `crates/billing/src/refund.rs` — not absolute paths and not paths escaping the repository."
            }
            DiagnosticCode::ImpactedRefUnresolvable => {
                "Use a ref resolvable in this repository, e.g. `main` or `HEAD~1`."
            }
            DiagnosticCode::ImpactedGitUnavailable => {
                "Install git and run inside a git repository, or pass explicit changed paths instead of `--ref`."
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
    "patch.source_drift",
    "patch.create_missing_placement",
    "patch.placement_not_adoc",
    "mcp.patch_apply_disabled",
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
    "schema.policy_review_overdue",
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
    "schema.contradiction_claim_not_a_claim",
    "schema.source_missing_kind",
    "schema.source_invalid_kind",
    "schema.source_missing_path_or_url",
    "schema.source_conflicting_path_and_url",
    "schema.source_invalid_path",
    "schema.source_invalid_url",
    "schema.source_kind_target_mismatch",
    "schema.api_missing_method_or_interface_type",
    "schema.api_conflicting_method_and_interface_type",
    "schema.api_invalid_method",
    "schema.api_missing_path_or_symbol",
    "schema.api_conflicting_path_and_symbol",
    "schema.api_invalid_path",
    "api.verified_missing_schema_evidence",
    "schema.observation_missing_status",
    "schema.observation_invalid_status",
    "schema.observation_invalid_sample_size",
    "schema.observation_invalid_observed_at",
    "schema.question_missing_status",
    "schema.question_answered_missing_resolved_by",
    "schema.question_resolved_by_not_found",
    "schema.question_resolved_by_wrong_kind",
    "schema.question_unexpected_resolved_by",
    "schema.task_missing_owner",
    "schema.task_missing_status",
    "schema.task_invalid_status",
    "schema.task_invalid_due",
    "task.overdue",
    "schema.evidence_target_not_found",
    "schema.evidence_target_not_a_source",
    "claim.evidence_quality_low",
    "schema.claim_contradicted_by_unresolved",
    "impacted.invalid_path",
    "impacted.ref_unresolvable",
    "impacted.git_unavailable",
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
