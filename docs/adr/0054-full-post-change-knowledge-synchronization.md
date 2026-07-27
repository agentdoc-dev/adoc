# ADR-0054: Full Post-Change Knowledge Synchronization

- Status: Accepted
- Date: 2026-07-27
- Roadmap: V9.3.4

## Context

ADR-0053 deliberately limited model proposals to new, non-authoritative
objects. Pilot use showed that this leaves changed existing claims and
resolved contradictions in the report without an executable review path.
AgentDoc already supports canonical `update_fields` and `replace_body`
operations, so a second patch or transaction format is unnecessary.

## Decision

1. The Action adds opt-in `full` proposal coverage. It assigns exactly one
   create, update, no-durable-change, covered-no-change, or
   insufficient-evidence disposition to every reviewed path.
2. Existing-object candidates must target an exact-head object cited by ID and
   `content_hash`. The Action constructs canonical `update_fields` and
   `replace_body` patches with the current `base_hash`; provider-authored patch
   provenance is never trusted.
3. An existing authoritative object update defaults to a reviewable lifecycle
   (`draft`, `proposed`, or `open`, by kind). Consumers may explicitly preserve
   status or keep updates advisory.
4. Contradiction lifecycle changes remain advisory by default. Explicit
   opt-in permits only cited `resolved` or `dismissed` proposals.
5. A logical update may require two existing single-operation patches.
   The Action validates those operations sequentially in one exact-head
   sandbox and rolls the whole logical candidate back when either operation
   fails. `adoc.patch.v0` remains unchanged; no bundle schema is added.
6. Delivery is atomic by default. Explicit partial delivery may retain
   independently valid logical candidates, but every rejection is prominent
   in the source report and proposal PR.
7. Follow-up proposal pull requests are drafts and remain human-governed.

## Consequences

- Changed existing knowledge and resolved contradictions can be proposed
  without granting the model verification, approval, or merge authority.
- Full coverage is auditable even when a path needs no durable knowledge
  change.
- AgentDoc remains the sole patch parser, validator, and source editor.
- Stable Action `v1` behavior is unchanged; this ships on the immutable `v2`
  prerelease train.
