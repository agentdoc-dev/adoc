# V5 Design

This document is the implementation contract for V5: the Expanded Knowledge Model. It is the V0-DESIGN / V1-DESIGN / V3-DESIGN / V4-DESIGN equivalent for the next milestone — small enough to start coding, large enough that the **Knowledge Object** vocabulary expansion, schema-version bump, value-object decomposition, slice ordering, and error model are decided before any new module lands.

V5 builds directly on the V0 compiler, the V1 graph and search artifacts, the V2 patch validation surface, the V2.1/V2.2 MCP gateway, the V3 diff and review envelopes, and the V4 Markdown Compatibility Mode. It does not change the parser for `.adoc` structural grammar, the Markdown Parser, the validator dispatch by file extension, any retrieval ranking, any patch operation set, or any MCP tool surface. It adds:

- Seven new **Knowledge Object** kinds — `constraint`, `procedure`, `example`, `policy`, `agent_instruction`, `contradiction`, `source` — discovered alongside the existing **Core Object Set** (`claim`, `decision`, `warning`, `glossary`), each with its own aggregate, validation rules, HTML rendering, and graph emission.
- One new shared value object — **Severity** — extracted from `warning`'s existing private `WarningSeverity` enum into a shared `Critical | High | Medium | Low` value object reused by `constraint`, `warning`, and `contradiction`.
- One new evidence-kind value object — **Evidence Kind** — covering the PRD §15.1 set (`source_code`, `test`, `commit`, `pull_request`, `issue`, `design_doc`, `human_review`, `external_url`, `api_schema`, `runtime_metric`, `incident`, `support_ticket`, `audit_record`, `policy_reference`, `dataset`, `experiment`). Inline evidence on `claim` and `decision` accepts both legacy V0 evidence fields and references to `source` Knowledge Objects.
- A graph artifact schema bump from `adoc.graph.v2` to `adoc.graph.v3`. The bump is additive: new `kind` values and new per-kind fields; existing nodes and edges are byte-identical to V4.
- Eight new shared **Field Change** variants on `adoc.diff.v0` — `Severity`, `Scope`, `Trust`, `AllowedActions`, `ForbiddenActions`, `EffectiveAt`, `ApprovedBy`, `ContradictionClaims` — extending the V3.2 projection. The `adoc.diff.v0` envelope stays at `v0` per its tolerant-reader contract.
- New **Proof Obligation** triggers on verified-status changes for the new object types, dispatched through the existing `obligations_for_change` trigger table introduced in V3.4.
- Seven new **Validation Rule** instances under `infrastructure/validate/objects/<kind>.rs`, one per new kind. Each enforces required fields, verified-status preconditions per PRD §14.3 and §15.4, and relation-target existence. All are Strict Mode rules; Compatibility Mode is unaffected.
- A new evaluation fixture — `examples/expanded-pilot/` — and its paired CLI integration test exercising every new kind end-to-end.

The architectural choices that frame the rest of this document live in ADR-0024 (Severity is a first-class shared value object), ADR-0025 (Agent Instruction objects are authored, rendered, and retrievable — not runtime-enforced permissions), ADR-0026 (Contradiction is manually authored in V5; automated detection deferred), ADR-0027 (Source objects coexist with inline evidence; inline evidence is not deprecated), and ADR-0028 (graph artifact bumps to `adoc.graph.v3`; additive object kinds and per-kind fields).

## Goals

- Close PRD MVP must-have #4 (Core schema validation), §13.3–§13.15 (Core Block Types), §14 (Knowledge Lifecycle proof obligations), and §15 (Evidence Model) for the seven object types not yet implemented.
- Hold the V0 thesis: typed Knowledge Objects are the change unit. Each new object kind earns its place by having a complete authoring → validation → rendering → graph emission → retrieval → diff/review story before V5 ships.
- Preserve the **Strict Mode** posture: malformed structure, unknown object kinds, duplicate IDs, broken references, and invalid verified objects remain errors. New kinds inherit existing strictness.
- Preserve every existing wire envelope except the explicit `adoc.graph.v3` bump. `adoc.search.v0`, `adoc.retrieval.v0`, `adoc.patch.v0`, `adoc.patch.check.v0`, `adoc.diff.v0`, `adoc.review.v0`, `adoc.project.status.v0` all stay at their current versions.
- Preserve the existing public API. `compile_workspace` remains the single compile entry point; new V5 functionality is added behind it, not around it.
- Preserve **Compatibility Mode** invariants. V4 Markdown ingestion stays prose-only; no V5 kind is ever produced from `.md` source.
- Preserve **DDD aggregate boundaries**: each new kind is an aggregate root with its own invariants, constructible only through validated factory functions, never via public struct literals.
- Preserve **Hexagonal layering**: parsing, validation, and rendering depend on domain types; the domain layer depends on nothing concrete. New value objects (`Severity`, `EvidenceKind`, `Trust`, `Scope`, ...) live in `domain/value_objects/`; new aggregates in `domain/knowledge_object/`; new validators in `infrastructure/validate/objects/`.

## Non-Goals

- **No custom schemas in V5.** Per ROADMAP V6 guidance and PRD §33.3 (Could-Have), custom schemas come only after the core object set proves stable. The seven new V5 kinds are hard-coded just as the V0 four are.
- **No automated contradiction detection.** Per ADR-0026 and PRD §33.3, V5 surfaces only manually-authored `contradiction` objects. Pairwise scanning of verified claims for conflicting bodies is deferred. The PRD §7.6 "system creates a contradiction object" is a V6+ surface.
- **No executable example sandbox.** Per ROADMAP V5 design guidance and PRD §33.2 (Should-Have), `example` objects in V5 carry `checks` and `sandbox` *declarations*; running them is a later runtime concern. Verified executable examples require both fields present but are not executed by `adoc check` or `adoc build`.
- **No runtime agent-permission enforcement.** Per ADR-0025, `agent_instruction` objects are authored, rendered, and retrievable knowledge — never runtime ACLs. `allowed_actions` and `forbidden_actions` are declarative; the MCP gateway does not consult them when deciding whether to run a tool. A clear "not enforced at runtime" caveat is required in the HTML rendering and in `adoc://agent/v0/agent-instruction-guide`.
- **No new validation modes.** Strict Mode and Compatibility Mode are the only two modes. V5 adds no third mode and no per-kind opt-out.
- **No source object as required evidence wrapper.** Per ADR-0027, inline V0 evidence fields (`source`, `test`, `reviewed_by`) on `claim` and `decision` continue to be accepted in V5. `source` Knowledge Objects coexist; references to them are an opt-in upgrade, not a forced migration.
- **No new MCP tools.** Existing `adoc_check`, `adoc_build`, `adoc_why`, `adoc_graph`, `adoc_search`, `adoc_patch_check`, `adoc_diff`, `adoc_review`, and `adoc_project_status` all inherit the extended behavior. A `adoc_resolve_contradiction` or `adoc_list_obligations` tool is a candidate for V5.10+ if measured demand emerges.
- **No new CLI commands.** All V5 surface lands behind existing commands. A future `adoc dashboard`, `adoc audit`, `adoc owners`, or `adoc obligations` command stays deferred.
- **No graph storage change.** V5 stays on JSON sidecar (`docs.graph.json`) per ADR-0011. SQLite, embedded graph DBs, and hosted graph stores remain V6+ candidates.
- **No prose retrieval, no V1.7 inclusion.** V5 reuses the V1 Knowledge-Object-only retrieval model; the new kinds become citable Knowledge Objects, but prose blocks in `.adoc` and `.md` files remain non-retrievable. V1.7 ships independently.
- **No `thiserror` / `anyhow` dependencies.** Hand-rolled diagnostic codes match existing V0–V4 precedent.

## Workspace Layout

V5 adds modules; it moves nothing. Every new file follows the existing **DDD + hexagonal** taxonomy.

