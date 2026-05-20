# ADR-0012: Agent Patch V2 Artifact Contract

## Status

Accepted.

## Context

V2 needs agents to propose Knowledge Object changes without rewriting `.adoc` source. Reviewers need a stable artifact-only precondition so a patch generated from stale knowledge cannot be accepted silently, and newly created objects need enough placement information for a human source edit to be unambiguous.

## Decision

`docs.graph.json` moves to `adoc.graph.v2`. Every Knowledge Object node carries `content_hash`, defined as `sha256:` over canonical JSON for the full graph Knowledge Object node excluding its own `content_hash`. The hash includes `id`, `kind`, `status`, `body`, `page_id`, `source_span`, `fields`, and `relations`.

`adoc patch --check <patch.json>` accepts a single-operation `adoc.patch.v0` document and returns an `adoc.patch.check.v0` report. The check is read-only: it validates intent against `docs.graph.json`, emits diffs, affected relations, diagnostics, and proof obligations, and never rewrites source files.

The serialized patch artifact and serialized patch-check report are the stable wire contract. The public Rust report/input structs in `adoc-core` are convenience mirrors for the CLI and other local in-process integrations; they are not a separate artifact schema and do not make graph/search DTOs public.

Local integrations may validate inline `adoc.patch.v0` JSON through `check_patch_json`; this is the same wire document lowered by the same JSON adapter, not a second patch schema.

`create_object` patches must provide `changes.placement.page_id`, with optional `changes.placement.after`. The page must exist in the graph artifact; when `after` is present, that object must exist on the same page.

## Consequences

Existing `adoc.graph.v1` artifacts are rejected by V2-era binaries with rebuild guidance.

Patch `base_hash` values are graph-node hashes, not search-artifact embedding-input hashes.

Proof obligations can appear in an otherwise valid patch report. They mean the patch is structurally acceptable for review, not that verified knowledge has been approved.
