# AgentDoc Question Guide

`question` Knowledge Objects are tracked open questions (PRD §13.10, V6.5.3). Each one records a question the team has not yet answered — or, once answered, points at the knowledge that answered it.

## What a question object is

A `question` object makes uncertainty first-class: instead of an unanswered thread in chat, the open question lives in the docs with an ID, an owner, and a lifecycle. When the question is answered, it does not get deleted — it transitions to `answered` and names the `claim` or `decision` that resolved it via `resolved_by`.

## Required fields

| Field | Notes |
|-------|-------|
| `id` | Object ID — dot-separated lowercase identifier, e.g. `billing.trial-credit-expiration` |
| `status` | Closed set: `open`, `answered` |
| `body` | Non-empty prose stating the question |

A missing status emits `schema.question_missing_status`.

## Optional fields

| Field | Notes |
|-------|-------|
| `owner` | Team or person accountable for driving the question to an answer |
| `resolved_by` | Required when `status: answered` — the Object ID of the `claim` or `decision` that answered the question |

## The answered-question rule

An `answered` question must point at the knowledge that answered it:

- `status: answered` without `resolved_by:` emits `schema.question_answered_missing_resolved_by`.
- `resolved_by:` on a question whose status is not `answered` emits `schema.question_unexpected_resolved_by`.
- `resolved_by:` naming an Object ID that does not exist emits `schema.question_resolved_by_not_found`.
- `resolved_by:` naming an object that is not a `claim` or `decision` (e.g. a `glossary` term) emits `schema.question_resolved_by_wrong_kind`.

## Authoring syntax

```
::question billing.trial-credit-expiration
owner: product-growth
status: open
--
Should unused trial credits expire after 30 days or remain available indefinitely?
::
```

## Wire surface

`question` nodes are emitted into the graph artifact (`adoc.graph.v4`) with:

- `kind: "question"` — the node-level kind discriminant
- `status` — `open` or `answered` (lifecycle-only per ADR-0039)
- `fields["owner"]` / `fields["resolved_by"]` — when authored
- `body` — the prose question

An answered question additionally emits a derived `resolved_by` graph edge from the question to the answering claim/decision, so traversal (and `adoc why` on the answering object) can walk question → answer.

They fold into the retrieval surface (`adoc.retrieval.v0`) like any other Knowledge Object — the question body is searchable.

## How to cite question objects in answers

- **Surface open questions.** When a retrieved `question` with `status: open` is relevant, say so explicitly — the docs record the uncertainty; do not invent an answer.
- **Follow `resolved_by`.** For an `answered` question, cite the resolving `claim`/`decision` by Object ID as the authoritative answer.
- **Reference by Object ID.** Cite the question's ID when reporting that something is an open question.
