# ADR-0061: Replay-Safe External Work Contracts

- Status: Accepted
- Date: 2026-08-25
- Slice: E3.7
- Depends on: ADR-0056

## Context

`source_ci` and `customer_worker` execute outside AgentDoc Cloud. Their output
must not be reusable across a request, Workspace, repository, or revision.

## Decision

1. `adoc.work_request.v0` binds request ID and nonce, Workspace, repository and
   source identity, exact revision/change request, ASCII-only contract/capability
   requirements sorted in ascending ASCII order, expiry, and the authorized workload Principal, subject, and
   audience. `request_digest` is SHA-256 over the canonical payload excluding
   that digest field. Expiry uses canonical UTC whole-second text so Rust,
   TypeScript, and source-CI producers hash identical bytes.
2. `adoc.work_result.v0` repeats the request ID/digest, Workspace, repository,
   revision, and workload identity, then binds runtime name/version, a distinct
   completion nonce, named output digests, and `result_digest` over the
   canonical payload excluding that digest field. Output names use lower snake
   case (`^[a-z][a-z0-9_]*$`), are unique on the wire, and serialize in ascending
   ASCII order. This excludes JavaScript integer-index ordering
   and gives Rust and TypeScript one digest representation.
3. The authoritative validator rejects unknown versions, digest mismatch, and
   any request/Workspace/repository/revision/workload substitution. Unknown
   versions carry explicit regeneration remediation.
4. Expiry, signature verification, and nonce consumption depend on current
   Cloud state and remain Cloud verifier responsibilities. Signature bytes and
   upload credentials are transport data outside both hashed envelopes.

## Consequences

All processing modes share one result contract. Cloud can reject replay before
ingestion, and Action upload failure cannot mutate the already-produced local
assessment.
