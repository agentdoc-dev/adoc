# Expanded Pilot

The Expanded Pilot at `examples/expanded-pilot/` is the end-to-end
evaluation fixture for the **V5 Expanded Knowledge Model** and the
**V5.10 Lifecycle Automation** signals. It mirrors the role the Billing
Pilot plays for V0–V3 and the Markdown Pilot plays for V4: a realistic,
hand-curated tree exercised by
`cargo test -p adoc-cli --test expanded_pilot` on every workspace build.

The pilot is pure native AgentDoc Source (`.adoc`, Strict Mode) and
exercises every new V5 kind — `constraint`, `procedure`, `example`,
`policy`, `agent_instruction`, `contradiction`, `source` — plus the V5.8
typed evidence model and all four V5.10 lifecycle signals, across the auth,
billing, and security domains.

## Build

```bash
adoc check examples/expanded-pilot/
adoc build examples/expanded-pilot/ --out dist
```

The pilot exits `0` from `check` and `build`. All diagnostics are warnings
(warnings never fail the build). Every `.adoc` file must remain
strict-valid — any error breaks the pilot.

## Directory Shape

```text
examples/expanded-pilot/
  agentdoc.config.yaml         # local embeddings (tests override to deterministic)
  auth/
    claims.adoc                # 4 claims (2 contradicted + 1 verified + 1 accepted/nudge)
    procedures.adoc            # verified procedure (role_required + rollback)
    agent-instructions.adoc    # agent_instruction w/ disjoint action sets
  billing/
    glossary.adoc              # 2 glossary terms
    claims.adoc                # verified evidence_ref claim + decision + expired claim
    examples.adoc              # verified-executable + non-executable example
    sources.adoc               # source_code source
  security/
    constraints.adoc           # constraint w/ impacts:
    policies.adoc              # active policy (overdue review) + stale verified claim + low-evidence claim
    contradictions.adoc        # contradiction over auth claims including nudge claim
    sources.adoc               # external_url source
  meta/
    REVIEW-CHECKLIST.md        # hand-review checklist (markdown)
```

Totals: 11 `.adoc` files. The graph artifact carries **12 `page` nodes**
(11 `.adoc` + the markdown `meta/REVIEW-CHECKLIST.md`) and **20
`knowledge_object` nodes**:

| Kind                | Count |
| :------------------ | :---: |
| `claim`             |   8   |
| `decision`          |   1   |
| `glossary`          |   2   |
| `constraint`        |   1   |
| `procedure`         |   1   |
| `example`           |   2   |
| `policy`            |   1   |
| `agent_instruction` |   1   |
| `contradiction`     |   1   |
| `source`            |   2   |

Two `evidence` edges (V5.8) link `billing.credits.consume` and
`billing.credits.use-ledger` to the `billing.consume-use-case` source.

## Diagnostic Budget

`adoc check examples/expanded-pilot/` produces **0 errors, 5 warnings**.
The integration test asserts each count exact-match; changing any count
requires updating both the fixture and the test in the same commit.

| Code                                  | Count | Object / File |
| :------------------------------------ | :---: | :------------ |
| `lifecycle.expired`                   |   2   | `billing.credits.legacy-export` (`billing/claims.adoc`), `security.audit.retention` (`security/policies.adoc`) |
| `schema.policy_review_overdue`        |   1   | `security.production-db-access` (`security/policies.adoc`) |
| `claim.evidence_quality_low`          |   1   | `security.csrf-advisory` (`security/policies.adoc`) |
| `schema.claim_contradicted_by_unresolved` | 1 | `auth.session.csrf-protection` (`auth/claims.adoc`) |

All warnings are driven by **fixed past dates** (2020–2024). A past date
stays in the past, so the budget is stable across runs regardless of the
system clock. Do not use dates relative to "now" — they would make the
count non-deterministic.

## V5.10 Lifecycle Signals Exercised

The pilot demonstrates all four V5.10 derived lifecycle signals (ADR-0033,
ADR-0034):

