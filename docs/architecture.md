# Current Architecture

This document is the current implementation map for AgentDoc. Historical
design documents and ADRs record the architecture at the time of their
decision; when a module path differs, this document and ADR-0046 describe the
active boundary.

## Workspace Boundaries

```text
adoc-cli  ----\
               -> adoc-local -> adoc-core
adoc-mcp  ----/
```

- `adoc-core` owns domain policy, source-language mechanics, application
  workflows, and concrete core adapters behind a narrow public facade.
- `adoc-local` is the protocol-free local workflow facade. It owns project
  config, path policy, exit-code policy, and filesystem publication.
- `adoc-cli` and `adoc-mcp` are driving adapters. They translate transport
  inputs and present `adoc-local` outcomes; they do not duplicate workflows.

## Core Dependency Direction

`adoc-core` follows this inward dependency rule:

```text
application -----> domain <----- infrastructure
     |
     +-----------> language ----> domain

lib.rs -> application + infrastructure (composition root)
```

- `domain/` contains aggregates, value objects, invariant policy, typed
  projections, and ports. It does not import `application`, `language`, or
  `infrastructure`.
- `language/` contains pure AgentDoc parsing, validation registries, HTML and
  source rendering, and graph projection. These mechanics depend on domain
  types but perform no filesystem, Git, process, or runtime-provider I/O.
- `application/` coordinates typed workflows. It may use domain ports and
  pure language functions, but production code does not import concrete
  infrastructure modules.
- `infrastructure/` contains filesystem source adapters, Git/worktree
  adapters, artifact JSON readers/writers, and embedding providers.
- `lib.rs` selects concrete adapters and exposes the Public Core Surface.

Architecture tests in `crates/adoc-core/tests/public_surface.rs` recursively
enforce these dependency rules.

## Compilation Flow

Compilation separates analysis from emission:

```text
SourceProvider
  -> parse and validate
  -> resolve typed Knowledge Objects and references
  -> WorkspaceAnalysis
  -> typed graph/search projections
  -> HTML and JSON encoders
  -> BuildArtifacts
```

`WorkspaceAnalysis` is the internal typed handoff. Review and patch-apply
workflows consume typed projections directly; JSON is a boundary format, not
an internal transport. Serialization failures become diagnostics rather than
panics.

Graph and search construction remain concrete because format polymorphism has
no second production implementation. `ArtifactWriter` was removed by
ADR-0046. Ports are retained where they isolate observable I/O or runtime
choice: source loading, artifact reads, embeddings, Git changed-file and
snapshot access, committed-source checks, and workspace writes.

## Knowledge Object Construction

`domain/services/resolve_pending_block.rs` is the canonical Pending-to-Typed
construction path for every supported Knowledge Object kind. Parsing and
patch draft validation both use it, so field grammars and aggregate
invariants cannot drift between authored source and proposed patches.

`domain/knowledge_object/field_decoder.rs` owns scalar trimming, list syntax,
empty/trailing segment policy, malformed-bracket failures, and Unicode-aware
field spans. Aggregates map those decoded values into their own diagnostic
codes and domain types.

## Local Workflow Facade

`LocalContext` remains the stable API shared by CLI and MCP. Its implementation
is split by command family:

- `use_cases/project.rs`: init, check, migrate, build, and project status.
- `use_cases/queries.rs`: why, graph, search, stale, contradictions, impacted.
- `use_cases/changes.rs`: diff, review, patch check, and patch apply.
- `use_cases/shared.rs`: config, artifact-path, and diagnostic policy shared by
  command families.
- `use_cases/artifact_commit.rs`: transactional build-output publication.

The build publisher stages and fsyncs every artifact beside its destination,
then promotes the set while retaining backups. A failed promotion rolls back
all earlier promotions. Failure-injection tests cover both replacement of an
existing set and first publication of a new set. Cross-process locking and
crash-consistent multi-file atomicity remain explicit non-goals; each rename
is filesystem-atomic, and handled failures restore the prior complete set.

## Testing And Change Discipline

- Pure domain and language behavior uses inline unit tests.
- Public crate contracts and architecture boundaries use integration tests.
- Filesystem, Git, artifact, and adapter failures are tested at their owning
  boundary with injected collaborators where failure order matters.
- Refactors land as independently green vertical slices. Each slice runs
  formatting, workspace clippy with warnings denied, and workspace tests
  before a Conventional Commit.
