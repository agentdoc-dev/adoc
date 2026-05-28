# ADR-0028: Graph Artifact Bumps to `adoc.graph.v3` (Additive)

## Status

Accepted.

## Context

The JSON graph artifact (`dist/docs.graph.json`, ADR-0011) is the canonical local read model. Its schema version is a single constant — `SUPPORTED_GRAPH_SCHEMA_VERSION` in `infrastructure/artifact/graph_json.rs` — written into every emitted artifact and checked on read. The reader rejects any artifact whose `schema_version` does not match exactly, emitting `DiagnosticCode::SchemaUnsupportedVersion`.

V5 adds seven new `kind` values to the Knowledge Object node payload (`constraint` first, in V5.1) plus a small number of new per-kind fields. The reader does not enumerate `kind` strings, so structurally a v2-labelled artifact *could* carry `constraint` nodes and a v2 reader would consume them. That is exactly the hazard to avoid: a stale `adoc.graph.v2` artifact on disk — written before V5 — has no awareness of the new kinds, and a consumer pinned to the v2 reading model would silently work against incomplete knowledge.

Two options:

1. **Keep `adoc.graph.v2`.** Rely on new `kind` strings being ignorable by tolerant readers. But the reader is intentionally *not* tolerant of version drift, and "the kind field is just a string" is the kind of implicit contract that rots. A reader and an artifact could disagree about what kinds exist with no signal.

2. **Bump to `adoc.graph.v3`.** A new version string forces every project to rebuild on first V5 use; stale v2 artifacts are rejected loudly instead of read silently.

Option 2 is the established pattern (the artifact has versioned before, v1 → v2) and gives a clean, loud failure for stale artifacts at no extra code cost, because the rejection path already exists.

## Decision

`SUPPORTED_GRAPH_SCHEMA_VERSION` is bumped from `"adoc.graph.v2"` to `"adoc.graph.v3"`.

The bump is **additive only**. Every V0–V4 node and edge shape — page nodes, prose-block nodes, the existing four Knowledge Object kinds, `contains`/`reference`/relation edges — is byte-identical in v3. New `kind` values and new per-kind fields appear only on the seven new V5 kinds. A v2 artifact and a v3 artifact built from the same V0–V4 source differ only in the `schema_version` string.

Stale `adoc.graph.v2` artifacts are rejected by the existing reader via `DiagnosticCode::SchemaUnsupportedVersion`. No new "outdated artifact" diagnostic is introduced; the existing version-mismatch rejection is the mechanism.

The bump lands once, in V5.1, and covers all seven new kinds. V5.2–V5.8 add kinds and fields *within* `adoc.graph.v3` without further version changes — the version expresses "the V5 Expanded Object Set may appear here," not "this exact field set."

No other wire envelope changes. `adoc.search.v0`, `adoc.retrieval.v0`, `adoc.patch.v0`, `adoc.patch.check.v0`, `adoc.diff.v0`, `adoc.review.v0`, and `adoc.project.status.v0` all stay at their current versions. The **Search Artifact** stays at `adoc.search.v0` because its Embedding Composition formula is unchanged — but its stored `graph_artifact_hash` changes with the v3 bump, so the first V5 build of any project triggers a full re-embed.

## Consequences

Every existing project rebuilds its graph artifact once on first V5 use, and (if embeddings are enabled) re-embeds once. For local JSON artifacts this is cheap and expected; `dist/` is gitignored, so no commit discipline is imposed.

The bump is the bulk of V5.1's mechanical work: the schema-version constant, the hardcoded references in `domain/graph/`, roughly fifteen test assertions across the workspace, and seven golden `.graph.json` fixture files all move from `v2` to `v3` in lockstep. A `grep` for `adoc.graph.v2` after the bump must come back empty.

Because the bump is additive and lands before any new kind is emitted, the constraint slice (V5.1) and every later V5 slice add nodes to a v3 artifact that is already the supported version — new-kind emission never requires touching the version again within V5.
