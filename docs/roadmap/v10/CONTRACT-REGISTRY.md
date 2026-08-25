# Product V1 Contract and Wire-Code Registry

**Status:** Accepted — canonical wire inventory (E0.3)
**Date:** 2026-08-22
**Registry version:** 1
**Authority:** [`EXECUTION-MAP.md`](EXECUTION-MAP.md) §E0.3 · corrections provenance [`RED-TEAM-CLOSURE.md §RT-21`](RED-TEAM-CLOSURE.md#rt-21--contract-inventory-corrections-from-original-pr-review)
**Guards:** `crates/adoc-mcp/tests/contract_registry_guard.rs` (`adoc`) and the completeness-scan CI referencing this file from `agentdoc-dev/action` and `agentdoc-dev/cloud` (E0.3.T5)

## Registry rule

No externally observable V1 wire code or contract exists outside this registry (E0.3 exit gate). A new envelope, Diagnostic Code, event code, state vocabulary entry, retention class, or replay posture ships only together with its row here; the guards fail any repository emitting a code this file does not carry.

The executable planning surface this registry governs is `docs/roadmap/v10`; preserved historical documents (non-executable per [`EXECUTION-MAP.md`](EXECUTION-MAP.md) §1) may cite retired codes as provenance without a row here.

Field vocabularies enclosed by an envelope (statuses, enum-valued fields) are governed by that envelope's schema and registered through its row; vocabularies observable independently of a single envelope are registered explicitly in the vocabulary sections at the end of this file.

**Statuses:** `shipped` — emitted by a released surface; `planned` — reserved id with an owning E-slice (name adjustments before first implementation are registry edits at slice start); `historical` — no longer emitted, documentation retained; `removed` — never to be emitted again, carried as a disposition (see “Dispositions”).

**Version cells** name the minimum–maximum tested producer/consumer releases; a single value means minimum = maximum. **v0-additive** as a migration posture means fields may be added JSON-optionally within the version and any breaking change requires a new registered version id.

## Envelopes — shipped, owner `adoc`

Producer for every row is the `adoc` release train (CLI, MCP server, and local gateway surfaces); each row records its tested producer versions.

<!-- registry:envelopes-shipped-adoc -->
| id | status | producer (min–max tested) | consumers (min–max tested) | migration posture |
| --- | --- | --- | --- | --- |
| `adoc.change_assessment.v0` | shipped | adoc 0.3.4 | Action v2.0.0-alpha.19; Cloud ingestion planned (E4.6) | v0-additive |
| `adoc.contradictions.v0` | shipped | adoc 0.3.4 | CLI/MCP agent clients at adoc 0.3.4 | v0-additive |
| `adoc.diff.v0` | shipped | adoc 0.3.4 | CLI/MCP agent clients at adoc 0.3.4 | v0-additive |
| `adoc.executor_qualification.v0` | shipped | adoc 0.4.x | adoc 0.4.x authoritative validator and MCP schema resource; Cloud v0.1.0 store/route (E3.3) | exact-match reader; four ordered eligibility layers are protocol-valid under the currently accepted protocol version, AgentDoc-evaluated for the named capability, organization-approved for the exact requested scope/risk/deployment, and runtime-policy-eligible for the exact operation; gate authority additionally requires caller-supplied bindings from the trusted immutable store for the exact qualification ID/record digest, requested capability and approval dimensions, accepted protocol version, and current organization/runtime policy digests, plus an unchanged exact executor configuration; protocol-valid-only or stale/untrusted output is advisory; model records bind exact executor/model/config digests and every named requalification input; human records bind an authenticated principal and permission-policy digest instead of benchmark evidence |
| `adoc.graph.traversal.v0` | shipped | adoc 0.3.4 | CLI/MCP agent clients at adoc 0.3.4 | v0-additive |
| `adoc.graph.v6` | shipped | adoc 0.4.0 | adoc 0.4.0 (CLI/MCP/local gateway surfaces) | exact-match reader; v5 rejected with `schema.unsupported_version` + rebuild guidance; migration is deterministic regeneration from source (ADR-0058) |
| `adoc.impacted.v0` | shipped | adoc 0.3.4 | CLI/MCP agent clients at adoc 0.3.4 | v0-additive |
| `adoc.lifecycle_mapping.v0` | shipped | adoc 0.4.0 | adoc 0.4.0 (domain contract-tested; schema `adoc.lifecycle_mapping.v0.schema.json`); Cloud v0.1.0 (pre-release) import route — data-only consumer, contract-tested (E1.5.T3) | exact-match versions: an unknown recorded mapping/projection version is rejected with `schema.unsupported_version`; a rule change requires a version bump (the serialized version-1 contract is pinned in domain tests); historical applications replay under their recorded version; mapping alone never establishes authority and approval is never mapped to verification (KNOWLEDGE-MODEL §K5) |
| `adoc.materiality.v0` | shipped | adoc 0.4.x | enclosed by `adoc.semantic_assessment.v0`; Cloud gate consumer planned (E5.3) | exact-match typed projection policy (ADR-0059): `consistent → immaterial`, extension/contradiction → `material`, insufficient evidence → `undetermined`; input also requires an exact cited diff-hunk fact; explanatory prose is not policy input; changing the mapping requires a new registered version |
| `adoc.mcp.command.v0` | shipped | adoc 0.3.4 | MCP agent clients (contract-tested at adoc 0.3.4) | v0-additive |
| `adoc.migrate.report.v0` | shipped | adoc 0.3.4 | CLI/MCP agent clients at adoc 0.3.4 | v0-additive |
| `adoc.patch.apply.v0` | shipped | adoc 0.3.4 | CLI/MCP agent clients at adoc 0.3.4; Action v2.0.0-alpha.19 | v0-additive |
| `adoc.patch.check.v0` | shipped | adoc 0.3.4 | CLI/MCP agent clients at adoc 0.3.4; Action v2.0.0-alpha.19 | v0-additive |
| `adoc.patch.v0` | shipped | adoc 0.3.4 (validator; input authored by agents) | adoc 0.3.4; Action v2.0.0-alpha.19 | v0-additive |
| `adoc.project.status.v0` | shipped | adoc 0.3.4 | MCP agent clients (contract-tested at adoc 0.3.4) | v0-additive |
| `adoc.proof_obligation.v0` | shipped | adoc 0.4.0 | adoc 0.4.0 (domain contract-tested; schema `adoc.proof_obligation.v0.schema.json`); Cloud approval surface — data-only consumer (E1.6.T4) | v0-additive; stage-bound stateful obligation record + waiver + classification policy (KNOWLEDGE-MODEL §K8, D16): obligation states and `required_at` stages are registered closed vocabularies (see “Proof obligation states” / “Proof obligation stages”); informational-vs-blocking per stage/risk/action is classification-policy data enclosed by this envelope; a waiver binds the exact obligation + workspace-qualified managed-version subject + principal + policy version and never converts unverified to verified — an expired waiver reopens its obligation as blocking; the record object embeds neither the waiver nor the policy — both are envelope-governed `$defs` subschemas carried by the obligation ledger's event stream (their enclosing audit envelope is registered when the E1.4/E4.2 enclosure lands), so a `waived` record is interpretable only alongside its binding waiver's ordinal bound; the stateless `ProofObligation` shape embedded in `adoc.review.v0`/`adoc.patch.check.v0` is a separate, unchanged contract related by a bridge constructor |
| `adoc.repository_baseline.v0` | shipped | adoc 0.3.4 | Action v2.0.0-alpha.19 | v0-additive; known registration-gap history — the original V10 inventory flagged it unregistered; true-up obligation tracked in [`DECISION-REGISTER.md`](DECISION-REGISTER.md) |
| `adoc.retrieval.v1` | shipped | adoc 0.3.4 | CLI/MCP agent clients at adoc 0.3.4 | v1 additive; v0 is historical (see “Envelopes — historical”) |
| `adoc.review.v0` | shipped | adoc 0.3.4 | CLI/MCP agent clients at adoc 0.3.4 | v0-additive |
| `adoc.search.v2` | shipped | adoc 0.4.0 | adoc 0.4.0 | exact-match reader; v1 rejected — the bump deliberately invalidates v1 embedding caches so the Graph Artifact v6 wave forces a full re-embed (E1.1.T5, ADR-0058); wire shape unchanged from v1 |
| `adoc.semantic_assessment.v0` | shipped | adoc 0.4.x | adoc 0.4.x (authoritative domain validator and MCP schema resource); Action/Cloud adapters (E3.4); Cloud human-independence policy (E3.6) | exact-match reader with additive human-review facts; provider-neutral assessments contain at least one finding and bind exact base/head revisions plus the canonical `adoc.semantic_context.v0` digest; affected Object ID/hash pairs and every citation resolve inside the declared assessment scope and supplied context; update candidates target cited affected Objects, candidate `body` is required but nullable, and `create_knowledge` carries no candidate until a trusted creation scope exists; provider + model identity mandatory; legacy human submissions without review facts remain base-valid but establish no review authority, while authoritative human validation exact-matches reviewing/requesting Principal IDs to trusted request bindings and derives the `self_assessment | independent` fact (ADR-0060); policy eligibility remains Cloud-owned; materiality policy `adoc.materiality.v0` deterministically projects `consistent → immaterial`, extension/contradiction → `material`, and insufficient evidence → `undetermined` from typed classification plus an exact cited diff hunk (ADR-0059); `no_change_required` is immaterial-only and carries exact context/scope; explanatory prose is never gate input; JSON Schema is transport preflight only and unvalidated JSON has no typed core representation |
| `adoc.semantic_context.v0` | shipped | adoc 0.4.0 | adoc 0.4.0 (domain validator, MCP schema resource, and receipt integration) | exact-match reader; deterministic digest-bound exact revisions; callers supply trusted revision, assessment, selection algorithm/version, complete context-class definitions (ID, required/optional requirement, and byte budget), authorized-scope, and capability-policy expectations; authorized scope is a duplicate-free set, supports multiple scopes, and may be empty when none is authorized; every included closed citation's context class, scope, and canonical content digest resolves against the caller-supplied projection; the projection authenticates included citations but is not an exhaustive retrieval proof; local Graph Artifact projection accepts an empty mapping for citation-free context, otherwise requires an explicit trusted Object ID → class/scope mapping, and maps `knowledge_object` to `{"body": <body>}`, `source_binding` to the exact serialized GraphSourceBinding, and `evidence` to the exact indexed evidence entry; truncated content requires an explicitly permitted truncated digest and therefore fails closed in local receipt mode, whose graph projection permits none; managed-revision validation requires a caller-supplied managed digest/projection and fails closed when absent; diff-hunk and Source Assertion citations require the E4.1 Source Record projections and therefore fail closed in local receipt mode until E4.1; JSON Schema is preflight only and `adoc-core` owns semantic validation |
| `adoc.semantic_context_input.v0` | shipped | adoc 0.4.x | Action v2 E3.4 adapters; generic/customer-hosted semantic executors | exact-match producer input; adoc-core sorts and validates the closed context input then derives coverage, outcome, and `adoc.semantic_context.v0` digest; producers cannot supply derived authority fields |
| `adoc.semantic_executor_request.v0` | shipped | adoc 0.4.x | Action v2 Claude/Codex/generic/human adapters; customer-hosted/local executors | exact-match reader; one request shape embeds an integrity-validated ready semantic context, closed adapter and endpoint classes, 60–3600 second timeout, prompt contract, and exact executor/model/config/task/prompt digests; the prompt digest is SHA-256 over compact canonical JSON containing exactly its contract version and instructions; human uses the identical boundary with closed `human` adapter/provider/endpoint bindings plus trusted reviewing/requesting Principal IDs |
| `adoc.semantic_executor_receipt.v0` | shipped | adoc 0.4.x | Action v2 adapter orchestration; Cloud ingestion planned (E4.6) | exact-match validator-owned receipt; completed digests the validator-owned canonical assessment serialization and binds exact request/context/adapter digests, so callers cannot pair a typed assessment with different bytes; failed requires a typed failure code and cannot carry an assessment digest; no wall-clock timestamp |
| `adoc.stale.v0` | shipped | adoc 0.3.4 | CLI/MCP agent clients at adoc 0.3.4 | v0-additive |
| `adoc.validation_receipt.v0` | shipped | adoc 0.4.0 | adoc 0.4.0 (CLI receipt mode, contract-tested; schema `adoc.validation_receipt.v0.schema.json`); checksum-pinned CI harness and Cloud driver consume receipt bytes (E1.7.T2/T3) | v0-additive; digest-bound AgentDoc Validation Runtime receipt (SEMANTICS §S6): closed result vocabulary `pass` / `fail`; `diagnostics_digest` is the sha256-prefixed digest of the canonically serialized diagnostics array; deterministic — stable ordering, no wall-clock timestamps anywhere, lifecycle pinned to the explicit `evaluation_date` input; `runtime.binary_digest` is supplied by the invoking harness as an attested input (a binary cannot hash itself deterministically) after verifying the binary against its recorded pin; validator-only construction — the runtime is the only constructor path and unvalidated receipt JSON has no core representation (no `Deserialize`); the published JSON Schema is preflight/documentation only, never domain authority (ADR-0015); digest binding covers the compiled source inputs (`inputs`) and named validation context (`context`) — Evidence Anchor reads sit outside it: advisory warnings only, never result-affecting (digest-bound evidence lands with E4.1 Source Records) |
<!-- /registry:envelopes-shipped-adoc -->

## Envelopes — shipped, owner `action`

<!-- registry:envelopes-shipped-action -->
| id | status | producer (min–max tested) | consumers (min–max tested) | migration posture |
| --- | --- | --- | --- | --- |
| `adoc.pr_assessment_receipt.v0` | shipped | Action v2.0.0-alpha.19 (ADR-0051) | Action report/enforce surfaces v2.0.0-alpha.19; Cloud ingestion planned (E4.6) | v0-additive |
| `adoc.semantic_review.v0` | shipped | Action v2.0.0-alpha.19 (ADR-0052) | Action report surfaces v2.0.0-alpha.19 | v0-additive; deprecation only via the E8.6 machinery |
<!-- /registry:envelopes-shipped-action -->

## Envelopes — historical

<!-- registry:envelopes-historical -->
| id | status | notes |
| --- | --- | --- |
| `adoc.graph.v5` | historical | superseded by `adoc.graph.v6` (E1.1, ADR-0058); production emission stopped; the v5 schema stays published at `docs/agent/v0/schema/graph-artifact.v5.json` for the historical record; rejection fixtures cite it from test scope only |
| `adoc.retrieval.v0` | historical | superseded by `adoc.retrieval.v1`; the v0 schema stays published at `docs/agent/v0/schema/retrieval-envelope.v0.json` for readers of retained output |
| `adoc.search.v1` | historical | superseded by `adoc.search.v2` (E1.1.T5, ADR-0058); production emission stopped; the wire shape is unchanged — the bump exists to invalidate v1 embedding caches for the v6 full re-embed; `docs/agent/v0/schema/search-artifact.json` is updated in place to v2 (unversioned filename) |
<!-- /registry:envelopes-historical -->

## Test-fixture ids — never emitted

Deliberately invalid version fixtures cited from test modules in `crates/*/src`, proving rejected-version handling. The completeness scan splits every file at its `#[cfg(test)] mod` boundary: production literals must match the shipped table exactly, and a fixture id emitted from production scope fails the scan. Back-compat tests citing a historical id need no fixture row — the historical table already registers the id — so a fixture id must never collide with any real contract row (guard-enforced).

<!-- registry:test-fixture-ids -->
| id | status | notes |
| --- | --- | --- |
| `adoc.graph.v99` | fixture | rejected-version fixture proving the Validation Runtime's exact-match context-artifact gating (E1.7.T4): neither an older nor a newer unknown graph version is consumed |
| `adoc.search.v99` | fixture | rejected-version fixture for Search Artifact version gating |
<!-- /registry:test-fixture-ids -->

## Envelopes and contracts — planned

Reserved ids for the accepted V1 contract set (E0.3.T2). Each row names its owning repository and the E-slice that implements it; producer/consumer versions are recorded when the owning slice ships its first tested implementation. A name adjustment before first implementation is a registry edit at slice start, never an unregistered rename afterwards.

<!-- registry:envelopes-planned -->
| id | owner | planned by | notes |
| --- | --- | --- | --- |
| `adoc.connector_acl_policy.v0` | adoc | E2.6 | activation-time ACL acquisition, freshness, refresh, revocation, outage, and cache/session invalidation declaration; contract-tested schema `adoc.connector_acl_policy.v0.schema.json` |
| `adoc.source_record.v0` | adoc | E4.1 | immutable source observation |
| `adoc.source_assertion.v0` | adoc | E4.1 | source assertion bound to its Source Record |
| `adoc.source_acl_snapshot.v0` | adoc | E2.6 | immutable historical ACL provenance only; `source_acl_ceiling.snapshot_id` records the consulted snapshot while the nested `current_authorization` input in `adoc.authorization_decision.v0` independently proves freshness-bounded current access; contract-tested schema `adoc.source_acl_snapshot.v0.schema.json` |
| `adoc.source_binding.v0` | adoc | E1.1 | exact source placement binding, independent of the semantic hash; carried since E1.1.T2 as the `source_binding` member of `adoc.graph.v6` Knowledge Object nodes (schema `graph-artifact.v6.json`), governed by that envelope's version — registered as a standalone envelope when a surface emits it outside the graph artifact |
| `adoc.sensitive_access.v0` | adoc | E6.3 | name held until a final registered successor (RT-08) |
| `adoc.egress_policy.v0` | adoc | E6.6 | provenance RT-21: absent from the original V10 inventory |
| `adoc.authorization_decision.v0` | adoc | E2.2 | `allow`/`deny`/`insufficient_context` decision record; extended at E2.4 with AgentDoc group and external-binding provenance |
| `adoc.work_request.v0` | adoc | E3.7 | versioned external work request with nonce/digest/expiry/workload identity |
| `adoc.work_result.v0` | adoc | E3.7 | result binding with replay/idempotency state |
| `adoc.migration_request.v0` | adoc | E7.1 | exact-revision standalone-to-Cloud migration request |
| `adoc.migration_receipt.v0` | adoc | E7.1 | migration receipt with qualification policy outcome |
| `adoc.connector_manifest.v0` | adoc | E4.5 | capability manifest bound to exact adapter version and publisher |
| `adoc.governance_event.v0` | cloud | E4.2 | append-only governance transition record |
| `adoc.semantic_endpoint_policy.v0` | cloud | E3.4 | immutable declaration binding one generic semantic endpoint id, endpoint class, exact URL, and allowed state; Action rejects a missing or non-matching declaration before invocation; moves to shipped at Cloud's first versioned release |
| `adoc.proposal.v0` | cloud | E5.1 | canonical proposal record; includes the typed per-finding no-change disposition record (E5.3.T3) |
| `adoc.approval.v0` | cloud | E5.2 | native approval bound to exact proposal digest, principal, policy version |
| `adoc.gate_result.v0` | adoc | E5.3 | four-mode gate decision record carrying registered `gate.*` codes |
| `adoc.reconciliation_candidate.v0` | adoc | E1.2 | typed same-Object-ID collision record (ADR-0057 invariant 1, RT-03/D36): names both parties by workspace canonical identity, repository identity, latest immutable version id, and content hash; reason vocabulary closed to `object_id_collision` — hash/title/similarity never produce a candidate and never merge; the record ships in `adoc-core` since E1.2.T1 with its serialized shape pinned in domain tests; moves to shipped when a surface emits it on the wire |
| `adoc.reconciliation_decision.v0` | adoc | E1.3 | principal-bound reconciliation decision record (RT-03; MILESTONES §E1.3): closed verb set `keep_distinct` / `link_alias` / `supersede` / `merge_rehome`; binds subject and counterpart by workspace canonical identity plus exact managed version id and carries non-optional principal and policy version — a decision missing any binding is unconstructible in `adoc-core`, and recording rejects fail-closed: unknown parties, non-latest version bindings, parties that never formed a reconciliation candidate pair, and decisions conflicting with a standing merge (a merged-away party or a merge chain); no wall-clock field, so replaying the recorded decisions over the same import history yields byte-identical reconciliation state; ships in `adoc-core` since E1.3.T1 with its serialized shape pinned in domain tests; E4.2's `adoc.governance_event.v0` (cloud) will enclose it as the event payload; moves to shipped when a surface emits it on the wire |
| `adoc.managed_object_identity.v0` | cloud | E1.2 | workspace-qualified managed Object identity record, served by the Cloud object-identities route since E1.2.T3 (payload: `schema_version`, `canonical_id`, `workspace_id`, `object_id`); the canonical identity is server-minted and never derived from the human-readable Object ID, so the same unqualified Object ID in two Workspaces stays unlinkable (RT-03; MILESTONES §E1.2 stop-ship); v0-additive; moves to shipped at Cloud's first versioned release |
<!-- /registry:envelopes-planned -->

## Diagnostic Codes — shipped, owner `adoc`

Shared row values: producer `adoc` 0.4.0 (`adoc-core` `diagnostic_codes!` table, the single declaring source); consumers are every envelope embedding `Diagnostic` records (CLI/MCP surfaces at adoc 0.4.0, Action v2.0.0-alpha.19 report rendering). Migration posture for every row: wire-stable string — a meaning change or removal requires a row in “Dispositions”, never reuse.

Explicit mapping (RT-21, like the attestation family): `audit.persistence_failed` is the operation-level code the owning operation surfaces when a state transition's audit record cannot be persisted (E1.4.T4); the planned gate-level `gate.audit_persistence_failed` (E5.3, “Gate codes” below) is a distinct surface that consumes it. Both stay registered; neither is a respelling of the other.

<!-- registry:diagnostic-codes -->
| code |
| --- |
| `api.verified_missing_schema_evidence` |
| `assessment.base_partial` |
| `assessment.changed_set_failed` |
| `assessment.comparison_base_unavailable` |
| `assessment.graph_failed` |
| `assessment.head_invalid` |
| `assessment.invalid_changed_path` |
| `assessment.invalid_config_path` |
| `assessment.ref_unresolved` |
| `assessment.semantic_citation_invalid` |
| `assessment.semantic_classification_unknown` |
| `assessment.semantic_identity_missing` |
| `assessment.semantic_identity_mismatch` |
| `assessment.semantic_revision_mismatch` |
| `assessment.semantic_schema_invalid` |
| `assessment.semantic_version_unsupported` |
| `assessment.snapshot_failed` |
| `audit.persistence_failed` |
| `build.embeddings_cache_ignored` |
| `build.embeddings_cached` |
| `build.embeddings_skipped` |
| `claim.evidence_quality_low` |
| `claim.status_casing` |
| `claim.verified_missing_evidence` |
| `compat.raw_html_quarantined` |
| `compat.unknown_extension` |
| `compat.unsafe_image_src_dropped` |
| `compat.unsafe_link_dropped` |
| `embed.compute_failed` |
| `embed.model_load_failed` |
| `embed.unexpected_dim` |
| `evidence.hash_drift` |
| `evidence.hash_invalid` |
| `evidence.hash_target_missing` |
| `evidence.hash_unverifiable` |
| `governance.record_conflict` |
| `graph.object_not_found` |
| `id.duplicate` |
| `id.duplicate_in_artifact` |
| `id.invalid` |
| `impacted.git_unavailable` |
| `impacted.invalid_path` |
| `impacted.ref_unresolvable` |
| `io.artifact_malformed` |
| `io.artifact_missing` |
| `io.artifact_unreadable` |
| `io.source_path_unsafe` |
| `io.unreadable_directory` |
| `io.unreadable_file` |
| `io.unsupported_source_extension` |
| `lifecycle.expired` |
| `lifecycle.invalid_expires_at` |
| `mcp.patch_apply_disabled` |
| `migrate.broken_link` |
| `migrate.export_typed_blocks_present` |
| `migrate.raw_html_quarantined` |
| `migrate.source_not_committed` |
| `migrate.target_exists` |
| `migrate.unrecognized_extension` |
| `parse.malformed_field` |
| `parse.malformed_markdown` |
| `parse.malformed_open_fence` |
| `parse.malformed_page_annotation` |
| `parse.nested_typed_block` |
| `parse.raw_html` |
| `parse.unclosed_fence` |
| `parse.unsafe_link` |
| `patch.base_hash_mismatch` |
| `patch.create_missing_placement` |
| `patch.invalid_document` |
| `patch.placement_invalid` |
| `patch.placement_not_adoc` |
| `patch.source_binding_stale` |
| `patch.source_drift` |
| `patch.target_already_exists` |
| `patch.validation_failed` |
| `procedure.verified_missing_evidence` |
| `ref.broken` |
| `retrieval.no_knowledge_objects_consider_migration` |
| `retrieval.object_not_found` |
| `schema.agent_instruction_actions_not_disjoint` |
| `schema.agent_instruction_invalid_trust` |
| `schema.agent_instruction_missing_allowed_actions` |
| `schema.agent_instruction_missing_forbidden_actions` |
| `schema.agent_instruction_missing_scope` |
| `schema.agent_instruction_missing_trust` |
| `schema.api_conflicting_method_and_interface_type` |
| `schema.api_conflicting_path_and_symbol` |
| `schema.api_invalid_method` |
| `schema.api_invalid_path` |
| `schema.api_missing_method_or_interface_type` |
| `schema.api_missing_path_or_symbol` |
| `schema.claim_contradicted_by_unresolved` |
| `schema.constraint_invalid_severity` |
| `schema.constraint_missing_severity` |
| `schema.contradiction_claim_not_a_claim` |
| `schema.contradiction_claim_not_found` |
| `schema.contradiction_claims_too_few` |
| `schema.contradiction_invalid_severity` |
| `schema.contradiction_invalid_status` |
| `schema.contradiction_missing_claims` |
| `schema.contradiction_missing_severity` |
| `schema.contradiction_missing_status` |
| `schema.duplicate_field` |
| `schema.evidence_target_not_a_source` |
| `schema.evidence_target_not_found` |
| `schema.example_invalid_lang` |
| `schema.example_invalid_sandbox` |
| `schema.example_missing_lang` |
| `schema.example_verified_requires_checks` |
| `schema.example_verified_requires_sandbox` |
| `schema.impacts_empty` |
| `schema.impacts_invalid_path` |
| `schema.invalid_status` |
| `schema.missing_field` |
| `schema.observation_invalid_observed_at` |
| `schema.observation_invalid_sample_size` |
| `schema.observation_invalid_status` |
| `schema.observation_missing_status` |
| `schema.policy_future_effective_at` |
| `schema.policy_invalid_effective_at` |
| `schema.policy_invalid_review_interval` |
| `schema.policy_missing_approved_by` |
| `schema.policy_missing_body` |
| `schema.policy_missing_effective_at` |
| `schema.policy_missing_owner` |
| `schema.policy_missing_status` |
| `schema.policy_review_overdue` |
| `schema.procedure_body_must_start_with_ordered_list` |
| `schema.procedure_missing_body` |
| `schema.procedure_missing_status` |
| `schema.question_answered_missing_resolved_by` |
| `schema.question_missing_status` |
| `schema.question_resolved_by_not_found` |
| `schema.question_resolved_by_wrong_kind` |
| `schema.question_unexpected_resolved_by` |
| `schema.source_conflicting_path_and_url` |
| `schema.source_invalid_kind` |
| `schema.source_invalid_path` |
| `schema.source_invalid_url` |
| `schema.source_kind_target_mismatch` |
| `schema.source_missing_kind` |
| `schema.source_missing_path_or_url` |
| `schema.task_invalid_due` |
| `schema.task_invalid_status` |
| `schema.task_missing_owner` |
| `schema.task_missing_status` |
| `schema.unknown_field` |
| `schema.unknown_kind` |
| `schema.unsupported_version` |
| `schema.visibility_invalid` |
| `search.artifact_missing` |
| `search.deterministic_quality` |
| `search.hash_drift` |
| `search.invalid_filter` |
| `search.invalid_scope` |
| `search.model_mismatch` |
| `semantic_context.basis_mismatch` |
| `semantic_context.digest_mismatch` |
| `semantic_context.failed` |
| `semantic_context.insufficient_context` |
| `semantic_context.invalid_document` |
| `store.retention_floor_violation` |
| `task.overdue` |
| `validation.context_artifact_drift` |
<!-- /registry:diagnostic-codes -->

## Action codes — owner `action`

Shared row values for shipped rows: producer Action v2.0.0-alpha.19 (workflow annotations, check conclusions, receipt `reason_codes`); consumers are GitHub check/annotation readers and receipt consumers. Migration posture: wire-stable string — meaning change or removal requires a row in “Dispositions”.

<!-- registry:action-codes -->
| code | status | meaning |
| --- | --- | --- |
| `action.assessment_contract_failed` | shipped | assessment envelope violated its contract |
| `action.assessment_not_evaluated` | shipped | assessment did not run for the change set |
| `action.assessment_partial` | shipped | assessment completed with partial coverage |
| `action.assessment_ref_failed` | shipped | assessment base/head ref resolution failed |
| `action.baseline_contract_failed` | shipped | repository baseline envelope violated its contract |
| `action.baseline_not_ready` | shipped | repository baseline not yet available for this head |
| `action.baseline_unavailable` | shipped | repository baseline could not be produced |
| `action.bootstrap_dirty` | shipped | bootstrap found a dirty working tree |
| `action.install_failed` | shipped | toolchain/provider installation failed |
| `action.invalid_input` | shipped | Action inputs invalid |
| `action.knowledge_delivery_failed` | shipped | knowledge proposal delivery failed |
| `action.knowledge_proposal_incomplete` | shipped | knowledge proposal set incomplete |
| `action.knowledge_review_incomplete` | shipped | knowledge review incomplete |
| `action.knowledge_sync_pending` | shipped | knowledge synchronization still pending |
| `action.path_limit_exceeded` | shipped | changed-path limit exceeded |
| `action.proposal_failed` | shipped | proposal creation failed |
| `action.proposal_rejected` | shipped | proposal rejected by validation |
| `action.provider_integrity_failed` | shipped | provider binary integrity verification failed |
| `action.receipt_failed` | shipped | receipt finalization failed |
| `action.semantic_review_failed` | shipped | semantic review failed; the single canonical Action semantic-failure reason code (see “Dispositions”) |
| `action.structural_errors_changed` | shipped | structural errors in changed objects |
| `action.structural_errors_full` | shipped | structural errors in the full graph |
| `action.unsupported_event` | shipped | unsupported triggering event |
| `action.cloud_sync_failed` | planned (E3.7) | Cloud hand-off upload failed; local assessment preserved and annotated, never failed retroactively |
| `action.attestation_bot_rejected` | planned (E8.1) | Action check wrapper for the canonical Cloud code `attestation.bot_approver_rejected` — one documented mapping, no competing suffix |
<!-- /registry:action-codes -->

## Gate codes — planned, owner `adoc`

Contract codes for the four-mode gate evaluator (E5.3; check publication E5.4). The failure matrix is a closed 12-code set fixed red-first at E5.3 slice start; the rows here are the subset already named by the execution map and milestones — the remaining rows register as a registry edit in that slice, never ad hoc.

<!-- registry:gate-codes -->
| code | status | planned by | meaning |
| --- | --- | --- | --- |
| `gate.assessment_missing` | planned | E5.3 | `assessment_required` without a valid complete deterministic + semantic assessment |
| `gate.semantic_invalid` | planned | E5.3 | semantic assessment present but invalid/incomplete |
| `gate.proposal_missing` | planned | E5.3 | materially affected finding without a proposal or accepted no-change disposition |
| `gate.proposal_hash_mismatch` | planned | E5.3 | approval bound to a proposal digest that no longer matches |
| `gate.approval_invalidated` | planned | E5.3 | semantic content change invalidated a prior approval |
| `gate.cloud_unavailable` | planned | E5.3 | required Cloud decision input unavailable — blocks, never defaults |
| `gate.audit_persistence_failed` | planned | E5.3 | decision audit record could not be persisted — blocks; gate-level surface consuming the operation-level `audit.persistence_failed` (E1.4.T4), explicit mapping per the note above the diagnostic-codes table |
| `gate.mode_unknown` | planned | E5.3 | unknown gate mode string is a configuration error, never a fallback |
| `gate.check_publish_failed` | planned | E5.4 | required check could not publish; blocks by absence, recorded for diagnosability |
<!-- /registry:gate-codes -->

## Permission primitives — planned, owner `cloud`

The immutable version-1 permission vocabulary implemented by E2.2. Policy evaluates these primitives, never role names; changing a primitive's meaning requires a new registry version.

<!-- registry:permission-primitives -->
| permission | registry version | status | planned by |
| --- | --- | --- | --- |
| `audit.export` | 1 | planned | E2.2 |
| `audit.read` | 1 | planned | E2.2 |
| `connector.configure` | 1 | planned | E2.2 |
| `connector.create` | 1 | planned | E2.2 |
| `connector.delete` | 1 | planned | E2.2 |
| `connector.read` | 1 | planned | E2.2 |
| `knowledge.declassify` | 1 | planned | E2.2 |
| `knowledge.propose` | 1 | planned | E2.2 |
| `knowledge.read` | 1 | planned | E2.2 |
| `migration.approve` | 1 | planned | E2.2 |
| `migration.execute` | 1 | planned | E2.2 |
| `obligation.read` | 1 | planned | E2.2 |
| `obligation.satisfy` | 1 | planned | E2.2 |
| `obligation.waive` | 1 | planned | E2.2 |
| `policy.manage` | 1 | planned | E2.2 |
| `policy.read` | 1 | planned | E2.2 |
| `proposal.approve` | 1 | planned | E2.2 |
| `proposal.edit` | 1 | planned | E2.2 |
| `proposal.read` | 1 | planned | E2.2 |
| `proposal.reject` | 1 | planned | E2.2 |
| `proposal.review` | 1 | planned | E2.2 |
| `semantic_executor.configure` | 1 | planned | E2.2 |
| `semantic_executor.qualify` | 1 | planned | E2.2 |
| `semantic_executor.read` | 1 | planned | E2.2 |
| `source.manage` | 1 | planned | E2.2 |
| `source.read` | 1 | planned | E2.2 |
| `source.sync` | 1 | planned | E2.2 |
| `workspace.configure` | 1 | planned | E2.2 |
| `workspace.manage_members` | 1 | planned | E2.2 |
| `workspace.read` | 1 | planned | E2.2 |
<!-- /registry:permission-primitives -->

## External group binding modes — planned, owner `cloud`

The complete E2.4 external-binding state vocabulary from `AUTHORIZATION.md` §A7. Only rows marked `yes` can confer a grant and therefore appear in authorization-decision provenance.

<!-- registry:group-binding-modes -->
| mode | status | planned by | confers grant | meaning |
| --- | --- | --- | --- | --- |
| `authoritative_sync` | planned | E2.4 | yes | external membership is authoritative for the binding epoch |
| `additive_sync` | planned | E2.4 | yes | external membership adds to manual membership for the binding epoch |
| `suggestion_only` | planned | E2.4 | no | external membership is advisory and never grants authorization |
| `disabled` | planned | E2.4 | no | the binding is inactive and never grants authorization |
<!-- /registry:group-binding-modes -->

## External group source kinds — planned, owner `cloud`

The closed E2.4 `source_kind` vocabulary carried by an external AgentDoc-group membership in authorization provenance. The sibling `membership_source` discriminator (`manual` / `external`) is structural and governed by the `adoc.authorization_decision.v0` row. `MILESTONES.md` §E2.4 names the OIDC/SCIM category; the contract records its two protocol-specific values separately. `scim_group` is registered for provenance while SCIM sync remains deferred to P4, and a future enterprise-directory adapter requires a new registered value.

<!-- registry:group-source-kinds -->
| source kind | status | planned by | meaning |
| --- | --- | --- | --- |
| `github_team` | planned | E2.4 | GitHub team membership |
| `gitlab_group` | planned | E2.4 | GitLab group membership |
| `slack_user_group` | planned | E2.4 | Slack user-group membership |
| `oidc_group` | planned | E2.4 | the source kind is claim-only in V1 and valid only for a human principal; group claim comes from a freshly issued and verified ID token, refreshes per principal at authentication with no out-of-band lookup or sweep, retains token issuance, validation/ingestion-commit, and session-expiry instants, requires the decision principal to identify that exact session, and confers no later than the cited identity session's expiry |
| `scim_group` | planned | E2.4 | SCIM group membership |
<!-- /registry:group-source-kinds -->

## Group membership-unavailability states — planned, owner `cloud`

The closed E2.4 state vocabulary retained when an authorization decision cannot establish a potentially grant-conferring membership input. Each state identifies an immutable record through `state_record_id`; source compatibility is schema-enforced.

<!-- registry:group-membership-unavailability-states -->
| state | status | planned by | compatible input | meaning |
| --- | --- | --- | --- | --- |
| `lifecycle_unavailable` | planned | E2.4 | manual membership | the manual-membership lifecycle read failed or remained unresolved |
| `observation_expired` | planned | E2.4 | connector or OIDC external membership | the retained positive observation reached its effective freshness or identity-session deadline; OIDC evidence cites that observation's exact historical identity session on the unavailable entry |
| `connector_read_failed` | planned | E2.4 | connector-read external membership | the retained current-state connector read failed |
| `link_read_pending` | planned | E2.4 | connector-read external membership | the retained post-link or post-relink binding read is pending |
| `link_read_failed` | planned | E2.4 | connector-read external membership | the retained post-link or post-relink binding read failed |
| `epoch_observation_pending` | planned | E2.4 | connector-read external membership | a requested grant-conferring transition is awaiting its sweep while the prior effective mode epoch, including `suggestion_only` or `disabled`, remains in force |
| `oidc_authentication_pending` | planned | E2.4 | claim-only OIDC external membership | a validated grant-conferring OIDC epoch is awaiting the principal's next authentication; the unavailable entry cites no session of its own |
<!-- /registry:group-membership-unavailability-states -->

## Cloud codes — owner `cloud`

Operation labels and typed failure codes owned by the private Cloud service. New Cloud wire codes register here before they ship: `workspace.bootstrap` names the identity-bootstrap operation, the E2.1 `workspace.*` failures cover repository registration and tenant isolation, and `governance.decision_binding_missing` belongs to the E1.3 reconciliation-decision route.

<!-- registry:cloud-codes -->
| code | status | meaning |
| --- | --- | --- |
| `workspace.bootstrap` | planned (in-flight scaffold work) | identity-bootstrap ledger operation label recorded when a workspace is created |
| `workspace.repository_limit_reached` | planned (E2.1) | repository registration would exceed the Workspace's repository limit |
| `workspace.duplicate_repository` | planned (E2.1) | the same external repository is already registered in the Workspace |
| `workspace.cross_tenant_denied` | planned (E2.1) | a Workspace-scoped operation is outside the authenticated principal's memberships; a nonexistent target emits this same code, so foreign and nonexistent targets remain indistinguishable to the caller |
| `governance.decision_binding_missing` | planned (E1.3, in-flight cloud cut) | Cloud reconciliation-decision route rejects a record whose subject/counterpart exact version binding or policy version is missing or padded (deny-by-default; MILESTONES §E1.3.T4); the principal binding is never client-supplied — the store binds the authenticated session, and absent authority context maps to the `insufficient_context` outcome value, envelope-governed vocabulary rather than a standalone code; E1.4 widens Cloud's contract-scan grep to the `governance.` family |
<!-- /registry:cloud-codes -->

## Attestation codes — planned, owner `cloud`

The canonical bot-attestation code family root (RT-21, E0.3.T3). The Action never mints its own bot-attestation family: its check surface wraps this code via the registered `action.attestation_bot_rejected` row above. The E8.1 attestation record contract and the sibling codes (`attestation.binding_mismatch`, `attestation.requirements_unmet`) register at E8.1.T1 as a registry edit, flipping this row from planned to implemented rather than re-registering it.

<!-- registry:attestation-codes -->
| code | status | planned by | meaning |
| --- | --- | --- | --- |
| `attestation.bot_approver_rejected` | planned | E8.1 | bot/service approver rejected by default for approval attestation |
<!-- /registry:attestation-codes -->

## Dispositions

Codes resolved out of existence (RT-21). A disposition is permanent: the id is never reused with another meaning.

<!-- registry:dispositions -->
| code | disposition |
| --- | --- |
| `action.semantic_failed` | removed — appeared only in pre-V10 planning text and never shipped (no occurrence in Action v2.0.0-alpha.19 sources); the shipped registered code `action.semantic_review_failed` is the single canonical Action semantic-failure reason code, and any gate-matrix or planning reference resolves there |
<!-- /registry:dispositions -->

## Untrusted-change states — owner `adoc` contracts, produced by `action`/`cloud`

The closed S8 state vocabulary for the base-controlled trusted workflow ([`SEMANTICS.md §S8`](SEMANTICS.md#s8-base-controlled-trusted-workflow-for-untrusted-changes), E3.8). A new state is a registry edit plus an S8 amendment, never an ad hoc string.

<!-- registry:untrusted-change-states -->
| state |
| --- |
| `not_required` |
| `awaiting_authorization` |
| `authorized` |
| `running` |
| `completed` |
| `denied` |
| `failed` |
| `expired_after_head_change` |
<!-- /registry:untrusted-change-states -->

## Source retention classes — owner `adoc` contracts, enforced by `cloud`

The closed K9 retention-class vocabulary ([`KNOWLEDGE-MODEL.md §K9`](KNOWLEDGE-MODEL.md#k9-policy-driven-layered-source-retention), E6.6). Full source mirroring stays exceptional and disabled by default.

<!-- registry:retention-classes -->
| class |
| --- |
| `digest_only` |
| `bounded_evidence` |
| `exact_candidate_input` |
| `temporary_processing` |
| `full_source_snapshot` |
<!-- /registry:retention-classes -->

## Replay postures — owner `adoc` contracts, recorded by `cloud`

The closed K9 replay-posture vocabulary. A digest-only record is never `fully_replayable`; deleting retained evidence appends a deletion/tombstone event and updates the posture without rewriting governance history.

<!-- registry:replay-postures -->
| posture |
| --- |
| `fully_replayable` |
| `source_access_required` |
| `intentionally_non_replayable` |
| `no_longer_replayable_after_deletion` |
<!-- /registry:replay-postures -->

## Managed state dimensions — owner `adoc` contracts, recorded by `cloud`

The closed six-dimension managed state vocabularies ([`KNOWLEDGE-MODEL.md §K4`](KNOWLEDGE-MODEL.md#k4-governance-effectivity-and-synchronization-are-separate), E1.4). Entries are dimension-qualified (`dimension.state`) so the six vocabularies stay separate in one table — dimensions are never conflated (D07/D15), and a value's spelling is scoped to its own dimension. Synchronization is always per connector, and every synchronization event also carries the boolean `required_before_effective` (§K4). A new value is a registry edit plus a §K4 amendment, never an ad hoc string.

<!-- registry:managed-state-dimensions -->
| dimension.state |
| --- |
| `governance.proposed` |
| `governance.approved` |
| `governance.rejected` |
| `governance.revoked` |
| `verification.unverified` |
| `verification.partially_verified` |
| `verification.verified` |
| `verification.failed` |
| `effectivity.pending` |
| `effectivity.scheduled` |
| `effectivity.effective` |
| `effectivity.suspended` |
| `effectivity.expired` |
| `freshness.current` |
| `freshness.needs_review` |
| `freshness.stale` |
| `integrity.clear` |
| `integrity.potentially_conflicting` |
| `integrity.contradicted` |
| `synchronization.in_sync` |
| `synchronization.pending_writeback` |
| `synchronization.pending_external_approval` |
| `synchronization.writeback_failed` |
| `synchronization.source_ahead` |
| `synchronization.source_diverged` |
| `synchronization.paused` |
| `synchronization.not_applicable` |
<!-- /registry:managed-state-dimensions -->

## Proof obligation states — owner `adoc` contracts

The closed K8 obligation-state vocabulary ([`KNOWLEDGE-MODEL.md §K8`](KNOWLEDGE-MODEL.md#k8-stage-aware-proof-obligations), E1.6), carried by `adoc.proof_obligation.v0`. `waived` is reachable only through an Obligation Waiver record — exact obligation/managed-version-subject/principal/policy bound, justified, and time-bounded where appropriate; a waiver never converts unverified to verified, and an expired waiver reopens its obligation as blocking. A new state is a registry edit plus a §K8 amendment, never an ad hoc string.

<!-- registry:proof-obligation-states -->
| state |
| --- |
| `open` |
| `satisfied` |
| `waived` |
| `failed` |
| `expired` |
<!-- /registry:proof-obligation-states -->

## Proof obligation stages — owner `adoc` contracts

The closed K8 `required_at` stage vocabulary. Whether an obligation is informational or blocking at a stage/risk/action is classification-policy data enclosed by `adoc.proof_obligation.v0`, never code; `approval_required` blocks only obligations explicitly required before gate passage (§K8) — other obligations may block verification, effectivity, synchronization, or high-risk actions instead. A new stage is a registry edit plus a §K8 amendment, never an ad hoc string.

<!-- registry:proof-obligation-stages -->
| stage |
| --- |
| `proposal_validation` |
| `approval` |
| `verification` |
| `effectivity` |
| `connector_synchronization` |
| `agent_action` |
<!-- /registry:proof-obligation-stages -->
