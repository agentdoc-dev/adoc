# AgentDoc Contradiction Guide

`contradiction` Knowledge Objects are manually authored cross-references that link two or more existing `claim` objects that conflict with each other. Per ADR-0026, V5 contradictions are read-only authored knowledge; automated contradiction detection is deferred to V6+.

## Read-only authored knowledge

A `contradiction` is authored by a human who has identified a conflict between two or more claims. It is:

- **Not automatically detected.** Authors write contradictions explicitly.
- **Not automatically resolved.** Resolution is a human decision reflected in the `status` field.
- **Not automatically propagated.** Changing a `contradiction` to `resolved` does NOT change the `status` of the cited claims.

## Required fields

| Field | Values | Notes |
|-------|--------|-------|
| `status` | `unresolved`, `resolved`, `dismissed` | lifecycle state |
| `severity` | `low`, `medium`, `high`, `critical` | how serious the conflict is |
| `claims` | `[object.id, ...]` | at least two distinct `claim` IDs |
| `body` | prose | explanation of the conflict |

## How to use a contradiction in answers

- **Surface active contradictions.** Before answering definitively about a claim, check whether any `unresolved` contradiction references that claim. If one exists, surface it to the user.
- **Cite by Object ID.** Reference the contradiction and both conflicting claims by their Object IDs.
- **Do not invent resolutions.** If the contradiction is `unresolved`, report the conflict as unresolved. Do not guess which claim is correct.
- **Resolved/dismissed contradictions.** A `resolved` contradiction means the conflict has been addressed (e.g. one claim was updated). A `dismissed` contradiction means the conflict was judged non-applicable. Both are terminal states; you may note them as context but they do not require active surfacing.

## `status` lifecycle

```
unresolved (active) --> resolved (terminal)
unresolved (active) --> dismissed (terminal)
```

Only `unresolved` contradictions are **active** and must be surfaced when answering about a cited claim.

## Wire surface

`contradiction` nodes are emitted into the graph artifact (`adoc.graph.v3`) with:

- `kind: "contradiction"`
- `status`: the lifecycle status (`unresolved`, `resolved`, `dismissed`)
- `fields.severity`: the severity level
- `contradiction_claims`: the list of conflicting claim Object IDs (sorted, deduplicated)
- `body`: the prose explanation

They fold into the retrieval surface (`adoc.retrieval.v0`) like any other Knowledge Object.
