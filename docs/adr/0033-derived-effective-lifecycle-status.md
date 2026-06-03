# ADR-0033: Derived Effective Lifecycle Status (`effective_status`)

**Status:** Accepted
**Date:** 2026-06-03
**Slice:** V5.10

## Context

adoc is a pure-projection compiler: authored fields are canonical, and the `content_hash` of every graph Knowledge Object node is computed from the authored source. The `adoc.graph.v3` patch contract (ADR-0028) depends on `content_hash` stability — a `patch --check` operation compares a base hash to the live artifact and rejects if they diverge. Any field that enters the hash therefore affects whether patches are accepted.

V5.10 introduces lifecycle automation: a `verified` claim with an `expires_at` date in the past should surface as effectively degraded — it can no longer be treated as authoritative without re-verification. However, injecting `status: "stale"` into the authored status field would:

1. Mutate the canonical authored value (violating the source-as-truth principle).
2. Change `content_hash` on every calendar day an expiry is crossed (breaking patch preconditions every midnight without any source change).
3. Require the expiry date to live outside `fields` to avoid self-reinforcing hash churn.

## Decision

**Derived `effective_status` is additive, projection-only, and excluded from `content_hash`.**

Two new fields are added to `GraphKnowledgeObjectNode` and to `RetrievalRecord`:

- `effective_status: Option<String>` — `Some("stale")` when the authored `status` is exactly `"verified"` and `expires_at` is a valid `%Y-%m-%d` date strictly less than `today`. `None` in all other cases.
- `effective_reason: Option<String>` — `Some("expired:<YYYY-MM-DD>")` when `effective_status` is set. Always `None` otherwise.

Both fields are skipped during serialization when `None` (via `serde(skip_serializing_if = "Option::is_none")`) so existing graph fixtures remain byte-stable when no object meets the stale condition.

The derivation logic lives in `infrastructure::artifact::graph_json::derive_effective_status` — a standalone, clearly named helper designed so that TB4 can layer the `contradicted` case on top without modifying the stale path.

**Authored `status` remains canonical and hashed.** The `KnowledgeObjectHashPayload` in `graph_json.rs` deliberately omits `effective_status` and `effective_reason`, so the `content_hash` of a node is identical regardless of what day it is compiled. A `patch --check` precondition computed last week is still valid today even if the claim has since crossed its `expires_at` boundary.

**Semantics of `effective_status`:

| authored status | `expires_at` | `< today` | `effective_status` |
|---|---|---|---|
| `"verified"` | present, valid | yes | `"stale"` |
| `"verified"` | present, valid | no (today or future) | `None` |
| `"verified"` | absent | — | `None` |
| `"verified"` | present, unparseable | — | `None` |
| any other | present, valid, past | — | `None` (only `lifecycle.expired` warning) |

Non-verified objects that carry a past `expires_at` continue to receive only the existing `lifecycle.expired` WARNING diagnostic (unchanged from pre-V5.10 behaviour).

**Threading `today`:** the compile pipeline already carries `today: NaiveDate` for lifecycle validation. It is now also threaded to `GraphJsonArtifact::build_for_date` (new method alongside the pre-existing trait `build`) and `HtmlRenderer::render_workspace_for_date`. This allows tests to pin the date without wall-clock dependency and keeps the default public-API entry points (`build_with_provider`, `compile_with_provider`) unchanged.

**HTML badge:** when `effective_status` is set to `"stale"`, the HTML renderer injects `<span class="ko__effective-status ko__effective-status--stale">stale</span>` immediately before the closing `</section>` tag of the Knowledge Object. This is additive and does not affect any existing assertion on nodes without effective_status.

## Consequences

- `GraphKnowledgeObjectNode` gains two new optional fields placed after all existing fields (field ordering preserves existing JSON key stability).
- `RetrievalRecord` mirrors the two fields as a clone-through from the graph node.
- `KnowledgeObjectHashPayload` is unchanged — no hash churn.
- Existing graph fixtures not containing `verified` claims with past `expires_at` are byte-identical after this change.
- TB4 will add the `contradicted` case: when a `contradiction` node names a `verified` claim, that claim gets `effective_status: "contradicted"`. If both stale and contradicted apply simultaneously, stale wins (stale is the stronger lifecycle signal). The `derive_effective_status` helper is intentionally structured for this layering.

## Alternatives considered

- **Mutating authored `status` in the graph artifact.** Rejected: changes `content_hash` on every expiry crossing, destabilising patch preconditions with no source change.
- **Separate derived-artifact file.** Rejected: an additional file per project creates deployment and synchronisation burden; the additive field approach is zero-friction.
- **Embedding `today` in the hash payload (making the hash date-sensitive).** Explicitly rejected: the entire point is a date-stable hash for stable `patch --check`.