```text
crates/adoc-core/
  Cargo.toml                                  # NO new external deps; V5 is pure-Rust schema expansion.
  src/
    domain/
      value_objects/
        mod.rs                                # extended: re-export new value objects
        rel_path.rs                           # existing
        severity.rs                           # NEW (V5.1): Severity enum
        lang.rs                               # NEW (V5.3): Lang newtype + grammar
        sandbox.rs                            # NEW (V5.3): SandboxName newtype
        trust.rs                              # NEW (V5.5): Trust enum
        scope.rs                              # NEW (V5.5): Scope value object
        action.rs                             # NEW (V5.5): AllowedAction, ForbiddenAction newtypes
        action_set.rs                         # NEW (V5.5): disjoint Allowed/Forbidden invariant
        effective_date.rs                     # NEW (V5.4): EffectiveDate newtype
        approved_by.rs                        # NEW (V5.4): ApprovedBy newtype
        review_interval.rs                    # NEW (V5.4): ReviewInterval (Duration wrapper)
        evidence_kind.rs                      # NEW (V5.7+V5.8): EvidenceKind enum
        contradiction_claims.rs               # NEW (V5.6): NonEmpty<ObjectId> with arity ≥ 2
      knowledge_object/
        mod.rs                                # extended: BlockKind variants and dispatch
        claim.rs                              # existing; extended (V5.8) for typed evidence
        decision.rs                           # existing; extended (V5.8) for typed evidence
        glossary.rs                           # existing
        warning.rs                            # existing; extended (V5.1) for Severity
        constraint.rs                         # NEW (V5.1)
        procedure.rs                          # NEW (V5.2)
        example.rs                            # NEW (V5.3)
        policy.rs                             # NEW (V5.4)
        agent_instruction.rs                  # NEW (V5.5)
        contradiction.rs                      # NEW (V5.6)
        source.rs                             # NEW (V5.7)
        draft.rs                              # existing; extended: handle new kinds
        metadata.rs                           # existing; extended: kind string table
        projection.rs                         # existing; extended: new fields in retrieval record
      review/
        field_change.rs                       # existing; extended (V5.1, V5.4, V5.5, V5.6)
      obligation.rs                           # existing (V3.4); extended: kind-aware triggers
    infrastructure/
      parser/
        adoc.rs                               # existing; extended: parse new fence words
      validate/
        mod.rs                                # extended: register new rules
        objects/
          mod.rs                              # NEW (V5.1): per-kind required-field rules
          constraint_required_fields.rs       # NEW (V5.1)
          procedure_required_fields.rs        # NEW (V5.2)
          example_required_fields.rs          # NEW (V5.3)
          policy_required_fields.rs           # NEW (V5.4)
          agent_instruction_required_fields.rs # NEW (V5.5)
          contradiction_required_fields.rs    # NEW (V5.6)
          source_required_fields.rs           # NEW (V5.7)
          example_verified_executable.rs      # NEW (V5.3): checks AND sandbox when verified
          policy_active_approval.rs           # NEW (V5.4): approved_by AND effective_at when active
          agent_disjoint_actions.rs           # NEW (V5.5): allowed ∩ forbidden = ∅
          contradiction_claims_resolve.rs     # NEW (V5.6): each claim ref exists and is a claim
      render/
        html.rs                               # extended: per-kind HTML rendering
                                              #   - constraint badge with Severity color
                                              #   - procedure ordered-step list
                                              #   - example fenced code block w/ lang + checks
                                              #   - policy approval header
                                              #   - agent_instruction with "NOT enforced at runtime" banner
                                              #   - contradiction side-by-side claim links
                                              #   - source kind+path+url metadata block
    application/
      compile.rs                              # extended: dispatch new kinds through pipeline
      review.rs                               # extended: new FieldChange variants
      review_envelope.rs                      # extended: serialize new variants
      patch.rs                                # extended: new field replacement targets
    domain/graph/
      schema.rs                               # extended: bump GRAPH_SCHEMA_VERSION to "adoc.graph.v3"
      record.rs                               # extended: new per-kind fields on graph node
                                              #   (kind-discriminated payload)

crates/adoc-cli/
  tests/
    expanded_pilot.rs                         # NEW (V5.9): end-to-end V5 pilot test

crates/adoc-mcp/
  src/
    lib.rs                                    # extended (V5.5): register
                                              #   adoc://agent/v0/agent-instruction-guide
                                              # extended (V5.6):
                                              #   adoc://agent/v0/contradiction-guide

docs/
  V5-DESIGN.md                                # this document
  expanded-pilot.md                           # NEW (V5.9): pilot maintenance guide
  adr/
    0024-severity-is-a-shared-value-object.md
    0025-agent-instruction-is-authored-not-enforced.md
    0026-contradiction-is-manually-authored.md
    0027-source-coexists-with-inline-evidence.md
    0028-graph-artifact-v3-additive-bump.md
  agent/v0/
    agent-instruction-guide.md                # NEW (V5.5)
    contradiction-guide.md                    # NEW (V5.6)
    usage-contract.md                         # extended (V5.9): reference new guides
    answer-contract.md                        # extended (V5.5): describe how an agent should
                                              #   cite agent_instruction objects (read-only,
                                              #   never as an authorization grant)

examples/
  expanded-pilot/                             # NEW (V5.9): hand-curated V5 fixture
    README.adoc
    src/
      auth/
        session-no-local-storage.adoc         # constraint
        revoke-user-session.adoc              # procedure
        docs-answering-policy.adoc            # agent_instruction
      billing/
        credit-decrement-timing.adoc          # contradiction over two claims
        consume-use-case.adoc                 # source
        credits-limit-rejection.adoc          # example (executable)
      security/
        production-db-access.adoc             # policy
```

Guidance:

- `domain/value_objects/` is the home for all new typed values. No primitive `String` or `Vec<String>` fields on any new aggregate when a value object captures a real invariant.
- `domain/knowledge_object/<kind>.rs` is one file per aggregate root, mirroring the V0 `claim.rs` / `decision.rs` / `glossary.rs` / `warning.rs` precedent. Each file owns its required-field invariants in the aggregate constructor; validation rules under `infrastructure/validate/objects/` catch the cross-aggregate or schema-level checks that aren't enforced by the type system at construction time.
- `infrastructure/validate/objects/` is a new directory grouping per-kind required-field rules. The existing flat layout under `infrastructure/validate/` continues to hold cross-cutting rules (`raw_html_forbidden`, `unsafe_link_forbidden`, `knowledge_object_unique_ids`, etc.). This keeps SRP at the rule level while preventing the directory from sprawling.
- The renderer dispatches per-kind via a sealed match in `infrastructure/render/html.rs`. Every new variant of `BlockKind` produces a compile-time exhaustiveness error in renderer dispatch until all kinds are handled; this is the OCP guardrail that proves we never silently fall through.
- All new value objects MUST implement `TryFrom<&str>` (or a stricter constructor) plus `Display`. Construction is fallible at the boundary; once constructed, the type encodes the invariant and cannot be invalidated.

## Public Core API Additions

V0's single compile entry point and V1's retrieval entry points are preserved. V5 adds the following to the public `adoc-core` surface:

### New `DiagnosticCode` variants

All `Severity::Error` unless explicitly noted; they fail `adoc check` and `adoc build` per Strict Mode posture.

