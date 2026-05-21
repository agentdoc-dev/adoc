# ADR-0016: Core Artifact Readiness Inspectors

## Status

Accepted.

## Context

`adoc_project_status` must answer whether local artifacts are ready for retrieval, semantic search, and patch validation. A raw JSON inspection in `adoc-local` can check existence and schema strings, but it duplicates artifact knowledge and can miss semantic invalidity such as invalid Object IDs, model mismatches, or graph hash drift.

Graph and Search artifact readiness is domain read-side behavior, even when surfaced through local project status.

## Decision

Add public `adoc-core` read-side inspectors:

- `inspect_graph_artifact(GraphArtifactInspectionInput) -> ArtifactInspection`
- `inspect_search_artifact(SearchArtifactInspectionInput) -> ArtifactInspection`

The shared inspection output reports `path`, `exists`, `load_status`, `schema_version`, `object_count`, and `diagnostics`. The inspectors use existing artifact readers, `GraphIndex` validation, active model-header validation, graph-hash drift checks, and typed diagnostics. Graph/Search artifact DTOs remain internal.

Refactor `adoc-local` Project Status so it resolves config and paths, optionally runs refresh, delegates readiness inspection to `adoc-core`, and computes the final readiness booleans.

## Consequences

Readiness semantics live with the artifact readers and graph/search validation code that define them.

`adoc-local` remains an orchestration layer. It no longer interprets graph/search JSON by hand, and MCP/CLI adapters can share the same readiness behavior without learning private artifact DTOs.
