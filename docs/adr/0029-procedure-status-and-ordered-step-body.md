# ADR-0029: Procedure Status Is a Closed Enum and Its Body Must Start With an Ordered List

## Status

Accepted.

## Context

V5.2 introduces the `procedure` Knowledge Object — a runbook of ordered steps. The V5 design contract (V5-DESIGN.md) left two shapes open, to be confirmed at slice implementation time:

1. **`status` modelling.** Procedures carry a required `status`. The codebase already has two precedents: `claim` uses a free-form `ClaimStatus(String)` where only the literal `"verified"` is special, and `decision` uses a closed `DecisionStatus { Proposed, Accepted }` enum that rejects unknown spellings. The contract specified a verified-procedure rule (`owner` + `verified_at` + ≥1 evidence) but did not enumerate the valid statuses.

2. **Body structure.** The contract's working assumption was that "a procedure body's first content block must be an ordered list; otherwise emit `schema.procedure_body_must_start_with_ordered_list`," explicitly flagged "Confirm in V5.2." The alternative was lenient rendering: render whatever ordered-list lines exist and let a body with no list render as a paragraph.

A third, smaller question was the evidence vocabulary for a `verified` procedure: the contract said "matching the verified-claim rule but with `human_review` accepted in place of `test`."

## Decision

**`ProcedureStatus` is a closed enum `Draft | Verified | Deprecated`.** It mirrors `DecisionStatus`: ASCII-trimmed, lowercase-exact match, empty rejected with `schema.procedure_missing_status`, unknown spellings rejected with the existing `schema.invalid_status`. `Verified` gates the verification block exactly as claim's literal `"verified"` does, but the closed enum means a typo like `varified` fails loudly instead of silently becoming an unverified procedure. The contract does not enumerate procedure statuses, so this slice fixes the set at the minimal lifecycle a runbook needs — authored (`draft`), validated (`verified`), and superseded (`deprecated`); the set can grow later without a breaking change to authored documents because adding an enum variant only widens what is accepted.

**A procedure body must begin with an ordered list (the strict working assumption).** The aggregate constructor checks the body's first non-blank line against an ordered-list marker (`1. `, `12. `) and rejects anything else with `schema.procedure_body_must_start_with_ordered_list`. This guarantees every accepted procedure renders as numbered steps, so the "procedure" kind always carries the structure that distinguishes it from a `claim` or `glossary` paragraph.

**Rendering is the renderer's responsibility; the graph stays flat prose.** Typed-block bodies are a flat `Vec<InlineSegment>` with lines separated by newline text segments — list structure does not survive parsing into the body. Rather than thread a structured body type through the parser, domain, and graph (a much larger change, and the graph contract wants "body as canonical prose text"), the HTML renderer splits the body into lines, detects ordered-list markers, and emits one `<ol><li>` per step, preserving inline formatting within each step. The `ordered_step_marker_len` helper that recognises a marker lives on the procedure aggregate and is reused by the renderer, mirroring the page parser's `parse_ordered_list_item` idiom.

**Verified procedures accept a new shared `human_review` evidence field.** The shared `Evidence` value object (in `claim`) gains a `HumanReview` variant; a `verified` procedure accepts `owner` + `verified_at` + ≥1 of `source`, `human_review`, `reviewed_by`. Claim's own accepted evidence set (`source`, `test`, `reviewed_by`) is unchanged — reusing the shared `Verification`/`Evidence`/`Owner`/`VerifiedAt` types keeps procedures on one verification path while letting a manual run stand in for an automated test, which is the realistic evidence for a runbook. Unifying the full typed evidence vocabulary is the dedicated concern of V5.8 and is not pulled forward here.

## Consequences

A procedure with a numbered body renders identically to the page-level ordered list (`<ol><li>...</li></ol>`) while the graph artifact records the verbatim prose body and `kind: "procedure"`, so retrieval and diff treat the body as text and only the human-facing HTML carries visual ordering.

The strict body rule means a procedure that is genuinely a single paragraph of guidance cannot be authored as a `procedure`; that content belongs in a `claim` or `glossary`. This is intentional — it keeps the kind's meaning ("ordered steps") enforced structurally rather than by convention.

Procedure verification reuses claim's machinery, so verified metadata (`owner`, `verified_at`, evidence) flows into the graph node fields through the existing metadata projection with no new graph shape. A verified-procedure re-verify proof obligation (re-verify when the body changes) is deferred; procedures have a `verified` status but no V5.2 trigger, mirroring how V5.1 deferred the verified-constraint obligation.

Adding `HumanReview` to the shared `Evidence` enum touches `claim.rs` (the enum's home) and the HTML evidence renderer, but is additive: claim verification never constructs the variant, and the V3 review-obligation projection (`knowledge_object/metadata.rs`), which enumerates only `source`/`test`/`reviewed_by`, is unaffected — `human_review` simply does not participate in review obligations until the V5.8 evidence-model slice revisits that vocabulary.