- `SchemaConstraintMissingSeverity`
- `SchemaConstraintInvalidSeverity`
- `SchemaProcedureMissingStatus`
- `SchemaProcedureMissingBody`
- `SchemaExampleMissingLang`
- `SchemaExampleVerifiedRequiresChecks`
- `SchemaExampleVerifiedRequiresSandbox`
- `SchemaPolicyMissingApprovedBy`
- `SchemaPolicyMissingEffectiveAt`
- `SchemaPolicyInvalidEffectiveAt`
- `SchemaAgentInstructionMissingScope`
- `SchemaAgentInstructionMissingTrust`
- `SchemaAgentInstructionInvalidTrust`
- `SchemaAgentInstructionMissingAllowedActions`
- `SchemaAgentInstructionMissingForbiddenActions`
- `SchemaAgentInstructionActionsNotDisjoint`
- `SchemaContradictionMissingSeverity`
- `SchemaContradictionMissingStatus`
- `SchemaContradictionClaimsTooFew`
- `SchemaContradictionClaimNotFound`
- `SchemaContradictionClaimNotAClaim`
- `SchemaSourceMissingKind`
- `SchemaSourceInvalidKind`
- `SchemaSourceMissingPathOrUrl`
- `SchemaSourceConflictingPathAndUrl`
- `SchemaEvidenceUnknownKind`               # V5.8
- `SchemaEvidenceTargetNotFound`            # V5.8
- `SchemaEvidenceTargetNotASource`          # V5.8

### New `BlockKind` variants

`#[non_exhaustive]`. Added to the existing `BlockKind` enum in `domain/knowledge_object/mod.rs`:

- `Constraint`
- `Procedure`
- `Example`
- `Policy`
- `AgentInstruction`
- `Contradiction`
- `Source`

### New `FieldChange` variants

`#[non_exhaustive]`. Added to the existing V3.2 enum in `domain/review/field_change.rs`:

- `Severity { before, after }`
- `Scope { before, after }`
- `Trust { before, after }`
- `AllowedActionsAdded(NonEmpty<AllowedAction>)`
- `AllowedActionsRemoved(NonEmpty<AllowedAction>)`
- `ForbiddenActionsAdded(NonEmpty<ForbiddenAction>)`
- `ForbiddenActionsRemoved(NonEmpty<ForbiddenAction>)`
- `EffectiveAt { before, after }`
- `ApprovedByAdded(NonEmpty<ApprovedBy>)`
- `ApprovedByRemoved(NonEmpty<ApprovedBy>)`
- `ContradictionClaimsAdded(NonEmpty<ObjectId>)`
- `ContradictionClaimsRemoved(NonEmpty<ObjectId>)`

### New `ProofObligation` triggers

Added to the existing V3.4 trigger table in `domain/obligation.rs`:

- `FieldChange::Severity` on a verified `constraint` → re-verify obligation.
- `FieldChange::EffectiveAt` on an `active` `policy` → re-approve obligation.
- `FieldChange::ApprovedByRemoved` on an `active` `policy` → re-approve obligation against the removed approver.
- `FieldChange::Trust` upgrade on an `agent_instruction` → security review obligation.
- `FieldChange::ForbiddenActionsRemoved` on an `agent_instruction` → security review obligation.
- Any field change on a `contradiction` with `status: unresolved` → owner re-assert obligation.

### Graph artifact schema version

The supported graph schema version (`SUPPORTED_GRAPH_SCHEMA_VERSION` in `infrastructure/artifact/graph_json.rs`) is bumped from `"adoc.graph.v2"` to `"adoc.graph.v3"`. Older artifacts are rejected by the existing graph reader with `DiagnosticCode::SchemaUnsupportedVersion` and require a rebuild. The bump is **additive only**: every V0–V4 node and edge shape is byte-identical; new fields appear only on the new kinds.

No new function exports. The new modules under `domain/value_objects/`, `domain/knowledge_object/`, and `infrastructure/validate/objects/` are `pub(crate)`. Value objects are exposed through the aggregates that own them; consumers reach them via the existing `KnowledgeObjectRecord` projection.

`compile_workspace()` continues to return the same `CompileResult` shape. `pages[]`, `knowledge_objects[]`, and `diagnostics[]` just gain new kinds and new diagnostic codes when V5-shaped source is present.

## Vocabulary

V5 extends the AgentDoc language. Each term is also added to `CONTEXT.md`.

**Expanded Object Set**: the V5 superset of the **Core Object Set**, comprising `claim`, `decision`, `warning`, `glossary` (V0) plus `constraint`, `procedure`, `example`, `policy`, `agent_instruction`, `contradiction`, `source` (V5). Every member is a **Knowledge Object** with stable identity, lifecycle, and (where the type requires it) evidence.

**Severity**: a shared value object with variants `Critical | High | Medium | Low`, used by `constraint`, `warning`, and `contradiction`. Extracted from `warning`'s existing `WarningSeverity` enum into a shared value object; the parse grammar (lowercase `low | medium | high | critical`, ASCII-trimmed) is unchanged. Construction is fallible at the parse boundary; once constructed, the value is total.

**Constraint Object**: a `Knowledge Object` representing a rule that must remain true (PRD §13.3). Required fields: `id`, `severity`, `body`. Constraints may declare `impacts:` per V3.3 source-path impact analysis. Verified constraints require an `enforced_by` evidence reference.

**Procedure Object**: a `Knowledge Object` representing an ordered sequence of steps (PRD §13.4). Required fields: `id`, `status`, `body`. Optional fields: `role_required`, `permissions_required`, `estimated_time`, `environment`, `rollback`, `risks`. The body's ordered-list structure is preserved through to HTML; the graph artifact stores body as canonical prose text.

**Example Object**: a `Knowledge Object` carrying a code, API, workflow, or usage example (PRD §13.5). Required fields: `id`, `lang` (or `format`), `body`. Verified examples additionally require both `checks` and `sandbox` declarations; V5 does not execute the checks. The PRD §33.2 Should-Have "Executable example declaration" is closed by V5.3; runtime execution is a separate later milestone.

**Policy Object**: a `Knowledge Object` representing an authoritative organizational rule (PRD §13.12). Required fields: `id`, `status`, `owner`, `approved_by`, `effective_at`, `body`. Optional: `review_interval`. Active policies require non-empty `approved_by` and a non-future `effective_at`. Policies do not support `verified` status — `active`, `proposed`, `archived`, and `revoked` are the supported lifecycle states.

**Agent Instruction Object**: a `Knowledge Object` declaring an explicit instruction targeted at AI agents (PRD §13.13). Required fields: `id`, `scope`, `trust`, `allowed_actions`, `forbidden_actions`, `body`. `allowed_actions` and `forbidden_actions` are disjoint sets of typed `AllowedAction` / `ForbiddenAction` value objects. **Per ADR-0025, agent_instruction objects are NOT runtime ACLs. They are authored, rendered, and retrievable knowledge.** The MCP gateway does not consult them. Authoring an agent_instruction does not change what `adoc_patch_check` will accept; runtime enforcement is a future permission-engine milestone.

**Contradiction Object**: a `Knowledge Object` declaring an explicit conflict between two or more existing Knowledge Objects (PRD §13.14, §7.6). Required fields: `id`, `severity`, `status`, `claims` (≥2 Object IDs that each resolve to an existing `claim`), `body`. **Per ADR-0026, V5 contradictions are manually authored.** Automated pairwise scanning is deferred. A `claim` listed in an active contradiction may carry a `status: contradicted` lifecycle state set by the author.

**Source Object**: a reusable evidence `Knowledge Object` (PRD §13.15). Required fields: `id`, `kind` (an **Evidence Kind**), one of `path` or `url`, `body`. The `body` is the prose explanation of what this source contains. Inline V0 evidence fields on `claim` and `decision` continue to accept literal paths; per ADR-0027, references to a `source` object by Object ID are an opt-in upgrade.

**Evidence Kind**: a value object enumerating the PRD §15.1 evidence types: `source_code`, `test`, `commit`, `pull_request`, `issue`, `design_doc`, `human_review`, `external_url`, `api_schema`, `runtime_metric`, `incident`, `support_ticket`, `audit_record`, `policy_reference`, `dataset`, `experiment`. Parsing is case-sensitive; unknown kinds emit `schema.evidence_unknown_kind`.

