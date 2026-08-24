# ADR-0059: Provider-Neutral Semantic Assessment and Materiality

- Status: Accepted
- Date: 2026-08-24
- Slice: E3.2
- Depends on: ADR-0052, ADR-0058

## Context

`adoc.semantic_review.v0` is an Action-owned advisory Claude contract
(ADR-0052). Product V1 instead requires one AgentDoc-owned contract for model
and human assessments, exact semantic-context citations, and typed materiality
that a later gate can consume without reading model prose.

The accepted roadmap requires deterministic materiality policy but does not
define its first rule. Leaving the rule inside a prompt would let provider text
become gate authority and would make the E3.2 material/immaterial fixtures
impossible to reproduce.

## Decision

1. `adoc.semantic_assessment.v0` contains schema version, exact base/head
   revisions, semantic-context digest, provider/model identity, exact assessed
   handle scope, and at least one finding. Each finding contains affected Object ID/hash
   pairs, closed classification, closed context-handle citations, typed
   materiality, proposed disposition, candidate body/field updates, unresolved
   questions, and explanatory prose. It contains no wall-clock timestamp.
2. The closed classification set remains the proven ADR-0052 set:
   `consistent`, `extends_existing_knowledge`,
   `contradicts_existing_knowledge`, and `insufficient_evidence`.
3. Materiality policy `adoc.materiality.v0` projects validated typed facts:

   | classification | materiality |
   | --- | --- |
   | `consistent` | `immaterial` |
   | `extends_existing_knowledge` | `material` |
   | `contradicts_existing_knowledge` | `material` |
   | `insufficient_evidence` | `undetermined` |

   Every finding must cite at least one exact `diff_hunk` handle from the
   validated context. The context binds that deterministic change fact; the
   classification remains the semantic executor's typed contribution. The
   projection is deterministic without pretending the semantic classification
   itself is deterministic.
4. `no_change_required` is valid only for `immaterial` findings and cannot
   coexist with candidate updates or unresolved questions. A material finding
   may propose an update/create or request human review. An undetermined
   finding cannot produce a negative verdict.
5. Explanatory prose is serialized for humans but is absent from the
   materiality projection and future gate input. Changing only prose cannot
   change materiality.
6. Provider JSON has no typed core representation until the authoritative Rust
   validator confirms exact version, context digest, revisions, identity,
   closed classification, citation membership, Object ID/hash bindings, and
   materiality projection. One failure rejects the whole artifact.
7. Human structured submissions use the identical boundary. They record
   `provider: human` plus a non-empty structured-format identity in `model`;
   authenticated principal and independence fields arrive in E3.6.
8. `adoc.semantic_review.v0` remains an advisory predecessor. Its removal or
   compatibility window belongs to E8.6 and is never implicit in this change.

## Worked fixtures

**Material.** An exact diff-hunk citation shows a durable billing branch changed;
the validated classification is `extends_existing_knowledge` and the affected
`billing.policy` Object ID/hash resolves in context. Policy v0 projects
`material`; changing the explanation does not change that result.

**Immaterial.** An exact diff-hunk citation shows a refactor; the validated
classification is `consistent`, disposition is `no_change_required`, and there
are no candidates or unresolved questions. Policy v0 projects `immaterial`.

Both fixtures are executable in
`crates/adoc-core/tests/semantic_assessment.rs`.

## Consequences

- A later gate consumes validator-owned materiality data; it never recomputes
  semantic policy or reads explanatory prose.
- A future classification or mapping change requires a version decision. It
  cannot silently change `adoc.materiality.v0` semantics.
- Provider invocation, qualification, fallback, and gate evaluation remain in
  E3.3–E5.3.
