# Expanded Pilot Review Checklist

Use this checklist when hand-reviewing the generated `docs.html` and
`docs.graph.json` for the V5 Expanded Pilot (V5.9).

## HTML — every V5 kind renders distinctly

- `agent_instruction` shows the banner "Agent Instruction. Authored knowledge, NOT runtime ACL."
- `contradiction` shows the conflicting claims as a linked list (`contradiction__claims`) with a severity badge.
- `source` shows the evidence-kind badge and the path (`source_code`) or URL link (`external_url`).
- `procedure` renders its numbered body as an ordered list (`<ol>`) with sequential steps.
- `constraint` shows its severity badge and body.
- `policy` shows an approval header listing approvers and the effective date.
- `example` shows a fenced code block in the declared `lang`; the executable one shows `checks` + `sandbox` with the "Not executed by adoc" caveat.
- Object references in prose (`[[id]]`) render as links.

## Graph JSON

- `schema_version` is `adoc.graph.v3`.
- 18 Knowledge Object nodes: 6 claim, 1 decision, 2 glossary, 1 constraint, 1 procedure, 2 example, 1 policy, 1 agent_instruction, 1 contradiction, 2 source.
- Two `evidence` edges: `billing.credits.consume` and `billing.credits.use-ledger` → `billing.consume-use-case`.
- `relation` and `reference` edges preserve source direction.
- Every Knowledge Object `source_span` points back to `examples/expanded-pilot/**/*.adoc`.

## Diagnostics

- `adoc check` reports exactly `0 errors, 2 warnings` — both `lifecycle.expired`
  (`billing.credits.legacy-export`, `security.audit.retention`). See the budget
  table in `docs/expanded-pilot.md`.
