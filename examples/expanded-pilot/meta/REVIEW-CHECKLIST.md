# Expanded Pilot Review Checklist

Use this checklist when hand-reviewing the generated `docs.html` and
`docs.graph.json` for the Expanded Pilot (V5.9, extended to the fifteen-kind
vocabulary by V6.5.5).

## HTML — every kind renders distinctly

- `agent_instruction` shows the banner "Agent Instruction. Authored knowledge, NOT runtime ACL."
- `contradiction` shows the conflicting claims as a linked list (`contradiction__claims`) with a severity badge.
- `source` shows the evidence-kind badge and the path (`source_code`, `api_schema`) or URL link (`external_url`).
- `procedure` renders its numbered body as an ordered list (`<ol>`) with sequential steps.
- `constraint` shows its severity badge and body.
- `policy` shows an approval header listing approvers and the effective date.
- `example` shows a fenced code block in the declared `lang`; the executable one shows `checks` + `sandbox` with the "Not executed by adoc" caveat.
- `api` shows the endpoint signature header: method badge (`POST`) plus the path in code style, above the body.
- `observation` shows sample size and observed date as metadata.
- The open `question` shows the prominent "Open" badge; the answered one links the resolving decision (`billing.credits.use-ledger`).
- The open overdue `task` card shows owner and due date with the overdue modifier (`task--overdue`); the done task shows its done state without it.
- Object references in prose (`[[id]]`) render as links.

## Graph JSON

- `schema_version` is `adoc.graph.v4`.
- 27 Knowledge Object nodes: 8 claim, 1 decision, 2 glossary, 1 constraint, 1 procedure, 2 example, 1 policy, 1 agent_instruction, 1 contradiction, 3 source, 1 api, 1 observation, 2 question, 2 task.
- Three `evidence` edges: `billing.credits.consume` and `billing.credits.use-ledger` → `billing.consume-use-case`; `billing.consume-credit` → `billing.openapi-schema`.
- The answered question emits a `resolved_by` edge to `billing.credits.use-ledger`; the open task a `depends_on` relation edge to `billing.credits.consume`.
- `relation` and `reference` edges preserve source direction.
- Every Knowledge Object `source_span` points back to `examples/expanded-pilot/**/*.adoc`.

## Diagnostics

- `adoc check` reports exactly `0 errors, 6 warnings`. See the budget table
  in `docs/expanded-pilot.md`.
