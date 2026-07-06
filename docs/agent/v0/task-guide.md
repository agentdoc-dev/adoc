# AgentDoc Task Guide

`task` Knowledge Objects are documentation action items (PRD §13.11, V6.5.4). Each one names a piece of documentation work — what needs doing, who owns it, and optionally by when.

## What a task object is

A `task` object records tracked documentation work: the action in prose (the body), the accountable party (`owner`), the lifecycle state (`open` or `done`), and an optional `due` date. A task without an owner is a wish — `owner` is required unconditionally, the only kind beyond `policy` where that holds.

## Required fields

| Field | Notes |
|-------|-------|
| `id` | Object ID — dot-separated lowercase identifier, e.g. `billing.update-support-runbook` |
| `status` | Closed set: `open`, `done` |
| `owner` | Non-empty accountable party |
| `body` | Non-empty prose describing the action |

Missing or invalid values emit `schema.task_missing_status` / `schema.task_invalid_status` / `schema.task_missing_owner` / `schema.task_invalid_due`.

## Optional fields

| Field | Notes |
|-------|-------|
| `due` | `YYYY-MM-DD` date the task is due |
| `depends_on`, `supersedes`, `related_to` | Standard relation fields, unchanged |

## The overdue warning

An `open` task whose `due` date is strictly before today emits the `task.overdue` WARNING at check/build time. Warnings never fail the build — the task stays valid; the warning is a nudge to complete the work and set `status: done`, or move the `due` date. `done` tasks and tasks without `due` are exempt. The check is clock-dependent: it runs against the local calendar date. The rendered HTML mirrors the same rule: an open past-due task's card carries a `task--overdue` class alongside `task--open`.

## Authoring syntax

```
::task billing.update-support-runbook
owner: support-ops
status: open
due: 2026-05-20
depends_on: billing.credits.refund-on-failed-persistence
--
Update the support runbook to mention refund behavior after persistence failure.
::
```

## Wire surface

`task` nodes are emitted into the graph artifact (`adoc.graph.v4`) with:

- `kind: "task"` — the node-level kind discriminant
- `status` — the lifecycle status (`open` / `done`, lifecycle-only per ADR-0039)
- `fields["owner"]` — the accountable party
- `fields["due"]` — the due date, when authored
- `body` — the prose action description

Relation fields emit graph edges as usual (`depends_on` and friends). Tasks fold into the retrieval surface (`adoc.retrieval.v1`) like any other Knowledge Object.

## How to cite task objects in answers

- **Reference by Object ID.** Cite the task object's ID when reporting documentation work status.
- **Report the state as stored.** Surface `status`, `owner`, and `due` exactly as recorded; do not infer completion.
- **Respect the lifecycle.** An `open` task is pending work, not a statement of fact — never cite a task body as verified knowledge.