**V5 Evidence Model**: the expanded evidence vocabulary on `claim` and `decision`. The V0 fields `source`, `test`, `reviewed_by` continue to accept string values for backwards compatibility. New evidence forms accept either an inline literal (matching the V0 string shape) or an Object ID reference to a `source` Knowledge Object. The minimum-evidence-by-kind table from PRD §15.4 is encoded in the verified-status validators per object kind.

**Disjoint Action Sets**: the V5.5 invariant that an `agent_instruction` object's `allowed_actions` and `forbidden_actions` sets share no common member. Enforced in `domain/value_objects/action_set.rs` via a value-object factory that takes both sets and rejects overlap.

**V5 Expanded Pilot**: the V5.9 evaluation fixture at `examples/expanded-pilot/`. 10–15 hand-curated `.adoc` files modeled on real product docs across auth, billing, and security domains, exercising every new V5 kind, the **Severity** value object, the **V5 Evidence Model**, **Disjoint Action Sets**, and at least one **Contradiction Object** referencing two pre-existing `claim` objects. Paired end-to-end test in `crates/adoc-cli/tests/expanded_pilot.rs`. Mirrors the Billing Pilot (V1.6) and Markdown Pilot (V4.4) pattern.

**Graph Artifact V3**: the V5 graph artifact, `dist/docs.graph.json`, with schema version `adoc.graph.v3`. Additive bump from V2 — every V0–V4 node and edge shape is preserved byte-identical; new fields appear only on the seven new V5 kinds.

## Slices

Nine vertical slices, in dependency order. Each ships source/contract changes, domain logic, an adapter when needed, CLI integration, golden fixtures, schema tests, and the relevant docs together. V5.1 lands the Severity foundation alongside the first new kind so the value object is exercised by a real aggregate from day one; V5.2–V5.7 each add one kind; V5.8 expands inline evidence to the typed kind set; V5.9 caps the milestone with a realistic pilot.

### V5.1: Constraint and Severity Foundation Slice

Goal: introduce the `constraint` Knowledge Object and the shared `Severity` value object end-to-end.

Scope:

- New `domain/value_objects/severity.rs` exposing `Severity` (`Critical | High | Medium | Low`), `TryFrom<&str>`, `Display`. `#[non_exhaustive]`.
- Replace `warning`'s private `WarningSeverity` enum with the shared `Severity` value object in `domain/knowledge_object/warning.rs`. This is behavior-preserving: `WarningSeverity` was already a typed enum (not a free-form string), so warning's severity grammar, diagnostics (`SchemaMissingField` / `SchemaInvalidStatus`), and existing fixtures are unchanged.
- New `domain/knowledge_object/constraint.rs` with the `Constraint` aggregate, including the required-field invariant in the constructor (`Constraint::try_new(id, severity, body, optional fields...)`).
- Constraint's required-field invariants (`id`, `severity`, `body`) are enforced in the aggregate constructor + `build_from_parsed` and registered in the `RESOLVERS` table in `domain/services/resolve_pending_block.rs`, mirroring `warning`. (No `infrastructure/validate/objects/` rule file in V5.1 — that directory is introduced in V5.6 for the first cross-aggregate rule, contradiction reference resolution.)
- `BlockKind::Constraint` variant. Renderer dispatch updated. Graph node emission updated.
- `FieldChange::Severity { before, after }` variant added to `domain/review/field_change.rs`. Re-verify obligation triggered when a verified constraint's `Severity` changes (the trigger lands in this slice — V5.1 closes both the projection AND its dispatch into `obligations_for_change`).
- Graph artifact bumped from `adoc.graph.v2` to `adoc.graph.v3`. Stale `adoc.graph.v2` artifacts are rejected by the existing graph reader with `DiagnosticCode::SchemaUnsupportedVersion` (no new diagnostic added).
- `adoc.diff.v0` and `adoc.review.v0` tolerantly serialize the new variant; envelope version unchanged.
- Constraint may declare `impacts:` per V3.3.
- Inline parser unit tests for `Severity` parsing. Constraint aggregate unit tests for required-field invariants. Validator unit tests for each diagnostic. CLI integration test against a fixture containing one verified constraint and one constraint with an invalid severity.

Acceptance: `adoc check` over a fixture with `::constraint auth.session.no-local-storage / severity: critical / owner: platform-security / -- / Session tokens must not be stored in localStorage. / ::` exits 0; `adoc build` emits the constraint into `docs.graph.json` with `kind: "constraint"`, `severity: "critical"`, and the verbatim body. A fixture with `severity: catastrophic` exits non-zero with `schema.constraint_invalid_severity`. `adoc diff` against a base where the same constraint had `severity: high` produces a `FieldChange::Severity { before: High, after: Critical }` entry. `cargo test -p adoc-core --test billing_pilot` continues to pass (i.e. the v3 bump is invisible to V0–V4 fixtures except for the rebuild requirement).

Deferred: V5 Pilot fixture (V5.9), per-kind constraint-status lifecycle expansion.

### V5.2: Procedure Slice

Goal: introduce the `procedure` Knowledge Object with ordered-step rendering.

Scope:

- New `domain/knowledge_object/procedure.rs` with the `Procedure` aggregate. Required: `id`, `status`, `body`. Optional: `role_required`, `permissions_required`, `estimated_time`, `environment`, `rollback`, `risks`.
- New `infrastructure/validate/objects/procedure_required_fields.rs`.
- `BlockKind::Procedure` variant. Renderer renders the body's ordered-list block(s) as an HTML `<ol>` with sequential step numbers visible to the reader; non-list prose in the body renders inline. The graph artifact stores body as canonical prose text — the renderer is responsible for visual ordering.
- Verified procedure rule: `owner` and `verified_at` and at least one evidence field (matching the V0 verified-claim rule but with `human_review` accepted in place of `test`).
- Inline parser unit tests for procedure parsing. Renderer unit test for ordered-step output. CLI integration test against a fixture with one verified procedure.

Acceptance: `adoc check` over a fixture with a procedure containing four numbered steps in the body exits 0; the rendered HTML contains an `<ol>` with four `<li>` items in source order; the graph artifact records `kind: "procedure"`, the verbatim body, and the verified metadata. A procedure missing `status:` exits non-zero with `schema.procedure_missing_status`.

Deferred: rollback-on-failure semantics, dependent-procedure traversal, V5 Pilot fixture (V5.9).

### V5.3: Example Slice (Declaration-Only)

Goal: introduce the `example` Knowledge Object with `lang`, `format`, `checks`, and `sandbox` declarations.

Scope:

- New `domain/value_objects/lang.rs` (`Lang` newtype: lowercase ASCII, must match `\A[a-z][a-z0-9_+-]*\z`), `domain/value_objects/sandbox.rs` (`SandboxName` newtype with the same grammar plus `:` as a separator).
- New `domain/knowledge_object/example.rs` with the `Example` aggregate. Required: `id`, one of `lang` or `format`, `body`. Optional: `checks` (String — a command line, not parsed further), `sandbox: SandboxName`. Verified status additionally requires both `checks` AND `sandbox`.
- New `infrastructure/validate/objects/example_required_fields.rs` and `infrastructure/validate/objects/example_verified_executable.rs`.
- `BlockKind::Example` variant. Renderer emits a fenced code block in the declared `lang`, with `checks` and `sandbox` shown as metadata above the code. The renderer adds an explicit "Not executed by adoc" caveat next to the `checks` line.
- Inline parser unit tests. Validator unit tests. CLI integration test against a fixture with one verified executable example and one non-executable example.

Acceptance: `adoc check` over a fixture with `::example billing.credits.limit-rejection / lang: ts / status: verified / checks: npm run test -- credits / sandbox: node-test / -- / expect(result.error).toBe("credits.limitExceeded"); / ::` exits 0. The same example with `status: verified` but missing `sandbox:` exits non-zero with `schema.example_verified_requires_sandbox`. The graph artifact records `kind: "example"`, `lang: "ts"`, `checks: "npm run test -- credits"`, `sandbox: "node-test"`.

