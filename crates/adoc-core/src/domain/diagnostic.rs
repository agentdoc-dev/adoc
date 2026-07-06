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

/// Single source of truth for every Diagnostic Code: one row per code
/// carrying `(Variant = "wire.string" => "default help";)`. Expands to the
/// `DiagnosticCode` enum, `all()`, `as_str()`, `default_help()`, and
/// `DIAGNOSTIC_CODE_VARIANTS`, so a new code is exactly one row and
/// serialize/deserialize/help completeness is guaranteed by construction —
/// previously these were five hand-synced lists, and a variant missing from
/// the two the compiler never checked (`all()`, `DIAGNOSTIC_CODE_VARIANTS`)
/// would serialize but silently fail to deserialize.
macro_rules! diagnostic_codes {
    ($($(#[$meta:meta])* $variant:ident = $wire:literal => $help:literal;)+) => {
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
            $($(#[$meta])* $variant,)+
        }

        const DIAGNOSTIC_CODE_VARIANTS: &[&str] = &[$($wire),+];

        impl DiagnosticCode {
            fn all() -> &'static [Self] {
                &[$(DiagnosticCode::$variant),+]
            }

            pub fn as_str(self) -> &'static str {
                match self {
                    $(DiagnosticCode::$variant => $wire,)+
                }
            }

            pub fn default_help(self) -> &'static str {
                match self {
                    $(DiagnosticCode::$variant => $help,)+
                }
            }
        }
    };
}

