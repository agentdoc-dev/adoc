# AgentDoc Observation Guide

`observation` Knowledge Objects record findings from support, analytics, research, and ops (PRD §13.9, V6.5.2). Each one captures what was seen — not what is claimed to be true.

## What an observation object is

An `observation` records a finding as data: what was observed, where it came from (`source`), how much data backs it (`sample_size`), and when (`observed_at`). Observations are never `verified` — their authority comes from the data itself, so the status set is the closed single value `observed`. Knowledge distilled from observations belongs in `claim` objects.

## Required fields

| Field | Notes |
|-------|-------|
| `id` | Object ID — dot-separated lowercase identifier, e.g. `onboarding.credit-confusion` |
| `status` | Closed set with a single value: `observed`. Any other value emits `schema.observation_invalid_status` |
| `body` | Non-empty prose describing what was seen |

A missing status emits `schema.observation_missing_status`.

## Optional fields

| Field | Notes |
|-------|-------|
| `source` | Free-string inline evidence naming where the finding came from (e.g. `support_tickets`) |
| `sample_size` | Positive integer — the number of data points behind the finding. Anything else emits `schema.observation_invalid_sample_size` |
| `observed_at` | ISO 8601 date (`YYYY-MM-DD`). Anything else emits `schema.observation_invalid_observed_at` |
| `evidence_ref` | Object IDs of `source` objects backing the finding — coexists with inline `source:` per ADR-0027 |

## Authoring syntax

```
::observation onboarding.credit-confusion
status: observed
source: support_tickets
sample_size: 37
observed_at: 2026-04-30
--
Users often misunderstand credit usage before their first generation.
::
```

## Wire surface

`observation` nodes are emitted into the graph artifact (`adoc.graph.v4`) with:

- `kind: "observation"` — the node-level kind discriminant
- `status: "observed"` — the lifecycle status (lifecycle-only per ADR-0039)
- `fields["sample_size"]` / `fields["observed_at"]` — the typed optionals, hashed as authored fields
- `evidence` — the inline `source:` entry and any resolved `evidence_ref` targets, feeding the derived `evidence_quality` unchanged (V5 evidence model)
- `body` — the prose finding

They fold into the retrieval surface (`adoc.retrieval.v0`) like any other Knowledge Object; the body is searchable, while `sample_size`/`observed_at` are echoed on records as metadata, not indexed as meaning.

## How to cite observation objects in answers

- **Reference by Object ID.** Cite the observation's ID when reporting user-facing findings.
- **Report the numbers as stored.** Surface `fields["sample_size"]` and `fields["observed_at"]` exactly as recorded; do not extrapolate.
- **Keep observations and claims apart.** An observation is evidence of what was seen, not verified knowledge — prefer `verified` claims for authoritative answers and cite observations as supporting context.
