# ADR-0017: Deterministic Embedding Provider

## Status

Accepted.

## Context

The original hash-based embedding adapter existed as test-only "in-memory" infrastructure. That made hermetic tests possible, but it left offline/local deterministic behavior hidden behind test feature flags and debug seams.

Agents and local users need a production-configurable provider that can build and query search artifacts without model downloads, while still seeing that quality is lower than a semantic embedding model.

## Decision

Promote the hash-based adapter as the **Deterministic Embedding Provider**, selected by `embeddings.provider: deterministic`. It emits a stable search model header:

```json
{ "provider": "deterministic", "id": "hash-v1", "dim": 384 }
```

Build, retrieval load, and query embedding paths are provider-aware so deterministic corpus vectors and deterministic query vectors use the same model header. The old test env value remains only as a compatibility alias for tests; production configuration uses `deterministic`.

Project Status reports semantic readiness when deterministic graph/search artifacts are valid, but includes a warning diagnostic explaining that deterministic vectors are repeatable/offline and non-semantic.

## Consequences

Offline and CI flows can use deterministic search artifacts through normal configuration, not hidden debug behavior.

The provider is useful for repeatability, smoke tests, and constrained environments. It is not a substitute for semantic model quality, and agent-facing status must keep that warning visible.