diagnostic_codes! {
    ParseRawHtml = "parse.raw_html" =>
        "Remove raw HTML or replace it with supported Markdown/ADoc syntax.";
    ParseUnsafeLink = "parse.unsafe_link" =>
        "Use a safe link scheme such as https, http, mailto, or a relative path.";
    ParseUnclosedFence = "parse.unclosed_fence" =>
        "Close the typed block with a matching fence before the end of the file.";
    ParseMalformedPageAnnotation = "parse.malformed_page_annotation" =>
        "Use a page annotation in the form `@doc(object.id)` with a valid Object ID.";
    ParseNestedTypedBlock = "parse.nested_typed_block" =>
        "Move nested typed blocks out of the current block body or field value.";
    ParseMalformedField = "parse.malformed_field" =>
        "Write typed block fields as `key: value` lines before the block body.";
    ParseMalformedOpenFence = "parse.malformed_open_fence" =>
        "Open typed blocks with `::kind object.id`, using a supported kind and valid Object ID.";
    SchemaUnknownKind = "schema.unknown_kind" =>
        "Use a supported object kind or update the schema before compiling.";
    SchemaMissingField = "schema.missing_field" =>
        "Add the required field with a non-empty value.";
    SchemaDuplicateField = "schema.duplicate_field" =>
        "Keep only one value for each field inside the object.";
    SchemaInvalidStatus = "schema.invalid_status" =>
        "Use one of the allowed status values for this object kind.";
    SchemaConstraintMissingSeverity = "schema.constraint_missing_severity" =>
        "Add a `severity` field to the constraint: one of low, medium, high, critical.";
    SchemaConstraintInvalidSeverity = "schema.constraint_invalid_severity" =>
        "Use a valid constraint severity: one of low, medium, high, critical.";
    SchemaProcedureMissingStatus = "schema.procedure_missing_status" =>
        "Add a `status` field to the procedure: one of draft, verified, deprecated.";
    SchemaProcedureMissingBody = "schema.procedure_missing_body" =>
        "Add a non-empty body to the procedure describing its ordered steps.";
    SchemaProcedureBodyMustStartWithOrderedList = "schema.procedure_body_must_start_with_ordered_list" =>
        "Begin the procedure body with an ordered list; write the steps as `1. ...`, `2. ...`.";
    ProcedureVerifiedMissingEvidence = "procedure.verified_missing_evidence" =>
        "Add at least one evidence field (`source`, `human_review`, or `reviewed_by`) before marking the procedure as verified.";
    SchemaExampleMissingLang = "schema.example_missing_lang" =>
        "An example requires either `lang` or `format`.";
    SchemaExampleInvalidLang = "schema.example_invalid_lang" =>
        "Valid `lang` is a lowercase token matching [a-z][a-z0-9_+-]* (e.g. `ts`, `python`, `c++`).";
    SchemaExampleInvalidSandbox = "schema.example_invalid_sandbox" =>
        "Valid `sandbox` is a lowercase token matching [a-z][a-z0-9_+:-]* (e.g. `node-test`, `docker:node-test`).";
    SchemaExampleVerifiedRequiresChecks = "schema.example_verified_requires_checks" =>
        "A verified example requires both `checks` and `sandbox`.";
    SchemaExampleVerifiedRequiresSandbox = "schema.example_verified_requires_sandbox" =>
        "A verified example requires both `checks` and `sandbox`.";
    ClaimVerifiedMissingEvidence = "claim.verified_missing_evidence" =>
        "Add evidence entries before marking the claim as verified.";
    ClaimStatusCasing = "claim.status_casing" =>
        "Use the canonical lowercase claim status value.";
    LifecycleExpired = "lifecycle.expired" =>
        "Update `expires_at` or remove it if this Knowledge Object is still valid.";
    LifecycleInvalidExpiresAt = "lifecycle.invalid_expires_at" =>
        "Use `YYYY-MM-DD` for `expires_at`, or remove the field.";
    IdDuplicate = "id.duplicate" =>
        "Give each object a unique ID across the compiled workspace.";
    IdInvalid = "id.invalid" =>
        "Use a valid Object ID with lowercase segments separated by dots.";
    RefBroken = "ref.broken" =>
        "Update the reference to an existing object ID or remove the reference.";
    IoUnreadableFile = "io.unreadable_file" =>
        "Check that the source path exists and can be read by the current user.";
    IoUnreadableDirectory = "io.unreadable_directory" =>
        "Check that the source directory exists and can be read by the current user.";
    IoUnsupportedSourceExtension = "io.unsupported_source_extension" =>
        "Use supported source files with the `.adoc` or `.md` extension.";
    IoArtifactMissing = "io.artifact_missing" =>
        "Build docs.graph.json before loading the retrieval artifact.";
    IoArtifactUnreadable = "io.artifact_unreadable" =>
        "Check that docs.graph.json exists and can be read by the current user.";
    IoArtifactMalformed = "io.artifact_malformed" =>
        "Rebuild docs.graph.json from valid source documents.";
    SchemaUnsupportedVersion = "schema.unsupported_version" =>
        "Regenerate the artifact with a schema version supported by this binary.";
    IdDuplicateInArtifact = "id.duplicate_in_artifact" =>
        "Rebuild the artifact after removing duplicate object IDs from the source.";
    RetrievalObjectNotFound = "retrieval.object_not_found" =>
        "Use an object ID present in the loaded retrieval artifact.";
    SearchInvalidFilter = "search.invalid_filter" =>
        "Change or remove the filter so it matches at least one object field in the artifact.";
    SearchInvalidScope = "search.invalid_scope" =>
        "Use lexical or blended search for prose records, and drop Knowledge Object metadata filters from a prose-only query.";
    EmbedModelLoadFailed = "embed.model_load_failed" =>
        "Check network access for the first model download, verify the local model cache is readable, ensure the binary was built with the `embeddings` feature, or rerun `adoc build --no-embeddings`.";
    EmbedComputeFailed = "embed.compute_failed" =>
        "Retry `adoc build`; if the error repeats, rebuild with `--no-embeddings` while investigating the embedding provider.";
    EmbedUnexpectedDimension = "embed.unexpected_dim" =>
        "Use an embedding provider that returns exactly one vector per input and the configured vector dimension.";
    BuildEmbeddingsCached = "build.embeddings_cached" =>
        "No action is required; this reports search artifact embedding cache reuse.";
    BuildEmbeddingsCacheIgnored = "build.embeddings_cache_ignored" =>
        "No action is required; the search cache will be recomputed for the current embedding model.";
    BuildEmbeddingsSkipped = "build.embeddings_skipped" =>
        "Run `adoc build` without `--no-embeddings` to emit docs.search.json.";
    SearchArtifactMissing = "search.artifact_missing" =>
        "Run `adoc build` to generate dist/docs.search.json for hybrid or semantic search.";
    SearchModelMismatch = "search.model_mismatch" =>
        "Rebuild dist/docs.search.json with the active embedding provider, or switch providers to match the artifact's model header.";
    SearchHashDrift = "search.hash_drift" =>
        "Re-run `adoc build` to regenerate dist/docs.search.json from the current graph artifact.";
    SearchDeterministicQuality = "search.deterministic_quality" =>
        "Use a semantic embedding provider for quality-sensitive retrieval; deterministic embeddings are repeatable but non-semantic.";
    GraphObjectNotFound = "graph.object_not_found" =>
        "Use an object ID present in the loaded graph artifact, or rebuild docs.graph.json.";
    PatchInvalidDocument = "patch.invalid_document" =>
        "Use the adoc.patch.v0 schema with exactly one supported operation and its required fields.";
    PatchValidationFailed = "patch.validation_failed" =>
        "Adjust the patch intent so it satisfies AgentDoc patch validation rules.";
    PatchBaseHashMismatch = "patch.base_hash_mismatch" =>
        "Rebuild docs.graph.json or regenerate the patch against the current target content_hash.";
    PatchTargetAlreadyExists = "patch.target_already_exists" =>
        "Use create_object only for a new Object ID, or choose an update operation for an existing object.";
    PatchPlacementInvalid = "patch.placement_invalid" =>
        "Use an existing page_id and, when after is supplied, an object already on that page.";
    PatchSourceDrift = "patch.source_drift" =>
        "Source changed since last build; run adoc build and re-propose the patch.";
    PatchCreateMissingPlacement = "patch.create_missing_placement" =>
        "Add changes.placement with a page_id (and optional after) so apply knows where to insert the new block.";
    PatchPlacementNotAdoc = "patch.placement_not_adoc" =>
        "Place the new object on an .adoc page; .md pages cannot host typed blocks.";
    McpPatchApplyDisabled = "mcp.patch_apply_disabled" =>
        "Set `mcp: { patch_apply: enabled }` in agentdoc.config.yaml to opt this project into MCP patch apply; adoc_patch_check remains available.";
    SchemaImpactsInvalidPath = "schema.impacts_invalid_path" =>
        "Use a repo-relative path under the project root; remove leading `/`, `..` segments, and blank entries.";
    SchemaImpactsEmpty = "schema.impacts_empty" =>
        "Remove the `impacts:` field entirely instead of leaving it empty; impacts must list at least one path.";
    SchemaPolicyMissingStatus = "schema.policy_missing_status" =>
        "Add a `status` field to the policy: one of proposed, active, archived, revoked.";
    SchemaPolicyMissingOwner = "schema.policy_missing_owner" =>
        "Add a non-empty `owner` field to the policy.";
    SchemaPolicyMissingApprovedBy = "schema.policy_missing_approved_by" =>
        "Add an `approved_by` field listing at least one approver (scalar or `[a, b]` list).";
    SchemaPolicyMissingEffectiveAt = "schema.policy_missing_effective_at" =>
        "Add an `effective_at` field in `YYYY-MM-DD` format.";
    SchemaPolicyInvalidEffectiveAt = "schema.policy_invalid_effective_at" =>
        "Use a valid `YYYY-MM-DD` date for `effective_at`.";
    SchemaPolicyInvalidReviewInterval = "schema.policy_invalid_review_interval" =>
        "Use a valid review interval in `[0-9]+d` form for `review_interval` (e.g. `90d`).";
    SchemaPolicyMissingBody = "schema.policy_missing_body" =>
        "Add a non-empty body to the policy describing its rules.";
    SchemaPolicyFutureEffectiveAt = "schema.policy_future_effective_at" =>
        "An `active` policy must have an `effective_at` date on or before today.";
    /// V5.10 TB1: an `active` policy's `effective_at + review_interval` is
    /// strictly before today — the review is overdue.
    SchemaPolicyReviewOverdue = "schema.policy_review_overdue" =>
        "Re-review the policy and update `effective_at`, or adjust `review_interval`.";
    SchemaAgentInstructionMissingScope = "schema.agent_instruction_missing_scope" =>
        "Add a `scope` field to the agent_instruction with a non-empty glob pattern (e.g. `docs/auth/*`).";
    SchemaAgentInstructionMissingTrust = "schema.agent_instruction_missing_trust" =>
        "Add a `trust` field to the agent_instruction: one of informal, team, authoritative, regulated, system.";
    SchemaAgentInstructionInvalidTrust = "schema.agent_instruction_invalid_trust" =>
        "Use a valid trust level: one of informal, team, authoritative, regulated, system.";
    SchemaAgentInstructionMissingAllowedActions = "schema.agent_instruction_missing_allowed_actions" =>
        "Add an `allowed_actions` field listing at least one action (scalar or `[a, b]` list).";
    SchemaAgentInstructionMissingForbiddenActions = "schema.agent_instruction_missing_forbidden_actions" =>
        "Add a `forbidden_actions` field listing at least one action (scalar or `[a, b]` list).";
    SchemaAgentInstructionActionsNotDisjoint = "schema.agent_instruction_actions_not_disjoint" =>
        "Remove the overlapping actions so `allowed_actions` and `forbidden_actions` are disjoint.";
    CompatRawHtmlQuarantined = "compat.raw_html_quarantined" =>
        "Replace raw HTML with Markdown syntax, or migrate the page to .adoc for strict validation.";
    CompatUnsafeLinkDropped = "compat.unsafe_link_dropped" =>
        "Use a safe link scheme such as https, http, mailto, or a relative path; the unsafe href was dropped from the rendered HTML.";
    CompatUnsafeImageSrcDropped = "compat.unsafe_image_src_dropped" =>
        "Use a safe image scheme such as https, http, or a relative path; the unsafe src was dropped from the rendered HTML.";
    CompatUnknownExtension = "compat.unknown_extension" =>
        "Remove the unsupported Markdown construct or migrate the page to .adoc; the source text was rendered as an escaped code block.";
    ParseMalformedMarkdown = "parse.malformed_markdown" =>
        "Check that Markdown blocks (tables, lists, fenced code) are well-formed; the parser tolerated the imbalance and rendered best-effort output.";
    RetrievalNoKnowledgeObjectsConsiderMigration = "retrieval.no_knowledge_objects_consider_migration" =>
        "Migrate .md files to .adoc with typed Knowledge Objects, or wait for `adoc migrate` (V4.5+); Markdown source contributes prose blocks but no citable Knowledge Objects.";
    SchemaContradictionMissingSeverity = "schema.contradiction_missing_severity" =>
        "Add a `severity` field to the contradiction: one of low, medium, high, critical.";
    SchemaContradictionInvalidSeverity = "schema.contradiction_invalid_severity" =>
        "Use a valid contradiction severity: one of low, medium, high, critical.";
    SchemaContradictionMissingStatus = "schema.contradiction_missing_status" =>
        "Add a `status` field to the contradiction: one of unresolved, resolved, dismissed.";
    SchemaContradictionInvalidStatus = "schema.contradiction_invalid_status" =>
        "Use a valid contradiction status: one of unresolved, resolved, dismissed.";
    SchemaContradictionMissingClaims = "schema.contradiction_missing_claims" =>
        "Add a `claims` field listing at least two claim IDs (scalar or `[a.b, c.d]` list).";
    SchemaContradictionClaimsTooFew = "schema.contradiction_claims_too_few" =>
        "List at least two distinct claim IDs in `claims` using `[a.b, c.d]` list form.";
    SchemaContradictionClaimNotFound = "schema.contradiction_claim_not_found" =>
        "Ensure every claim ID in `claims` refers to an existing `claim` object in the workspace.";
    SchemaContradictionClaimNotAClaim = "schema.contradiction_claim_not_a_claim" =>
        "Every `claims` entry must reference a `claim` object; this ID resolves to a different kind.";
    SchemaSourceMissingKind = "schema.source_missing_kind" =>
        "Add a `kind` field to the source: one of source_code, test, commit, pull_request, issue, design_doc, human_review, external_url, api_schema, runtime_metric, incident, support_ticket, audit_record, policy_reference, dataset, experiment.";
    SchemaSourceInvalidKind = "schema.source_invalid_kind" =>
        "Use a valid evidence kind: one of source_code, test, commit, pull_request, issue, design_doc, human_review, external_url, api_schema, runtime_metric, incident, support_ticket, audit_record, policy_reference, dataset, experiment.";
    SchemaSourceMissingPathOrUrl = "schema.source_missing_path_or_url" =>
        "Add either a `path` (repo-relative) or `url` (absolute URL) field to the source object.";
    SchemaSourceConflictingPathAndUrl = "schema.source_conflicting_path_and_url" =>
        "Provide only one of `path` or `url` on a source object, not both.";
    SchemaSourceInvalidPath = "schema.source_invalid_path" =>
        "Use a repo-relative path (e.g. `src/main.rs`); avoid leading `/`, `..` segments, backslashes, and Windows drive letters.";
    SchemaSourceInvalidUrl = "schema.source_invalid_url" =>
        "Use a well-formed absolute URL with an allowed scheme (http, https, or mailto).";
    SchemaSourceKindTargetMismatch = "schema.source_kind_target_mismatch" =>
        "The evidence kind restricts target to path-only or url-only. Adjust the `kind`, `path`, or `url` field accordingly.";
    /// V6.5.1: `api` Knowledge Object (PRD §13.7).
    SchemaApiMissingMethodOrInterfaceType = "schema.api_missing_method_or_interface_type" =>
        "Add either a `method` (HTTP method, e.g. POST) or an `interface_type` (e.g. grpc, graphql) field to the api object.";
    SchemaApiConflictingMethodAndInterfaceType = "schema.api_conflicting_method_and_interface_type" =>
        "Provide only one of `method` or `interface_type` on an api object, not both.";
    SchemaApiInvalidMethod = "schema.api_invalid_method" =>
        "Use an uppercase HTTP method: one of GET, HEAD, POST, PUT, DELETE, CONNECT, OPTIONS, TRACE, PATCH.";
    SchemaApiMissingPathOrSymbol = "schema.api_missing_path_or_symbol" =>
        "Add either a `path` (`/`-prefixed route template) or a `symbol` (code symbol) field to the api object.";
    SchemaApiConflictingPathAndSymbol = "schema.api_conflicting_path_and_symbol" =>
        "Provide only one of `path` or `symbol` on an api object, not both.";
    SchemaApiInvalidPath = "schema.api_invalid_path" =>
        "Use a non-empty `/`-prefixed route template (e.g. `/api/billing/credits/consume`).";
    /// V6.5.1: a `verified` api has neither an inline `source_code` evidence
    /// entry nor an `evidence_ref` resolving to an `api_schema`/`source_code`
    /// source — an API contract is verified by its schema source.
    ApiVerifiedMissingSchemaEvidence = "api.verified_missing_schema_evidence" =>
        "A verified api requires schema evidence: an inline `source:` entry or an `evidence_ref` to an `api_schema`/`source_code` source object.";
    /// V6.5.2: `observation` Knowledge Object (PRD §13.9).
    SchemaObservationMissingStatus = "schema.observation_missing_status" =>
        "Add a `status` field to the observation. The only valid observation status is: observed.";
    SchemaObservationInvalidStatus = "schema.observation_invalid_status" =>
        "The only valid observation status is: observed.";
    SchemaObservationInvalidSampleSize = "schema.observation_invalid_sample_size" =>
        "Use a positive integer for `sample_size` (e.g. `37`).";
    SchemaObservationInvalidObservedAt = "schema.observation_invalid_observed_at" =>
        "Use a valid `YYYY-MM-DD` date for `observed_at`.";
    /// V6.5.3: `question` Knowledge Object (PRD §13.10).
    SchemaQuestionMissingStatus = "schema.question_missing_status" =>
        "Questions require non-empty `status`. Valid question statuses are: open, answered.";
    /// V6.5.3: an `answered` question does not name the object that answered
    /// it via `resolved_by:`.
    SchemaQuestionAnsweredMissingResolvedBy = "schema.question_answered_missing_resolved_by" =>
        "An answered question must name the knowledge that answered it: add `resolved_by: <object-id>` referencing a `claim` or `decision`.";
    /// V6.5.3: the `resolved_by:` on a question names an Object ID that does
    /// not exist anywhere in the workspace.
    SchemaQuestionResolvedByNotFound = "schema.question_resolved_by_not_found" =>
        "Ensure the `resolved_by` ID refers to an existing object in the workspace.";
    /// V6.5.3: the `resolved_by:` on a question names an Object ID that exists
    /// but is neither a `claim` nor a `decision`.
    SchemaQuestionResolvedByWrongKind = "schema.question_resolved_by_wrong_kind" =>
        "`resolved_by` must reference a `claim` or `decision` object — the knowledge that answered the question.";
    /// V6.5.3: a non-`answered` question carries a `resolved_by:` field —
    /// only answered questions name the object that answered them.
    SchemaQuestionUnexpectedResolvedBy = "schema.question_unexpected_resolved_by" =>
        "Remove `resolved_by` or set `status: answered`.";
    /// V6.5.4: `task` Knowledge Object (PRD §13.11).
    SchemaTaskMissingOwner = "schema.task_missing_owner" =>
        "Tasks require a non-empty `owner` field; a task without an owner is a wish.";
    SchemaTaskMissingStatus = "schema.task_missing_status" =>
        "Tasks require non-empty `status`. Valid task statuses are: open, done.";
    SchemaTaskInvalidStatus = "schema.task_invalid_status" =>
        "Valid task statuses are: open, done.";
    SchemaTaskInvalidDue = "schema.task_invalid_due" =>
        "Use a valid `YYYY-MM-DD` date for `due`.";
    /// V6.5.4: an `open` task's `due` date is strictly before today. WARNING
    /// severity — clock-dependent, so fixture dates use the wide-margin
    /// discipline (the `schema.policy_review_overdue` precedent).
    TaskOverdue = "task.overdue" =>
        "Complete the task and set `status: done`, or move its `due` date.";
    /// V5.8 TB2: the `evidence_ref:` on a claim names an Object ID that does
    /// not exist anywhere in the workspace.
    SchemaEvidenceTargetNotFound = "schema.evidence_target_not_found" =>
        "Ensure every `evidence_ref` ID refers to an existing `source` object in the workspace.";
    /// V5.8 TB2: the `evidence_ref:` on a claim names an Object ID that exists
    /// but is not a `source` Knowledge Object.
    SchemaEvidenceTargetNotASource = "schema.evidence_target_not_a_source" =>
        "An `evidence_ref` must point to a `source` Knowledge Object; update the ID or change the referenced object's kind.";
    /// V5.10 TB3: a `verified` claim's best inline evidence tier is Low-only.
    ///
    /// Emitted when a verified claim has at least one inline evidence entry but
    /// every inline entry maps to the `Low` evidence tier, and the claim has no
    /// `ObjectRef` evidence (which counts as ≥ Medium per ADR-0034).
    ClaimEvidenceQualityLow = "claim.evidence_quality_low" =>
        "This verified claim relies only on low-quality evidence (external URL, issue, ticket, metric, dataset, or experiment). Add a test, source-code reference, API schema, audit record, or policy reference to strengthen verification.";
    /// V5.10 TB4: a `claim` is referenced by an `unresolved` contradiction
    /// but its authored `status` is not `"contradicted"`.
    ///
    /// This is a WARNING nudge only — the authored `status` is never mutated
    /// (ADR-0026). The effective `contradicted` state is projected at graph/HTML
    /// output time without touching the authored field.
    SchemaClaimContradictedByUnresolved = "schema.claim_contradicted_by_unresolved" =>
        "This claim is referenced by an unresolved contradiction. Consider setting `status: contradicted` on the claim to make its effective state explicit. The effective_status is already projected as `contradicted` in graph and HTML output regardless of the authored status.";
    /// V6.3: a positional `adoc impacted-by` path argument is not a valid
    /// repo-relative path (absolute, escaping, or empty).
    ImpactedInvalidPath = "impacted.invalid_path" =>
        "Pass repo-relative paths as emitted by `git diff --name-only`, e.g. `crates/billing/src/refund.rs` — not absolute paths and not paths escaping the repository.";
    /// V6.3: the `--ref` base could not be resolved in this repository.
    ImpactedRefUnresolvable = "impacted.ref_unresolvable" =>
        "Use a ref resolvable in this repository, e.g. `main` or `HEAD~1`.";
    /// V6.3: git itself was unavailable or failed while deriving the
    /// changed-file set for `--ref`.
    ImpactedGitUnavailable = "impacted.git_unavailable" =>
        "Install git and run inside a git repository, or pass explicit changed paths instead of `--ref`.";
}

impl DiagnosticCode {
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
    fn search_invalid_scope_code_roundtrips_through_wire_string() {
        let value = serde_json::to_value(DiagnosticCode::SearchInvalidScope)
            .expect("diagnostic code serializes");

        assert_eq!(value, "search.invalid_scope");
        assert_eq!(
            serde_json::from_value::<DiagnosticCode>(value).expect("diagnostic code deserializes"),
            DiagnosticCode::SearchInvalidScope
        );
        assert!(
            DiagnosticCode::SearchInvalidScope
                .default_help()
                .contains("prose-only")
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
