# ADR-0066: Bind gateway-reported sensitive access to a trusted session

Status: Accepted for E6.3.T1 under the continuing E6 scope. Extends ADR-0065 and A3/A4/A8; reuses the accepted PRD and roadmap.

## Context

Managed `adoc.sensitive_access.v0` already binds native canonical versions to current Cloud authorization. Local MCP results have Object IDs and original hashes, but no managed version UUIDs; graph repository identity is deliberately nonunique. Planning reproduced E63T1-01: public local retrieval exposes an authored internal owner without classification. Current record labels alone cannot establish correct audit subjects.

## Decision

Reuse authored-field projection and material-source attribution before local indexes. Validate original input, retain authorized siblings and original hashes, and refresh references/lifecycle metadata safely. Derive typed subjects from the same filtered snapshot and actually exposed references/copied metadata. Do not scan arbitrary JSON keys, reread artifacts after evaluation, or invent prose identities.

Register gateway-only exact `adoc.sensitive_access.v1`; preserve managed v0 and its sink. Native setup derives authenticated human/auth session, registered repository and gateway session. A digest identifies the complete reported local policy; it grants no native permission. Storage explicitly labels subject facts `gateway_reported`, distinct from native managed authorization proof, client delivery or model use. Trusted startup binds one canonical served root; tool arguments cannot replace identity or policy.

Cloud remains the final sink. Record before releasing sensitive MCP bytes. Public/no-hit/denied calls emit none; direct single-user CLI constructs no recorder. T1 is the connected synchronous cut; disconnected/default pending, spool and recovery remain T2. Use existing human authentication and repository `knowledge.read`, with immutable exact-byte replay, scoped sequence and current checks after waits and insertion. No new permission or service-as-human shortcut.

Before every event attempt, replay idempotent setup as a fresh `audit_metadata` egress preflight. Return transmission metadata separately from immutable binding; carry its digest outside event bytes and recheck current native policy. This is not a lease across the network gap. Failed preflight sends no event body; subsequent policy drift refuses admission and sensitive response release. Do not cache wider permission.

Every audited MCP result adds a Sensitive text block using its highest exposed subject class. Existing KO labels remain; closed graph/signal/prose payloads remain unchanged. Prose may expose a real sensitive reference without acquiring a fake KO hash. Ordinary output stays byte-identical.

## Consequences and checks

Gateway v1 supports six retrieval commands, 1–1000 ordered distinct sensitive subjects and at most 1 MiB UTF-8; overflow refuses, never truncates. Events are clock-free; the sink assigns received-at and exact digest. Ambiguous persistence cannot silently create a new event identity.

Prove real stdio → authenticated HTTP → native recording, sink failure, caller/root isolation, fresh egress checks, replay/concurrency/expiry, accurate field/reference classification and ordinary byte compatibility. Workload attestation, T2 recovery, T3 embedding exclusion, T4 invalidation and E6.6 policy-change receipts remain separate. No merge/deploy.
