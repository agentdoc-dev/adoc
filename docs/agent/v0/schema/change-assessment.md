# AgentDoc Change Assessment Schema

`adoc.change_assessment.v0` is the experimental V9.2.1 local assessment contract emitted by `adoc assess-changes`. It binds one Git comparison, one explicit lifecycle evaluation date, the configuration effective for that comparison, and every deterministic review fact AgentDoc can establish.

The envelope separates `completeness` from `outcome`. Consumers must accept only these tuples:

- `complete/pass`, `complete/review_required`, or `complete/uncovered`;
- `partial/not_evaluated`;
- `error/invalid` or `error/not_evaluated`.

Complete outcomes are advisory data and the command exits 0. Partial and error envelopes exit 2. A failed analysis can never appear as `complete/pass`.

## Availability

Sections that could not be established carry `status: unavailable` and omit `value`. An unavailable collection is not equivalent to an available empty collection. In particular, a partial assessment retains the trustworthy head graph and head objects, marks their `changed_in_pr` value `unknown`, and leaves `knowledge_changes` unavailable.

## Paths and authority

Every changed path occurs once in a complete envelope. Classification is `excluded`, `covered`, `provisional`, or `uncovered`. Exact `impacts:` and exact source-code/test evidence paths are the only V9.2.1 linkage rules.

Authoritative kind/status pairs are closed: verified claim, accepted decision, verified API, active policy, and verified procedure. Every other match is provisional. `agent_instruction` never grants runtime authority.

`required_reviewers` contains only identities authored on Knowledge Objects. Policy changes emit a human-review proof obligation for `agentdoc.config.yaml`; repository CODEOWNERS supplies the reviewer identity.

## Privacy and determinism

`objects` and `knowledge_changes` contain metadata, hashes, source coordinates, and reasons but never Knowledge Object bodies. The contract also excludes raw diffs, timestamps, actors, GitHub metadata, prompts, and model data.

`graph_sha256` hashes the exact head graph bytes. `object_set_sha256` hashes compact `(id, content_hash)` data sorted at the assessment digest boundary. Canonical serialization failures make the assessment `error/not_evaluated`; fallback bytes are never hashed. The assessment has no self digest; a delivery adapter may hash the exact serialized envelope bytes.

The normative JSON Schema is available at `adoc://agent/v0/schema/adoc.change_assessment.v0.schema.json`.
