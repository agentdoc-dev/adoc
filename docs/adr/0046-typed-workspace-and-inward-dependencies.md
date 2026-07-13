# ADR-0046: Typed Workspace Analysis and Inward Dependencies

**Status:** Accepted
**Date:** 2026-07-13
**Slice:** refactor/hexagonal-architecture

## Context

The four-crate workspace already has a narrow Public Core Surface, strong
domain types, and useful ports for sources, artifact reads, embeddings, path
policy, and workspace writes. The application layer nevertheless imports
concrete infrastructure modules for parsing, validation, rendering, artifact
construction, filesystem inspection, and Git operations.

Compile also exposes serialized artifacts too early inside the core. Review
and apply compile a workspace to graph JSON and immediately deserialize that
JSON to recover typed state. Those round trips add work, make JSON schema
details part of application orchestration, and turn serialization assumptions
into `expect` calls.

ADR-0006 described `ArtifactWriter` as an extensibility seam. In practice it
has one production implementation, the production compilation path bypasses
the trait, and its only alternate implementation is a structural test double.
Deleting it removes interface complexity without redistributing behavior.

## Decision

### 1. Dependency direction

The core uses these dependency rules:

1. `domain` owns business concepts, invariant policies, value objects, and
   narrow ports. It does not depend on application or infrastructure.
2. Pure AgentDoc source-language mechanics may live in a private `language`
   module. Application code may call these pure modules directly when an
   interchangeable adapter would be hypothetical.
3. `application` coordinates domain behavior through domain ports and pure
   language modules. It does not import concrete filesystem, Git, JSON,
   embedding, or artifact adapters.
4. `infrastructure` owns concrete adapters and may depend inward on the
   contracts and types it implements.
5. `adoc-core/src/lib.rs` is the core composition root. It selects concrete
   adapters and preserves the narrow Public Core Surface from ADR-0005.
6. `adoc-local` remains the local workflow facade shared by the CLI and MCP
   driving adapters. Exit-code policy remains local workflow policy.

An architecture test enforces the production application rule. Test-only
collaborators may use infrastructure adapters when they are testing an
adapter contract rather than application behavior.

### 2. Typed analysis precedes artifact emission

Compilation produces a private typed workspace analysis and projection before
any output format is selected. The typed result contains the validated
workspace, diagnostics, derived relationships, and deterministic hashes
needed by downstream application workflows.

The public compile and build functions compose analysis with output adapters
to preserve `CompileResult` and `BuildArtifacts`. Check stops after analysis
unless its public contract requires the existing HTML and graph strings.
Review and apply consume the typed result directly; they do not serialize and
deserialize graph JSON as an internal transport.

Graph, search, and HTML encoders remain concrete output adapters. Their JSON
DTOs are wire-format implementation types, not application aggregates.
Serialization failures are returned and mapped to stable diagnostics instead
of panicking.

### 3. Ports require behavioral leverage

Keep ports that isolate I/O, runtime selection, failure behavior, or multiple
real adapters. This includes `SourceProvider`, `ArtifactReader`,
`EmbeddingProvider`, path policy, workspace writes, changed-file discovery,
and snapshot loading.

Remove `ArtifactWriter`. Graph and search construction stay concrete until
there are two genuinely interchangeable implementations with the same
behavioral contract. Output persistence may define a separate artifact-set
commit seam because staging, rename, cleanup, and failure recovery are
observable behaviors rather than format polymorphism.

Git cleanliness and snapshot source loading are accessed through narrow ports.
The Git adapters own process execution, temporary checkout lifecycle, file
loading, and cleanup. Artifact readers report typed missing, unreadable,
malformed, and unsupported-version failures so application code does not
inspect paths or reverse-engineer diagnostic messages.

### 4. Existing architecture decisions remain in force

This ADR supersedes ADR-0006 only where it requires `ArtifactWriter`, places
artifact serialization in application orchestration, or treats concrete
renderers as application dependencies. It refines ADR-0007 by moving domain
validation policy inward while retaining separate parse and validation
passes. It refines ADR-0009's module placement without changing the tactical
domain model.

ADR-0045 remains fully accepted: no aggregate accessor trait is added over the
`KnowledgeObject` enum, no deep Public Core Surface entries are added for the
lifecycle signal queries, and no generic envelope presenter is introduced.

## Consequences

- Build output and public wire contracts remain stable while review, apply,
  and check can reuse typed analysis without output-format round trips.
- Application tests use domain ports or typed inputs. Adapter tests own
  filesystem, Git, serialization, and failure-injection coverage.
- Pure source-language logic is not forced behind single-implementation
  traits merely to satisfy a layered naming convention.
- Adding a new output format means adding a concrete adapter and composition
  wiring. A shared port is introduced only after common behavior is proven.
- The refactor lands as independently green vertical commits. Each commit
  includes the behavior tests and documentation needed to explain its change.

## Implementation Status

Implemented on 2026-07-13. The core now analyzes into a typed workspace before
emission; review and apply consume typed projections; Git and artifact reads
use typed ports; pure parser/validator/renderer code lives under `language/`;
and recursive architecture tests enforce inward dependencies.

`adoc-local` keeps `LocalContext` as its public facade while delegating to
focused project, query, change, and shared modules. Build outputs use the
separate artifact-set commit seam anticipated by this ADR: all files are
staged and fsynced before promotion, backups support rollback, and injected
staging/promotion failures prove that no handled failure leaves a partial new
set. See the [current architecture map](../architecture.md).