Deferred: sandbox execution runtime, `example` `Lang` extension to free-form formats, V5 Pilot fixture (V5.9).

### V5.4: Policy Slice

Goal: introduce the `policy` Knowledge Object with approval metadata.

Scope:

- New `domain/value_objects/approved_by.rs` (`ApprovedBy` newtype, same grammar as `Owner`), `domain/value_objects/effective_date.rs` (`EffectiveDate` wrapping the existing date value type), `domain/value_objects/review_interval.rs` (`ReviewInterval` parsing duration strings `30d`, `90d`, `1y`).
- New `domain/knowledge_object/policy.rs` with the `Policy` aggregate. Required: `id`, `status`, `owner`, `approved_by` (`NonEmpty<ApprovedBy>`), `effective_at`, `body`. Optional: `review_interval`. Supported statuses: `proposed | active | archived | revoked`. **No `verified` status on policy** — policy authority comes from approvers, not verification.
- New `infrastructure/validate/objects/policy_required_fields.rs` and `infrastructure/validate/objects/policy_active_approval.rs` (enforces active-status requires non-empty `approved_by` and `effective_at <= today`).
- `BlockKind::Policy` variant. Renderer emits an approval header block listing approvers and effective date prominently. The body renders as normal prose.
- `FieldChange::EffectiveAt`, `FieldChange::ApprovedByAdded`, `FieldChange::ApprovedByRemoved` added.
- Inline parser unit tests. Validator unit tests. CLI integration test against a fixture with one active policy and one missing `approved_by`.

Acceptance: `adoc check` over a fixture with `::policy security.production-db-access / status: active / owner: security / approved_by: security-lead / effective_at: 2026-04-01 / review_interval: 90d / -- / Production database access requires MFA and manager approval. / ::` exits 0. The same policy with `status: active` but no `approved_by:` exits non-zero with `schema.policy_missing_approved_by`. The graph artifact records `kind: "policy"`, `approved_by: ["security-lead"]`, `effective_at: "2026-04-01"`, `review_interval: "90d"`.

Deferred: review-interval drift diagnostics (V5.10+), approval-chain validation, V5 Pilot fixture (V5.9).

### V5.5: Agent Instruction Slice

Goal: introduce the `agent_instruction` Knowledge Object with disjoint action sets and an explicit "not enforced at runtime" caveat.

Scope:

- New `domain/value_objects/trust.rs` (`Trust` enum: `Informal | Team | Authoritative | Regulated | System`, per PRD §17.2).
- New `domain/value_objects/scope.rs` (`Scope` value object — initial V5 surface is a glob string like `docs/auth/*`; the grammar is held narrow on purpose to make a richer V6+ scope model painless).
- New `domain/value_objects/action.rs` exposing `AllowedAction` and `ForbiddenAction` newtypes — both wrap lowercase kebab-case strings; both are opaque to the validator. The V5 spec does not enumerate the allowed action vocabulary; that's a permission-engine milestone.
- New `domain/value_objects/action_set.rs` with `DisjointActionSets::try_new(allowed, forbidden) -> Result<Self, OverlapError>`. The constructor is the only path to a valid disjoint pair.
- New `domain/knowledge_object/agent_instruction.rs` with the `AgentInstruction` aggregate.
- New `infrastructure/validate/objects/agent_instruction_required_fields.rs` and `infrastructure/validate/objects/agent_disjoint_actions.rs`.
- `BlockKind::AgentInstruction` variant. **Renderer emits a prominent banner: "Agent Instruction. Authored knowledge, NOT runtime ACL. See [agent-instruction-guide](adoc://agent/v0/agent-instruction-guide)."** The body renders as normal prose below.
- `FieldChange::Trust`, `FieldChange::Scope`, `FieldChange::AllowedActionsAdded`, `FieldChange::AllowedActionsRemoved`, `FieldChange::ForbiddenActionsAdded`, `FieldChange::ForbiddenActionsRemoved` added.
- New Agent Guidance Resource `adoc://agent/v0/agent-instruction-guide`, backed by `docs/agent/v0/agent-instruction-guide.md`. The guide explicitly tells consuming agents that V5 `agent_instruction` objects are **read-only declarative knowledge**: cite them in answers; never treat them as a permission grant.
- Update to `docs/agent/v0/answer-contract.md` to describe how an agent should cite `agent_instruction` objects (read-only, never as an authorization signal).
- Inline parser tests; validator tests; CLI integration test; MCP test for the new guidance resource.

Acceptance: `adoc check` over a fixture with `::agent auth.docs-answering-policy / scope: docs/auth/* / trust: internal / owner: ai-platform / allowed_actions: [summarize, cite, suggest_edits] / forbidden_actions: [execute_shell, access_secrets, modify_auth_code] / -- / Prefer verified claims over draft notes when answering auth questions. / ::` exits 0. An instruction with `allowed_actions: [cite]` and `forbidden_actions: [cite]` exits non-zero with `schema.agent_instruction_actions_not_disjoint`. The graph artifact records `kind: "agent_instruction"`, the typed `trust`, and both action sets. MCP `resources/list` includes `adoc://agent/v0/agent-instruction-guide`.

Deferred: scope-matching at retrieval time, runtime action enforcement, multi-agent identity validation, V5 Pilot fixture (V5.9).

### V5.6: Contradiction Slice (Manual)

Goal: introduce the `contradiction` Knowledge Object as a manually-authored cross-reference between two or more existing `claim` objects.

Scope:

- New `domain/value_objects/contradiction_claims.rs` exposing `ContradictionClaims` (`NonEmpty<ObjectId>` with arity ≥ 2, deduplicated, sorted by Object ID).
- New `domain/knowledge_object/contradiction.rs` with the `Contradiction` aggregate. Required: `id`, `severity`, `status`, `claims`, `body`. Supported statuses: `unresolved | resolved | dismissed`.
- New `infrastructure/validate/objects/contradiction_required_fields.rs` and `infrastructure/validate/objects/contradiction_claims_resolve.rs` (the latter verifies each `claims[]` entry resolves to an existing Knowledge Object with `kind == "claim"`; missing → `schema.contradiction_claim_not_found`; wrong kind → `schema.contradiction_claim_not_a_claim`).
- `BlockKind::Contradiction` variant. Renderer emits a side-by-side or stacked block listing the conflicting claim Object IDs as links, the contradiction's severity badge (reusing V5.1's `Severity`), and the prose body explaining the conflict.
- A `claim` may carry `status: contradicted` lifecycle, authored manually. V5 does NOT automatically transition referenced claims to `contradicted`. The author is responsible.
- `FieldChange::ContradictionClaimsAdded`, `FieldChange::ContradictionClaimsRemoved` added.
- New Agent Guidance Resource `adoc://agent/v0/contradiction-guide` backed by `docs/agent/v0/contradiction-guide.md`. The guide tells consuming agents: when answering, surface any active contradiction touching a cited claim before answering definitively.
- Inline parser tests; validator tests; CLI integration test; MCP guidance-resource test.

Acceptance: `adoc check` over a fixture containing two claims `billing.credits.decrement-before-generation` and `billing.credits.decrement-after-success` plus a `::contradiction billing.credit-decrement-timing / severity: high / status: unresolved / claims: [billing.credits.decrement-before-generation, billing.credits.decrement-after-success] / owner: backend-platform / -- / Conflicting claims about credit decrement timing. / ::` exits 0. A contradiction listing only one claim exits non-zero with `schema.contradiction_claims_too_few`. A contradiction referencing a nonexistent claim exits non-zero with `schema.contradiction_claim_not_found`.

Deferred: automated contradiction detection (V6+), automatic claim status propagation, resolution workflow, V5 Pilot fixture (V5.9).

