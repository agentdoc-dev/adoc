# Expanded Pilot

The Expanded Pilot at `examples/expanded-pilot/` is the end-to-end
evaluation fixture for the **V5 Expanded Knowledge Model**. It mirrors the
role the Billing Pilot plays for V0–V3 and the Markdown Pilot plays for V4:
a realistic, hand-curated tree exercised by
`cargo test -p adoc-cli --test expanded_pilot` on every workspace build.

The pilot is pure native AgentDoc Source (`.adoc`, Strict Mode) and
exercises every new V5 kind — `constraint`, `procedure`, `example`,
`policy`, `agent_instruction`, `contradiction`, `source` — plus the V5.8
typed evidence model, across the auth, billing, and security domains.

## Build

```bash
adoc check examples/expanded-pilot/
adoc build examples/expanded-pilot/ --out dist
```

The pilot exits `0` from `check` and `build`. The only diagnostics are two
`lifecycle.expired` warnings (see the budget below); warnings never fail
the build. Every `.adoc` file must remain strict-valid — any error breaks
the pilot.

## Directory Shape

```text
examples/expanded-pilot/
  agentdoc.config.yaml         # local embeddings (tests override to deterministic)
  auth/
    claims.adoc                # 3 claims (2 contradicted + 1 verified)
    procedures.adoc            # verified procedure (role_required + rollback)
    agent-instructions.adoc    # agent_instruction w/ disjoint action sets
  billing/
    glossary.adoc              # 2 glossary terms
    claims.adoc                # verified evidence_ref claim + decision + expired claim
    examples.adoc              # verified-executable + non-executable example
    sources.adoc               # source_code source
  security/
    constraints.adoc           # constraint w/ impacts:
    policies.adoc              # active multi-approver policy + expired claim
    contradictions.adoc        # contradiction over the two auth claims
    sources.adoc               # external_url source
  meta/
    REVIEW-CHECKLIST.md        # hand-review checklist (markdown)
```

Totals: 11 `.adoc` files. The graph artifact carries **12 `page` nodes**
(11 `.adoc` + the markdown `meta/REVIEW-CHECKLIST.md`) and **18
`knowledge_object` nodes**:

| Kind                | Count |
| :------------------ | :---: |
| `claim`             |   6   |
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

`adoc check examples/expanded-pilot/` produces **0 errors, 2 warnings**.
The integration test asserts each count exact-match; changing either
requires updating both the fixture and the test in the same commit.

| Code                | Count | Source                                                              |
| :------------------ | :---: | :----------------------------------------------------------------- |
| `lifecycle.expired` |   2   | `billing/claims.adoc` (`billing.credits.legacy-export`), `security/policies.adoc` (`security.audit.retention`) |

Both warnings are driven by **fixed past `expires_at` values**
(`2026-01-15`, `2026-02-01`). A past date stays in the past, so the budget
is stable across runs regardless of the system clock. Do not use
expirations relative to "now" — they would make the count non-deterministic.

## What This Pilot Exercises

- **Every V5 kind** with a complete authoring → validation → rendering →
  graph emission → retrieval story.
- **The V5.8 evidence model**: inline typed evidence (`test:`) combined
  with `evidence_ref:` to a `source` object, on both a `claim` and a
  `decision`, producing typed `evidence[]` projections and
  `evidence` graph edges.
- **Manual contradictions** (ADR-0026): `auth.session.conflict` links two
  pre-existing claims that are manually `status: contradicted`. V5 does not
  auto-detect or auto-propagate.
- **The `agent_instruction` runtime-not-enforced contract** (ADR-0025):
  the rendered banner and the `adoc://agent/v0/agent-instruction-guide`
  resource.
- **Retrieval, diff, review, and patch** over the new kinds:
  `adoc search "policy"` returns the active policy first;
  `adoc graph security.production-db-access` traverses to its related claim;
  a verified-claim body edit yields a re-verify obligation and a clean
  embedded `adoc.patch.check.v0`.
- **Graceful rejection of the old graph model**: a stale `adoc.graph.v2`
  artifact is rejected with `schema.unsupported_version` rather than
  silently dropping the new kinds.

## What This Pilot Does Not Exercise

- **Lifecycle automation.** Scheduled `verified` → `stale` transitions,
  automatic claim-status propagation on contradiction resolution,
  evidence-quality scoring, and policy review-interval drift are scheduled
  as **V5.10**.
- **Example execution.** Executable examples are a declaration-only
  contract in V5; `adoc` never runs the declared `checks`.
- **Prose retrieval.** Prose blocks remain non-retrievable; only Knowledge
  Objects are citable (V1.7 extends retrieval to prose independently).

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