| Signal | Fixture object | Graph field | Warning |
| :----- | :------------- | :---------- | :------ |
| Stale | `security.audit.retention` (verified, `expires_at: 2024-01-01`) | `effective_status: "stale"`, `effective_reason: "expired:2024-01-01"` | `lifecycle.expired` |
| Policy review overdue | `security.production-db-access` (`effective_at: 2020-01-01`, `review_interval: 90d`) | — | `schema.policy_review_overdue` |
| Evidence quality low | `security.csrf-advisory` (verified, `external_url:` only) | `evidence_quality: "low"` | `claim.evidence_quality_low` |
| Contradicted nudge | `auth.session.csrf-protection` (`status: accepted`, in unresolved contradiction) | `effective_status: "contradicted"` | `schema.claim_contradicted_by_unresolved` |

Two verified claims additionally carry **far-future** `expires_at` dates —
`billing.credits.consume` (`expires_at: 2120-01-01`) and
`auth.mfa.enforced` (`expires_at: 2125-01-01`). They fire no diagnostics
and no derived `effective_status` (the budget above is unchanged); they
exist so `adoc stale --within <N>d` (V6.1) has deterministic
`expiring_soon` records to report.

## V6.1 `adoc stale` Acceptance

The pilot is also the acceptance fixture for the V6.1 stale query
(`crates/adoc-cli/tests/expanded_pilot.rs::expanded_pilot_stale_query`).
Against a built pilot artifact, `adoc stale --format json` exits 0 with
exactly 3 records, most-overdue first:

| # | Object | Category | Reason |
| :- | :----- | :------- | :----- |
| 1 | `security.production-db-access` | `review_overdue` | `review_due:2020-03-31` |
| 2 | `security.audit.retention` | `stale` (verified → effective `stale`) | `expired:2024-01-01` |
| 3 | `billing.credits.legacy-export` | `stale` (draft, status echoed) | `expired:2026-01-15` |

`adoc stale --within 36500d` additionally lists `billing.credits.consume`
then `auth.mfa.enforced` as `expiring_soon`. All dates are wide-margin fixed
dates, so the records and their order are clock-stable.

## V6.2 `adoc contradictions` Acceptance

The pilot is also the acceptance fixture for the V6.2 contradictions query
(`crates/adoc-cli/tests/expanded_pilot.rs::expanded_pilot_contradictions_query`).
Against a built pilot artifact, `adoc contradictions --format json` exits 0
with exactly 1 contradiction — `auth.session.conflict` (severity `high`,
status `unresolved`, summary = the first body line) — and exactly 3
`contradicted_claims` in id order:

| # | Claim | Authored | Effective | Via |
| :- | :---- | :------- | :-------- | :-- |
| 1 | `auth.session.csrf-protection` | `accepted` | `contradicted` | `auth.session.conflict` |
| 2 | `auth.session.local-storage-allowed` | `contradicted` | `contradicted` | `auth.session.conflict` |
| 3 | `auth.session.memory-storage` | `contradicted` | `contradicted` | `auth.session.conflict` |

`--all` output is identical (the pilot has no resolved or dismissed
contradictions). The envelope carries no `evaluated_at`: it is a pure
function of the artifact, stable on any run date.

## V6.3 `adoc impacted-by` Acceptance

The pilot is also the evidence-path acceptance fixture for the V6.3 impacted
query (`crates/adoc-cli/tests/expanded_pilot.rs::expanded_pilot_impacted_by_query`).
Against a built pilot artifact,
`adoc impacted-by apps/backend/src/features/credits/consume.use-case.ts --format json`
exits 0 with exactly 2 impacted objects in id order, each with one
`evidence_path` reason resolved through the shared `source` object and one
impact-review proof obligation:

| # | Object | Kind / Status | Reason |
| :- | :----- | :------------ | :----- |
| 1 | `billing.credits.consume` | claim, `verified` | `evidence_path` via `billing.consume-use-case` |
| 2 | `billing.credits.use-ledger` | decision, `accepted` | `evidence_path` via `billing.consume-use-case` |

