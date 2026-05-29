# ADR-0030: Example Status Is an Optional Closed Enum and Verification Is Declaration-Only

## Status

Accepted.

## Context

V5.3 introduces the `example` Knowledge Object — a code, API, or usage example closing PRD §33.2 (executable example declaration). The V5 design contract (V5-DESIGN.md §V5.3) fixed the required fields (`id`, one of `lang`/`format`, `body`) and the verified rule (a `verified` example requires both `checks` and `sandbox`), but left several shapes to be confirmed at slice time:

1. **`status` modelling.** The acceptance criteria use `status: verified`, yet `status` is not in the required-field list and the contract never enumerates example statuses. The codebase has three precedents: `claim`'s free-form `ClaimStatus(String)`, `decision`'s closed `DecisionStatus`, and `procedure`'s required closed `ProcedureStatus` (ADR-0029).

2. **What "verified" means for an example.** `procedure`/`claim` verification is evidence-backed (`owner` + `verified_at` + evidence via the shared `Verification` value object). The contract's verified-example rule names only `checks` and `sandbox`, with no evidence fields.

3. **Validation location.** The original V5-DESIGN workspace layout placed `example_required_fields.rs` and `example_verified_executable.rs` under `infrastructure/validate/objects/`. But that directory does not exist — V5.1 (`constraint`) and V5.2 (`procedure`) made required-field and verified-status validation aggregate-owned, and the ROADMAP defers `infrastructure/validate/objects/` to V5.6's first genuinely cross-aggregate rule.

4. **Diagnostic vocabulary.** The contract names three example diagnostics (`schema.example_missing_lang`, `_verified_requires_checks`, `_verified_requires_sandbox`) but introduces two new typed value objects (`Lang`, `SandboxName`) whose malformed input needs a strict-mode error.

## Decision

**`ExampleStatus` is an OPTIONAL closed enum `Draft | Verified | Deprecated`.** Unlike `procedure`, `status` is not required — an absent status means an ordinary, unverified example, matching the contract's required-field set ("`id`, one of `lang`/`format`, `body`") and its non-executable pilot example ("lang only"). When present it is ASCII-trimmed and lowercase-exact; an unknown spelling reuses the existing `schema.invalid_status` so a typo like `varified` fails loudly rather than silently demoting a verified example. The graph node discriminant (`status`) is therefore `Option`-shaped and null when absent.

**A `verified` example is *executable-declared*, not evidence-reviewed.** Verification here requires only that both `checks` and `sandbox` declarations are present; it does NOT use the shared `Verification`/`Owner`/`VerifiedAt`/`Evidence` machinery that backs `claim` and `procedure`. Per the V5 non-goal, neither `checks` nor `sandbox` is executed by `adoc check` or `adoc build` — running them is a later runtime milestone. The HTML renderer makes this explicit with a "Not executed by adoc" caveat next to `checks`.

**Required-field and verified-executable validation is aggregate-owned in `example.rs`.** This mirrors V5.1/V5.2 and supersedes the older V5-DESIGN text that placed `example_required_fields.rs`/`example_verified_executable.rs` under `infrastructure/validate/objects/`. The verified-executable rule (`verified ⇒ checks ∧ sandbox`) is intra-aggregate, so it lives in the constructor; the `infrastructure/validate/objects/` directory remains deferred to V5.6, when the first cross-aggregate rule (contradiction claim-reference resolution) actually needs it.

**Two diagnostics are added beyond the three named in the contract, for strict-mode grammar enforcement.** `schema.example_invalid_lang` and `schema.example_invalid_sandbox` reject malformed `Lang`/`SandboxName` values, mirroring `constraint`'s missing/invalid severity pairing. The `SandboxName` grammar is exactly the `Lang` grammar (`[a-z][a-z0-9_+-]*`) plus `:` as a namespace separator (`[a-z][a-z0-9_+:-]*`) — `.` is deliberately disallowed, since the only contract example is `node-test`; the grammar can widen later without breaking authored documents. A missing body reuses the generic `schema.missing_field`.

## Consequences

`lang`, `format`, `checks`, and `sandbox` are projected into the graph node `fields` map through the existing metadata projection (mirroring how `procedure` verified-metadata fields flow), and `status` is the node discriminant. No new graph shape is introduced; the `adoc.graph.v3` bump from V5.1 already covers the new `example` kind and its fields additively.

Because example "verified" carries no evidence, there is no verified-example re-verify proof obligation — `example` deliberately sits outside the evidence/obligation machinery that `claim`, `decision`, and (in V5.8) the typed evidence model govern. If a future slice wants examples whose code is genuinely checked, that is a runtime-execution milestone, not a widening of this aggregate.

Keeping validation aggregate-owned means an `example` cannot be invalidated after construction: the only path to an `Example` is its fallible `build_from_parsed`/`try_new` constructor, and every invariant (lang-or-format present, verified ⇒ checks ∧ sandbox, typed-field grammars) is enforced there. The deferred `infrastructure/validate/objects/` directory stays unbuilt until a rule actually spans more than one aggregate.
