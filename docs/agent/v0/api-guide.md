# AgentDoc API Guide

`api` Knowledge Objects are typed API contracts (PRD §13.7, V6.5.1). Each one names a single operation — an HTTP endpoint or a non-HTTP interface entry point — with its behavior described in prose and its shape carried by typed fields.

## What an api object is

An `api` object records the documented contract for one operation: what it is called on the wire (`method` + `path`, or `interface_type` + `symbol`) and what it does (the body). A `verified` api is verified by its schema source, not by human assertion — verification requires schema evidence.

## Required fields

| Field | Notes |
|-------|-------|
| `id` | Object ID — dot-separated lowercase identifier, e.g. `billing.consume-credit` |
| `method` **or** `interface_type` | Exactly one. `method` is a closed uppercase HTTP method (`GET`, `HEAD`, `POST`, `PUT`, `DELETE`, `CONNECT`, `OPTIONS`, `TRACE`, `PATCH`); `interface_type` is an open string (`grpc`, `graphql`, …) |
| `path` **or** `symbol` | Exactly one. `path` is a non-empty `/`-prefixed route template; `symbol` names a code entry point |
| `body` | Non-empty prose describing the contract's behavior |

Providing both sides of a one-of pair emits `schema.api_conflicting_method_and_interface_type` / `schema.api_conflicting_path_and_symbol`; providing neither emits `schema.api_missing_method_or_interface_type` / `schema.api_missing_path_or_symbol`.

## Optional fields

| Field | Notes |
|-------|-------|
| `status` | Closed set: `draft`, `verified`, `deprecated` |
| `owner` | Required when `status: verified` |
| `verified_at` | Required when `status: verified` (ISO 8601 date) |
| `impacts` | Repo-relative paths this contract depends on — an api naturally declares its OpenAPI/proto file |
| `evidence_ref` | Object IDs of `source` objects backing the contract |

## The verified-api evidence rule

A `verified` api must carry **schema evidence**: an inline `source:` entry, or an `evidence_ref` resolving to a `source` object whose kind is `api_schema` or `source_code`. Human review alone (`reviewed_by:`) is not sufficient — that emits `api.verified_missing_schema_evidence`.

## Authoring syntax

```
::api billing.consume-credit
method: POST
path: /api/billing/credits/consume
status: verified
source: openapi/billing.yaml#/paths/~1credits~1consume
owner: backend-platform
verified_at: 2026-04-30
--
Consumes one or more credits for a completed generation job.
::
```

## Wire surface

`api` nodes are emitted into the graph artifact (`adoc.graph.v4`) with:

- `kind: "api"` — the node-level kind discriminant
- `status` — the lifecycle status, when authored (lifecycle-only per ADR-0039)
- `fields["method"]` / `fields["interface_type"]` — the operation half
- `fields["path"]` / `fields["symbol"]` — the location half
- `body` — the prose contract description

They fold into the retrieval surface (`adoc.retrieval.v0`) like any other Knowledge Object. A verified api with `impacts:` participates in `adoc impacted-by`: a change to its declared schema file flags the api for contract re-verification.

## How to cite api objects in answers

- **Reference by Object ID.** Cite the api object's ID when answering questions about endpoint behavior.
- **Report the signature as stored.** Surface `fields["method"]` and `fields["path"]` (or their interface/symbol counterparts) exactly as recorded; do not guess routes or methods.
- **Respect the lifecycle.** Prefer `verified` apis over `draft` ones; flag `deprecated` contracts when citing them.