### V5.7: Source Object Slice

Goal: introduce the `source` Knowledge Object as a reusable evidence pointer.

Scope:

- New `domain/value_objects/evidence_kind.rs` exposing `EvidenceKind` enum with all PRD §15.1 variants. `TryFrom<&str>` rejects unknown strings with `SchemaEvidenceUnknownKind`.
- New `domain/knowledge_object/source.rs` with the `Source` aggregate. Required: `id`, `kind: EvidenceKind`, exactly one of `path: RelPath` or `url: Url`, `body`. Optional: `owner`, `symbol`, `commit`, `last_seen_at`, `hash`.
- New `infrastructure/validate/objects/source_required_fields.rs`. Path-XOR-URL invariant lives in the `Source` constructor; the validator catches schema-level rejection on parse if both are present (`schema.source_conflicting_path_and_url`) or both are absent (`schema.source_missing_path_or_url`).
- `BlockKind::Source` variant. Renderer emits a metadata block with the evidence kind badge, the path or URL link, and the prose body.
- Inline V0 evidence fields on `claim` and `decision` (e.g. `source: packages/auth/src/refresh-token.ts`) continue to accept literal strings. Per ADR-0027, V5.7 does NOT deprecate the inline form. The path is parsed as a literal — no resolution to a `Source` Object ID in V5.7.
- Inline parser tests; validator tests; CLI integration test for the `source` object itself (V5.8 closes the reference-from-evidence loop).

Acceptance: `adoc check` over a fixture with `::source billing.consume-use-case / kind: source_code / path: apps/backend/src/features/credits/consume.use-case.ts / owner: backend-platform / -- / Source implementation for credit consumption. / ::` exits 0. A source object with both `path:` and `url:` exits non-zero with `schema.source_conflicting_path_and_url`. The graph artifact records `kind: "source"`, the typed evidence kind, and the path.

Deferred: source-object reference resolution in inline evidence (V5.8), source-object impact analysis, V5 Pilot fixture (V5.9).

### V5.8: V5 Evidence Model Slice

Goal: expand inline evidence on `claim` and `decision` to the typed `EvidenceKind` vocabulary, with both inline string evidence and `source` object references accepted.

Scope:

- Extension of `domain/knowledge_object/claim.rs` and `domain/knowledge_object/decision.rs` to model an `Evidence` enum: `Evidence::Inline { kind: EvidenceKind, value: String }` or `Evidence::ObjectRef(ObjectId)`.
- The V0 evidence fields (`source: ...`, `test: ...`, `reviewed_by: ...`) continue to parse. Each parses into `Evidence::Inline { kind: EvidenceKind::SourceCode | Test | HumanReview, value }`. Existing V0 fixtures parse byte-identical.
- A new field syntax — `evidence_ref: <object-id>` on `claim` and `decision` — accepts an Object ID resolving to a `source` Knowledge Object. The validator checks that the target exists and is a `source`; missing → `schema.evidence_target_not_found`; mismatched kind → `schema.evidence_target_not_a_source`.
- Per PRD §15.4, the verified-status validators are upgraded to type-aware checks:
  - `claim` verified: at least one of `source_code | test | human_review | external_url` evidence.
  - `decision` verified: `human_review` or approver evidence.
  - The new typed checks replace the V0 "at least one of source/test/reviewed_by" rule but accept the same V0 fixtures through the inline string evidence form (which classifies to the same kinds).
- No new `FieldChange` variants — `EvidenceAdded` / `EvidenceRemoved` already cover the change. The `Evidence` projection in the retrieval record gains a typed-kind hint.
- Extension of `application/patch.rs` to allow `update_field` patches targeting `evidence` to accept either inline-string or object-ref evidence shape.
- Inline parser tests; validator tests; CLI integration test.

Acceptance: `adoc check` over a V0 billing-pilot fixture (using only inline evidence) exits 0 with byte-identical diagnostics to V4. A new fixture combining inline `test:` evidence with `evidence_ref: billing.consume-use-case` (resolving to a V5.7 source object) exits 0; the graph artifact records the evidence as a typed list including the object-ref entry. A claim with `evidence_ref: missing.thing` exits non-zero with `schema.evidence_target_not_found`.

Deferred: evidence-quality scoring (PRD §15.3), automated evidence freshness checks, V5 Pilot fixture (V5.9).

### V5.9: V5 Expanded Pilot Slice

Goal: prove V5 end-to-end against a realistic mixed-domain docs tree.

Scope:

- Growth of `examples/expanded-pilot/` to 10–15 `.adoc` files spread across auth, billing, and security domains, exercising every new V5 kind and the V5 evidence model. At minimum:
  - One `constraint` with `impacts:` (V5.1 + V3.3).
  - One verified `procedure` with `role_required` and `rollback`.
  - One verified executable `example` with `lang`, `checks`, `sandbox`.
  - One non-executable `example` (lang only).
  - One active `policy` with multi-approver `approved_by` and a `review_interval`.
  - One `agent_instruction` with disjoint allowed/forbidden action sets and a non-trivial scope glob.
  - One `contradiction` referencing two pre-existing `claim` objects, both of which carry `status: contradicted`.
  - Two `source` objects: one `source_code`, one `external_url`.
  - One `claim` using V5.8 evidence references to a `source` object plus an inline `test:` field.
- New `crates/adoc-cli/tests/expanded_pilot.rs` end-to-end test asserting `adoc check`, `adoc build`, `adoc why`, `adoc graph`, `adoc search`, `adoc diff`, `adoc review`, and `adoc patch --check` all behave per V5.1–V5.8 design over the pilot input. Diagnostic counts and graph node counts are exact-match assertions.
- New `docs/expanded-pilot.md` documenting the pilot's maintenance contract — analogous to `docs/v1-retrieval.md` for the Billing Pilot and `docs/markdown-pilot.md` for the Markdown Pilot.
- MCP dogfood test extension exercising the new guidance resources against the pilot.
- Update to `docs/ROADMAP.md` "Implemented" section: V5 Expanded Knowledge Model shipped, V6 composition deferred and motivated.

Acceptance: `cargo test -p adoc-cli --test expanded_pilot` exits 0 with the documented diagnostic counts. `dist/docs.html` for the pilot is hand-reviewed and visually correct (every kind renders distinctly; agent_instruction shows the runtime-not-enforced banner; contradiction shows side-by-side conflicting claim links). `adoc search "policy"` returns the policy first. `adoc graph security.production-db-access` traverses to its approvers and back.

Deferred: V6 composition (includes, nested blocks, custom schemas), V7 web app and governance, automated contradiction detection.

## Error Model

V5 follows the existing project pattern: schema-level problems become `Diagnostic` values flowing through `CompileResult`; there are no new system-level error enums.

### Diagnostics added in V5

All new diagnostic codes are `Severity::Error`. V5 schema rules fail `adoc check` and `adoc build` because the Expanded Object Set is part of the Strict Mode contract. Compatibility Mode is unaffected — `.md` source never produces V5 kinds.

