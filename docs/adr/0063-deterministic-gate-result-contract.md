# ADR-0063: Deterministic Gate Result Contract

- Status: Accepted
- Date: 2026-09-03
- Slice: E5.3
- Depends on: ADR-0051, ADR-0059, ADR-0062

## Context

Cloud must evaluate four gate modes from validated typed assessment, proposal,
approval, obligation, and authority facts. Action then publishes that
conclusion. A Cloud-private or prose-bearing result would make replay and
cross-repository contract checks unreliable.

## Decision

1. `adoc.gate_result.v0` is the shared AgentDoc-owned result contract. Cloud
   owns evaluation and audit persistence; AgentDoc owns validation and
   canonical serialization. No evaluator is added to `adoc-core`.
2. The record contains exactly a lowercase 40-hex head SHA, canonical nonblank
   policy version, sorted unique `sha256:` input digests, optional configured
   mode, derived effective mode, `pass` / `block`, and sorted unique reasons.
   It contains no timestamp or explanatory prose. Equal facts and conclusion
   therefore serialize to equal bytes. A pass in any required mode carries at
   least one input digest. Advisory passes and fail-closed blocks may carry no
   input digests because no validated evidence may exist.
3. Effective modes are closed to `advisory`, `assessment_required`,
   `proposal_required`, and `approval_required`. An absent configured mode
   derives `advisory`. A known configured mode derives itself. Configured text
   is preserved verbatim but is limited to 128 Unicode scalar values and may
   contain no C0 or C1 control character. Any other admitted present string,
   including blank or non-canonical configuration text, derives no effective
   mode and must block with only `gate.mode_unknown`; it never falls back.
4. Reasons are closed to the 12 E5.3 `gate.*` rows in the contract registry.
   Every blocking result names at least one reason; a silent block is invalid.
   Every passing result names none because each registered reason denotes a
   blocker; nonblocking advisory diagnostics belong in semantic or check
   annotations rather than this gate result. `gate.check_publish_failed`
   belongs to E5.4 publication and cannot appear in this record.
5. `agentdoc.cloud.gate_decision.v0.payload` references this shared contract.
   Consumers resolve the published AgentDoc schema rather than copying it.

## Consequences

- Wire bytes become a typed result only through `GateResult::new` or
  `validate_gate_result`; the latter rejects derived-field, ordering,
  duplicate, and unknown-member drift.
- Gate policy inputs and reason selection remain Cloud implementation details
  tested against the shared shape and closed vocabulary.