The scope negative: `adoc impacted-by crates/auth/src/session.rs` returns an
empty impacted set, exit 0 — the only object declaring that path is the
`auth.session.no-local-storage` constraint, which is outside the
verified-subject (claim/decision) scope.

## What This Pilot Exercises

- **Every V5 kind** with a complete authoring → validation → rendering →
  graph emission → retrieval story.
- **The V5.8 evidence model**: inline typed evidence (`test:`) combined
  with `evidence_ref:` to a `source` object, on both a `claim` and a
  `decision`, producing typed `evidence[]` projections and
  `evidence` graph edges.
- **Manual contradictions** (ADR-0026): `auth.session.conflict` links three
  claims — two manually `status: contradicted` and one `status: accepted`
  (the V5.10 nudge case). V5 does not auto-detect or auto-propagate.
- **The `agent_instruction` runtime-not-enforced contract** (ADR-0025):
  the rendered banner and the `adoc://agent/v0/agent-instruction-guide`
  resource.
- **V5.10 lifecycle automation** (ADR-0033, ADR-0034): all four signals
  fire with clock-stable wide-margin fixture dates; see table above.
- **Retrieval, diff, review, and patch** over the new kinds:
  `adoc search "policy"` returns the active policy first;
  `adoc graph security.production-db-access` traverses to its related claim
  (now stale); a verified-claim body edit yields a re-verify obligation and
  a clean embedded `adoc.patch.check.v0`.
- **Graceful rejection of the old graph model**: a stale `adoc.graph.v2`
  artifact is rejected with `schema.unsupported_version` rather than
  silently dropping the new kinds.

## What This Pilot Does Not Exercise

- **Example execution.** Executable examples are a declaration-only
  contract in V5; `adoc` never runs the declared `checks`.
- **Prose retrieval.** Prose blocks remain non-retrievable; only Knowledge
  Objects are citable (V1.7 extends retrieval to prose independently).
- **Automated contradiction detection.** Contradictions remain manually
  authored in V5 and V5.10 per ADR-0026; automated detection is V6+.

## Updating the Pilot

When adding a fixture object or file:

1. Place it under the matching domain directory with a valid two-segment
   Object ID.
2. If it changes a per-kind count, update the kind table above **and** the
   exact-match assertions in `crates/adoc-cli/tests/expanded_pilot.rs`.
3. If it adds an `evidence_ref` or relation, update the edge expectations.
4. If it triggers a new diagnostic, update the budget table **and** the
   test in the same commit. Use only clock-stable diagnostics (e.g. a
   fixed past `expires_at`) so the budget stays deterministic.
5. Run `cargo test -p adoc-cli --test expanded_pilot --locked` and inspect
   `dist/docs.html` against `meta/REVIEW-CHECKLIST.md`.

When removing a fixture file, keep at least: one verified claim (for the
diff/review/patch sub-test), one `agent_instruction` and one
`contradiction` (for the rendering and guide contracts), and one
`evidence_ref` (for the evidence-edge assertion).

## Apply-Loop Proof (V6.4)

`crates/adoc-cli/tests/apply_loop.rs` drives the full agent editing loop —
`adoc impacted-by` → patch → `adoc patch --apply` → post-check →
`adoc stale`/`adoc contradictions` re-check → stale-`base_hash` refusal —
against a **tempdir copy** of this pilot. The in-repo tree stays pristine:
the test copies the fixture with `support::copy_tree` and asserts every
non-target file remains byte-identical to its original. The rewritten
`billing/claims.adoc` is pinned byte-for-byte by the golden fixture
`crates/adoc-cli/tests/fixtures/v6_4_apply_loop/billing-claims.after.adoc`;
editing the pilot's `billing.credits.consume` block requires regenerating
that golden in the same commit. The diagnostic budget above is
body-edit-invariant and re-asserted by the loop test's post-check.