| Slice | Code | Trigger |
|---|---|---|
| V5.1 | `schema.constraint_missing_severity` | Constraint block without `severity:` |
| V5.1 | `schema.constraint_invalid_severity` | Constraint with severity not in `{critical, high, medium, low}` |
| V5.1 | `schema.invalid_status` (existing, unchanged) | Warning with severity not in `{critical, high, medium, low}` — warning keeps its existing diagnostic; the Severity extraction is behavior-preserving. A future cleanup may unify warning + constraint severity codes. |
| V5.2 | `schema.procedure_missing_status` | Procedure block without `status:` |
| V5.2 | `schema.procedure_missing_body` | Procedure block without body content |
| V5.3 | `schema.example_missing_lang` | Example block without `lang:` or `format:` |
| V5.3 | `schema.example_verified_requires_checks` | Example with `status: verified` and no `checks:` |
| V5.3 | `schema.example_verified_requires_sandbox` | Example with `status: verified` and no `sandbox:` |
| V5.4 | `schema.policy_missing_approved_by` | Policy with `status: active` and empty `approved_by:` |
| V5.4 | `schema.policy_missing_effective_at` | Policy with `status: active` and no `effective_at:` |
| V5.4 | `schema.policy_invalid_effective_at` | Policy `effective_at` parses but is in the future |
| V5.5 | `schema.agent_instruction_missing_scope` | Agent instruction without `scope:` |
| V5.5 | `schema.agent_instruction_missing_trust` | Agent instruction without `trust:` |
| V5.5 | `schema.agent_instruction_invalid_trust` | Agent instruction with trust not in `{informal, team, authoritative, regulated, system}` |
| V5.5 | `schema.agent_instruction_missing_allowed_actions` | Agent instruction without `allowed_actions:` (or empty list) |
| V5.5 | `schema.agent_instruction_missing_forbidden_actions` | Agent instruction without `forbidden_actions:` (or empty list) |
| V5.5 | `schema.agent_instruction_actions_not_disjoint` | Agent instruction with `allowed ∩ forbidden ≠ ∅`, lists each overlap action |
| V5.6 | `schema.contradiction_missing_severity` | Contradiction without `severity:` |
| V5.6 | `schema.contradiction_missing_status` | Contradiction without `status:` |
| V5.6 | `schema.contradiction_claims_too_few` | Contradiction with fewer than 2 claim Object IDs |
| V5.6 | `schema.contradiction_claim_not_found` | Contradiction referencing an Object ID with no matching Knowledge Object |
| V5.6 | `schema.contradiction_claim_not_a_claim` | Contradiction referencing an Object ID of the wrong kind |
| V5.7 | `schema.source_missing_kind` | Source object without `kind:` |
| V5.7 | `schema.source_invalid_kind` | Source object with kind not in the EvidenceKind enum |
| V5.7 | `schema.source_missing_path_or_url` | Source object with neither `path:` nor `url:` |
| V5.7 | `schema.source_conflicting_path_and_url` | Source object with both `path:` and `url:` |
| V5.8 | `schema.evidence_unknown_kind` | Inline evidence field name not in the EvidenceKind enum |
| V5.8 | `schema.evidence_target_not_found` | `evidence_ref:` resolves to a missing Object ID |
| V5.8 | `schema.evidence_target_not_a_source` | `evidence_ref:` resolves to an Object ID of the wrong kind |

### Error enums

V5 adds no new error enums. All new failure modes are schema-level `Diagnostic` values. Existing patterns are preserved:

- `#[non_exhaustive]` on the `DiagnosticCode`, `BlockKind`, `FieldChange`, `Severity`, `EvidenceKind`, `Trust`, `ProofObligation` enums.
- No `unwrap`/`expect` in `domain/` or `application/` outside `#[cfg(test)]`. Existing prek hooks enforce.
- Structured fields, never string-only errors. New diagnostics carry `Span`, source path, the offending Object ID where available, and a fix-oriented message.
- Every new diagnostic variant has at least one positive test producing it.

No `thiserror` or `anyhow` dependency. Matches existing precedent.

## Schema Evolution

V5 bumps exactly one wire envelope: `adoc.graph.v2` → `adoc.graph.v3`. Every other envelope stays at its current version through the milestone.

| Envelope | V4 version | V5 version | Change |
|---|---|---|---|
| `adoc.graph` | `v2` | `v3` | Additive: new `kind` values; new per-kind fields. Old artifacts rejected with `DiagnosticCode::SchemaUnsupportedVersion`. |
| `adoc.search` | `v0` | `v0` | Unchanged. **Embedding Composition** continues to fold `{kind}: {body_plain_text}\n[id: ...] [status: ...] [owner: ...]` — new kinds simply produce new embeddings; the composition rule is unchanged. |
| `adoc.retrieval` | `v0` | `v0` | Unchanged. Tolerant readers see new fields in **Retrieval Record** projections automatically. |
| `adoc.patch` | `v0` | `v0` | Unchanged. Existing operations (`update_field`, `replace_body`, `create_draft`, `revoke`) extend cleanly to new kinds. |
| `adoc.patch.check` | `v0` | `v0` | Unchanged. New proof-obligation triggers ride inside the existing `proof_obligations[]` array. |
| `adoc.diff` | `v0` | `v0` | Unchanged. New **Field Change** variants are additive per V3.2 contract; tolerant readers see them as new variants. |
| `adoc.review` | `v0` | `v0` | Unchanged. New `impacts:`-bearing kinds fold into the existing `impact[]` array. |
| `adoc.project.status` | `v0` | `v0` | Unchanged. V5 adds counts (`knowledge_objects_by_kind`) only if measured demand emerges; otherwise the existing readiness booleans suffice. |

Agent prompts pinned to existing envelope versions stay stable through V5 except for `adoc.graph.v2` consumers. The V5.9 pilot must verify that an MCP agent pinned to a V2-shaped reading model fails gracefully (returns the existing `SchemaUnsupportedVersion` diagnostic rather than silently dropping new kinds).

## Test Pyramid

V5 follows ADR-0008 test taxonomy. Each slice ships tests at the layer where the new behavior lives.

| Layer | Test type | Coverage |
|---|---|---|
| `domain/value_objects/*` | inline `#[cfg(test)]` units | `Severity`, `Lang`, `SandboxName`, `Trust`, `Scope`, `AllowedAction`/`ForbiddenAction`, `DisjointActionSets`, `EffectiveDate`, `ApprovedBy`, `ReviewInterval`, `EvidenceKind`, `ContradictionClaims` — each tests valid construction, invalid input rejection, `Display` round-trip |
| `domain/knowledge_object/<kind>.rs` | inline units | aggregate constructor invariants for each new kind |
| `infrastructure/parser/adoc.rs` | inline units | per-fence-word parsing for each new kind |
| `infrastructure/validate/objects/*` | inline units | positive and negative cases per rule |
| `infrastructure/render/html.rs` | inline units | per-kind HTML structure (severity badge, ordered list, code fence, approval header, agent-instruction banner, side-by-side contradiction layout, source metadata block) |
| `application/compile.rs` | inline units | dispatch by `BlockKind` is exhaustive; new kinds participate in `KnowledgeObjectRecord` |
| `application/review.rs` | inline units | new `FieldChange` projection for each new variant |
| `application/patch.rs` | inline units | `update_field` operations targeting new fields |
| `domain/graph/` | inline units | schema-version bump; rejection of `adoc.graph.v2` artifacts |
| `crates/adoc-cli/tests/` | full binary spawn | V5.1–V5.8 fixture acceptance tests; V5.9 Expanded Pilot |
| `crates/adoc-mcp/tests/stdio_dogfood.rs` | extended | `adoc://agent/v0/agent-instruction-guide` and `adoc://agent/v0/contradiction-guide` resources served; existing tools' behavior over a V5-shaped project |

Slice-by-slice TDD entry test (outer-in):

| Slice | First failing test |
|---|---|
| V5.1 | CLI: `adoc check examples/v5-seed/constraint.adoc` exits 0 with one constraint emitted under `kind: "constraint"` in the graph artifact, schema version `adoc.graph.v3` |
| V5.2 | Renderer unit: a procedure body containing a four-line ordered list renders as `<ol><li>...</li></ol>` with four items in source order |
| V5.3 | Validator unit: an `example` with `status: verified` and `checks:` but no `sandbox:` emits exactly one `schema.example_verified_requires_sandbox` diagnostic |
| V5.4 | Validator unit: a `policy` with `status: active` and empty `approved_by` emits exactly one `schema.policy_missing_approved_by` diagnostic |
| V5.5 | Validator unit: an `agent_instruction` with `allowed_actions: [cite]` and `forbidden_actions: [cite]` emits exactly one `schema.agent_instruction_actions_not_disjoint` diagnostic naming `cite` as the overlap |
| V5.6 | Validator unit: a `contradiction` listing a single claim emits exactly one `schema.contradiction_claims_too_few` diagnostic |
| V5.7 | Validator unit: a `source` object with both `path:` and `url:` emits exactly one `schema.source_conflicting_path_and_url` diagnostic |
| V5.8 | Validator unit: a `claim` with `evidence_ref: missing.thing` emits exactly one `schema.evidence_target_not_found` diagnostic |
| V5.9 | CLI: `cargo test -p adoc-cli --test expanded_pilot` passes |

