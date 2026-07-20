# AgentDoc Source Guide

`source` Knowledge Objects are reusable evidence pointers that represent a single external artefact — a source file, test, URL, commit, or any other named evidence kind. Per ADR-0027, V5.7 introduces `source` objects as opt-in authored knowledge; they coexist with inline V0 evidence fields and do not deprecate them.

## What a source object is

A `source` object (PRD §13.15) is a named, reusable reference to evidence outside the documentation workspace. It can point to a repo-relative file path (e.g. a source code file or test) or an absolute URL (e.g. a pull request, incident report, or external specification). Once authored, a `source` object can be cited anywhere a Knowledge Object ID is accepted.

Source objects appear in `adoc.graph.v4` nodes with `kind: "source"`. Evidence kind and path or URL are projected into the node's `fields` map.

## Required fields

| Field | Notes |
|-------|-------|
| `id` | Object ID — dot-separated lowercase identifier, e.g. `billing.consume-use-case` |
| `kind` | Evidence kind — one of the canonical values listed below |
| `path` **or** `url` | Exactly one must be present; providing both is an error |
| `body` | Non-empty prose describing what this evidence artefact contains |

## Optional fields

| Field | Notes |
|-------|-------|
| `owner` | Team or person responsible for the artefact |
| `symbol` | Exported symbol name within the artefact (e.g. a function or class) |
| `commit` | Git commit SHA at which the artefact was last reviewed |
| `last_seen_at` | Date the artefact was last verified (ISO 8601) |
| `hash` | Evidence Anchor (ADR-0048): `sha256:` + 64 lowercase hex over the cited file's full bytes (`shasum -a 256 <file>`). On path-target sources `adoc check` re-hashes the file and warns `evidence.hash_drift` when the content changed, `evidence.hash_target_missing` when the path is gone, and `evidence.hash_invalid` for malformed values (the help carries the actual hash). On url-target sources a `hash` warns `evidence.hash_unverifiable`. Warnings never fail check |

## Evidence kind vocabulary

| Kind | Target | Description |
|------|--------|-------------|
| `source_code` | path only | Production source code file |
| `test` | path only | Automated test file |
| `commit` | path or url | Git commit |
| `pull_request` | url only | Pull request in a code-review system |
| `issue` | url only | Issue or ticket in a tracker |
| `design_doc` | path or url | Design document or RFC |
| `human_review` | path or url | Human review artefact (e.g. a review document) |
| `external_url` | url only | Any external web resource |
| `api_schema` | path or url | API schema (OpenAPI, GraphQL, Protobuf, etc.) |
| `runtime_metric` | url only | Live or historical runtime metric |
| `incident` | url only | Incident report or post-mortem |
| `support_ticket` | url only | Customer support ticket |
| `audit_record` | path or url | Audit or compliance record |
| `policy_reference` | path or url | Policy or regulatory reference |
| `dataset` | path or url | Dataset used in analysis or training |
| `experiment` | url only | Experiment report or A/B test result |

## The path-XOR-url rule

A `source` object must carry exactly one of `path` or `url`:

- `path` — a repo-relative path with no leading `/`, no `..` segments, no Windows drive letters, and no backslashes (e.g. `src/features/credits/consume.use-case.ts`).
- `url` — a well-formed absolute URL with an `http`, `https`, or `mailto` scheme (e.g. `https://github.com/org/repo/pull/42`).

Providing both emits `schema.source_conflicting_path_and_url`. Providing neither emits `schema.source_missing_path_or_url`.

## Kind-to-target correlation

Each evidence kind restricts which target type is valid:

- **Path-only** (`source_code`, `test`): `url` is not accepted. Using `url` emits `schema.source_kind_target_mismatch`.
- **URL-only** (`pull_request`, `issue`, `external_url`, `runtime_metric`, `incident`, `support_ticket`, `experiment`): `path` is not accepted. Using `path` emits `schema.source_kind_target_mismatch`.
- **Either** (`commit`, `design_doc`, `human_review`, `api_schema`, `audit_record`, `policy_reference`, `dataset`): both `path` and `url` are structurally valid (but still mutually exclusive — only one at a time).

## Coexistence with inline V0 evidence (ADR-0027)

Per ADR-0027, inline evidence fields on claims and decisions (`source:`, `test:`, `reviewed_by:`) are **not deprecated** in V5. Source objects are an opt-in upgrade for teams that want a named, reusable evidence pointer. The resolution mechanism — how a `source` object ID is linked from a claim or decision — lands in V5.8.

Do not remove existing inline evidence fields when authoring `source` objects. Both forms are valid and carry independent semantics.

## Authoring syntax

```
::source billing.consume-use-case
kind: source_code
path: apps/backend/src/features/credits/consume.use-case.ts
owner: backend-platform
--
Implementation of credit consumption. This file is the authoritative
reference for how credits are decremented after a purchase event.
::
```

```
::source billing.stripe-api
kind: external_url
url: https://stripe.com/docs/api/charges
owner: backend-platform
--
Stripe Charges API documentation. Referenced by the payment processing claim.
::
```

## Wire surface

`source` nodes are emitted into the graph artifact (`adoc.graph.v4`) with:

- `kind: "source"` — the node-level kind discriminant
- `fields["kind"]` — the evidence kind string (e.g. `"source_code"`)
- `fields["path"]` — the repo-relative path, if the target is a path
- `fields["url"]` — the absolute URL, if the target is a URL
- `body` — the prose description

They fold into the retrieval surface (`adoc.retrieval.v1`) like any other Knowledge Object.

## How to cite source objects in answers

- **Reference by Object ID.** Cite a source object by its ID when explaining what evidence supports a claim or decision.
- **Surface the evidence kind and target.** When surfacing a `source` node, note the `fields["kind"]` and `fields["path"]` or `fields["url"]` so the reader knows where to find the artefact.
- **Do not invent paths or URLs.** Only report the `path` or `url` as stored in the graph; do not guess or extrapolate artefact locations.
