# ADR-0025: Agent Instruction Objects Are Authored Knowledge, Not Runtime ACLs

## Status

Accepted.

## Context

V5.5 introduces the `agent_instruction` Knowledge Object — an explicit instruction targeted at AI agents (PRD §13.13), declaring a `scope`, a `trust` level, and disjoint `allowed_actions` / `forbidden_actions` sets. The V5 design contract (V5-DESIGN.md §V5.5) fixed the required fields (`id`, `scope`, `trust`, `allowed_actions`, `forbidden_actions`, `body`) and the disjointness invariant, but left several shapes to be confirmed at slice time:

1. **Runtime enforcement vs. declarative knowledge.** A type called `agent_instruction` with `allowed_actions` and `forbidden_actions` reads like an access-control list. Does the MCP Agent Gateway consult these sets when deciding whether to run a tool, and does authoring one change what `adoc_patch_check` accepts?

2. **Fence word.** Every prior kind uses its full kind name as the fence word (`::policy`, `::claim`), and the codebase enforces `BlockKind::as_str() == fence_word == graph kind`. The V5-DESIGN acceptance fixture authors the block as `::agent` and the resolver's deferred-kind help named the kind `agent`, yet the kind string is `agent_instruction` everywhere else (diagnostics `schema.agent_instruction_*`, graph `kind`, the guide URI).

3. **Trust vocabulary.** The contract defines `Trust = Informal | Team | Authoritative | Regulated | System` (PRD §17.2), but the V5-DESIGN acceptance fixture writes `trust: internal`, which is not a member.

4. **Validation location.** V5-DESIGN's file tree placed `agent_instruction_required_fields.rs` and `agent_disjoint_actions.rs` under `infrastructure/validate/objects/` — a directory that does not exist and that ADR-0030/0031 deferred.

5. **Diff payload and graph layout.** The contract sketches `FieldChange::AllowedActionsAdded(NonEmpty<AllowedAction>)` (batch), but the shipped V5.1–V5.4 machinery emits one `{ value: String }` variant per element. And `trust`/`scope`/action-sets need a home on the graph node that keeps non-agent nodes hash-stable.

## Decision

**`agent_instruction` objects are authored, rendered, and retrievable knowledge — never runtime ACLs.** The MCP Agent Gateway does not consult `allowed_actions` or `forbidden_actions` when deciding whether to run a tool; authoring or editing an `agent_instruction` does not change what `adoc_patch_check` accepts. `forbidden_actions` is not an enforcement boundary. This is made explicit to humans and agents two ways, both non-negotiable: the HTML renderer emits a prominent banner — `Agent Instruction. Authored knowledge, NOT runtime ACL. See [agent-instruction-guide](adoc://agent/v0/agent-instruction-guide).` — before the body, and the new `adoc://agent/v0/agent-instruction-guide` MCP resource (plus an `answer-contract` addendum) tells consuming agents to cite these objects as guidance, never as authorization. Runtime permission enforcement is a future permission-engine milestone.

**The fence word, kind string, and graph kind are all `agent_instruction`.** The `::agent` shorthand in the V5-DESIGN fixture is corrected to `::agent_instruction`, preserving the `as_str() == fence_word == kind` invariant; `agent` is removed from the resolver's deferred-kind help.

**`Trust` is a five-level ordered enum: `informal < team < authoritative < regulated < system`.** It is declared in ascending-authority order and derives `Ord`, so a "trust upgrade" is simply `after > before`. `trust: internal` is invalid and fails with `schema.agent_instruction_invalid_trust`; the V5-DESIGN fixture is corrected to `trust: team`.

**Validation is aggregate-owned; there is no infrastructure rule.** Required-field, format, and disjointness checks (`schema.agent_instruction_missing_scope|trust|allowed_actions|forbidden_actions`, `schema.agent_instruction_invalid_trust`, `schema.agent_instruction_actions_not_disjoint`) live in `agent_instruction.rs`'s fallible constructor, mirroring V5.1–V5.4 and keeping `infrastructure/validate/objects/` deferred. Unlike `policy`, `agent_instruction` has no clock-dependent invariant, so — unlike `PolicyActiveApproval` — it adds no `ValidationRule` at all. Disjointness is enforced through `DisjointActionSets::try_new(allowed, forbidden) -> Result<Self, OverlapError>`, the only public path to a valid pair; on overlap the aggregate emits `schema.agent_instruction_actions_not_disjoint` naming each shared action.

**Diff is per-element; `trust` rides the status slot, `scope` and action sets ride dedicated graph slots.** Six `FieldChange` variants are added — `Trust`, `Scope`, and `{Allowed,Forbidden}Actions{Added,Removed}` — each using the shipped `{ value: String }` / scalar-pair shape, not the batch sketch. `trust` is projected onto the graph node's `status` slot via the metadata discriminant (as `constraint` does with `Severity`); the diff projection therefore maps that slot's delta to `FieldChange::Trust` for `agent_instruction` nodes rather than a mislabelled `Status` change, and emits it exactly once. `scope` flows through the node `fields` map; `allowed_actions` and `forbidden_actions` get dedicated `Vec<String>` node slots (mirroring `approved_by`). On an `agent_instruction`, a `Trust` upgrade or a `ForbiddenActionsRemoved` each emits a security-review `ProofObligation`; a trust downgrade, same-level change, or `ForbiddenActionsAdded` emits none.

## Consequences

The action-set slots and the new per-kind fields are `#[serde(skip_serializing_if = "Vec::is_empty")]` in the content-hash payload, so every non-agent node keeps a byte-identical `content_hash` and the `adoc.graph.v3` envelope from V5.1 covers the new kind additively — no schema bump, and no change to `adoc.search.v0`, `adoc.retrieval.v0`, `adoc.patch.v0`, `adoc.diff.v0`, or `adoc.review.v0`.

Because validation is aggregate-owned and `agent_instruction` carries no clock-dependent rule, an `AgentInstruction` cannot be invalidated after construction: its only constructors are the fallible `build_from_parsed`/`try_new`. The `DisjointActionSets` factory likewise guarantees no valid value ever holds overlapping sets.

No runtime permission model is introduced. `scope` is a glob string with presence-only validation; the action vocabulary is opaque (the validator does not enumerate allowed actions). Scope-matching at retrieval time, a structured (non-glob) scope object, runtime action enforcement, and multi-agent identity validation are all deferred to a later permission-engine milestone (V5.10+ or beyond).