## Boundary Invariants

Frozen by ADRs 0024–0028 and applied to every V5 slice:

- **DDD aggregate invariants enforced at construction**: each new aggregate (`Constraint`, `Procedure`, `Example`, `Policy`, `AgentInstruction`, `Contradiction`, `Source`) exposes only fallible constructors. Public struct-literal construction is forbidden; the only path to a valid aggregate is through `try_new` (or equivalent). The aggregate cannot be invalidated after construction.
- **Hexagonal port direction**: `domain/value_objects/`, `domain/knowledge_object/`, and `domain/review/` depend on nothing concrete. New validation rules under `infrastructure/validate/objects/` depend on domain types only. New renderer dispatch under `infrastructure/render/html.rs` depends on domain types only. No new ports added; no domain type imports `pulldown_cmark`, `fastembed`, or any other infrastructure concern.
- **SRP at the rule level**: one validation rule per file. `constraint_required_fields.rs` validates the required-field set; `example_verified_executable.rs` validates the verified-status precondition; `agent_disjoint_actions.rs` validates the disjoint-set invariant. No omnibus "ConstraintRule" that mixes concerns.
- **OCP via `#[non_exhaustive]` enums**: every new public enum (`BlockKind`, `Severity`, `Trust`, `EvidenceKind`, `FieldChange`, `DiagnosticCode`, `ProofObligation`) gets `#[non_exhaustive]`. Adding a V6+ variant must not be a breaking change for downstream tolerant readers; adding a variant within V5 cannot break inline tests outside the new slice.
- **LSP for KnowledgeObject**: every member of the Expanded Object Set satisfies the shared `KnowledgeObject` contract: stable `ObjectId`, `BlockKind`, lifecycle status, optional owner, optional relations, optional `impacts:`. Retrieval, diff, patch validation, and review all treat any kind uniformly through the existing `KnowledgeObjectRecord` projection.
- **DIP through pure value objects**: new value objects (`Severity`, `Trust`, `EvidenceKind`, etc.) are pure-Rust enums or newtypes. No `dyn` dispatch is introduced anywhere in V5. Type-based dispatch via `match BlockKind` is the OCP guardrail.
- **DRY at the value-object level**: `Severity` is shared by `constraint`, `warning`, and `contradiction`. `Owner`, `Date`, `RelPath` continue to be reused. New per-kind fields use the smallest existing value object that fits before introducing a new one.
- **YAGNI**: V5 ships the seven new kinds and the V5.8 evidence model. It does NOT ship: custom schemas (V6+), automated contradiction detection (V6+), executable-example sandboxing (later runtime milestone), runtime agent-permission enforcement (later permission-engine milestone), evidence-quality scoring (V5.10+), evidence-freshness checks (V5.10+), policy review-interval drift diagnostics (V5.10+), scope-matching at retrieval time (V5.10+).
- **Reuse**: the existing `Page` AST, the existing graph artifact emitter (just bumped to v3), the existing HTML renderer pipeline, the existing retrieval session, the existing patch validator, the existing review pipeline, and all existing wire envelopes except `adoc.graph.v3` are reused without modification.
- **TDD entry test per slice**: each slice's first failing test is the outermost test that the slice has work left to do — CLI integration when behavior is user-facing, validator unit when only schema enforcement is in play. The slice is complete only when that test passes AND the slice's inner test pyramid is full per the table above.
- **Security boundary preserved**: V5 does not add HTML rendering of untrusted Markdown. `agent_instruction` bodies, `contradiction` bodies, `policy` bodies all render as escaped prose with existing V0 raw-HTML-forbidden and unsafe-link-forbidden rules. The renderer remains the security boundary; new kinds do not relax it.

## Deferred Tactical Questions

These are resolved at slice implementation time, not in this contract:

- Procedure body rendering: whether the renderer detects ordered lists inside arbitrary prose or whether procedure bodies require a top-level ordered list. Working assumption: a procedure body's first content block must be an ordered list; otherwise emit `schema.procedure_body_must_start_with_ordered_list`. Confirm in V5.2.
- Example syntax sugar: whether the body of `::example` blocks supports backtick-fenced code spans inside the body (preserving language) or requires bare code text with `lang:` as the only syntax-binding field. Working assumption: bare body text with `lang:` field is canonical; fenced inner spans are stripped. Confirm in V5.3 against the pilot fixture.
- Policy `verified` status: PRD §13.12 does not explicitly forbid `verified` policies, but the V5 schema disallows it (`active | proposed | archived | revoked` only). If the V5.4 pilot reveals real demand for `verified` policy semantics, lift to `active + verified` co-status. Default decision: stay narrow.
- Agent instruction scope grammar: V5.5 starts with a glob string. Whether to upgrade to a structured scope object (per PRD §16.2) is a V6+ question.
- Contradiction `status: dismissed`: whether a dismissed contradiction still triggers proof obligations on the referenced claims. Working assumption: dismissed contradictions emit no obligations, but the referenced claims may continue to carry `status: contradicted` set by the author. Confirm in V5.6 + V5.10.
- Source object resolution at retrieval time: whether `evidence_ref:` produces an extra reference edge in the graph artifact or a per-record projection only. Working assumption: edge in `adoc.graph.v3` with relation kind `evidence`; confirm in V5.8.
- Per-kind project-status counts: whether `adoc.project.status.v0` gains a `knowledge_objects_by_kind: { claim: N, constraint: N, ... }` field. Working assumption: not in V5; revisit at V5.10+ if MCP agents need it.
- Embedding cache invalidation across the v2→v3 graph bump: the **Search Artifact** carries `graph_artifact_hash`. The V5.1 bump changes every project's graph hash, forcing a full re-embed on first V5 build. Confirm acceptable cost in V5.1 against the existing billing-pilot fixture.
- ADR numbering: ADRs 0024–0028 are reserved here. If V5 slice implementation reveals an additional binding decision worth recording, allocate the next free number (ADR-0029+) at slice time.

## Sequencing Context

V5 closes PRD MVP must-have #4 (Core schema validation) for the seven object types not yet implemented, plus large portions of PRD §13.3–§13.15 (Core Block Types), §14.3 (Proof Obligations), §14.4 (Staleness Rules — partial; auto-detection is V5.10+), and §15 (Evidence Model). It does not close PRD §7.6 / §9.4 (automated contradiction detection); per ADR-0026 that's a V6+ milestone.

Two related milestones are factored out of V5:

- **V5.10: Lifecycle Automation** — scheduled freshness-driven status transitions (`verified` → `stale` on `expires_at`), automatic claim-status propagation on contradiction resolution, evidence-quality scoring per PRD §15.3, and policy review-interval drift diagnostics. Sequenced after V5.9 once the Expanded Knowledge Model is in real use and lifecycle pain is measured.
- **V6: Composition and Advanced Graphs** — `@include`, nested typed blocks, custom schema registry, automated contradiction detection. Sequenced after V5 because all of V6 assumes the V5 Expanded Object Set is stable as the schema target.

Both V5.10 and V6 are framed in this document's Sequencing Context as deferred milestones. `docs/ROADMAP.md` is updated alongside V5-DESIGN to reference them; the V5.10 and V6 design contracts themselves are drafted only when their slice work begins.
