# ADR-0018: V3 Review Architecture

## Status

Accepted.

## Context

V3 brings AgentDoc into pull-request workflows. The PRD §9.2 ("Code Change Invalidates Docs") and §9.3 ("Agent Proposes a Doc Patch") describe two adjacent capabilities: an object-level **Object Diff** between two versions of a project, and an enriched **Review Report** that adds impact analysis, owners, proof obligations, and optional patch composition. Without these, V3's product bet — "turn object-level changes, proof obligations, and ownership into useful pull-request feedback" — has no concrete tracer-bullet.

Two architectural choices dominate the cost of every V3 slice and must be locked before any code lands: the diff input strategy (compare committed artifacts versus recompute graphs from `.adoc` source at each ref) and the envelope strategy (single growing schema versus separate pure-and-enriched envelopes). Getting either wrong forces a schema break later or pulls infrastructure decisions into domain code.

## Decision

Recompute graphs from `.adoc` source at each git ref, not compare pre-built artifacts. The driving adapter checks out a temporary linked git worktree (`git worktree add --detach`) and the existing `FsSourceProvider` reads `.adoc` files from that path, so `compile_workspace` runs twice and the existing compile pipeline is unchanged. RAII cleanup runs `git worktree remove` on drop. A single new internal port, `SnapshotWorkspaceProvider`, hides this from the application layer.

V3 ships two stable wire envelopes. `adoc.diff.v0` is the pure mechanical object diff — `{ created[], deleted[], changed[] }` over Knowledge Object scope only, sorted by Object ID, with full before/after `KnowledgeObjectRecord`s on changed entries. `adoc.review.v0` is the enriched review report — diff plus `impact[]`, `required_reviewers[]`, `proof_obligations[]`, and an optional `patch_check` field. Two envelopes, two CLI commands (`adoc diff`, `adoc review`), and two MCP tools (`adoc_diff`, `adoc_review`). New fields added during V3.4–V3.7 are JSON-optional with empty defaults so the schema version stays `v0` across the milestone.

The work is split into seven vertical slices: V3.1 Diff (domain types, port, git-worktree adapter, CLI, envelope), V3.2 Field-Level Projection, V3.3 Source-Path Impact and Required Reviewers (introduces the `impacts:` field on Knowledge Objects and the `ChangedFilesProvider` port — see ADR-0019), V3.4 Proof Obligations (reuses the promoted `ProofObligation` type — see ADR-0020), V3.5 CI Markdown Output, V3.6 MCP Surface, and V3.7 Patch Composition.

Enterprise error handling follows the existing project pattern. Compile remains infallible — schema problems in V3.3's `impacts:` field become `DiagnosticCode` variants flowing through `CompileResult`. System failures (missing `git` binary, unresolvable ref, worktree creation failure, patch parse failure) become hand-rolled `#[non_exhaustive]` error enums layered `GitError` → `SnapshotError` → `ReviewError`, each with structured fields, `std::error::Error::source()` chains for wrapped causes, and mapping at layer boundaries so lower-layer errors never leak past the port.

## Consequences

The diff is always accurate against current source even when authors keep `dist/` gitignored; no commit discipline is imposed on users. The git-worktree adapter is the only new infrastructure module — domain and application layers stay pure and reuse the V0 compile pipeline unchanged.

Two envelopes mean two contract-tested schemas under `docs/agent/v0/schema/`, but consumer agents that only need pure diff data pay for nothing more, and the enriched review surface evolves additively across V3.4–V3.7 without bumping schema versions. Agent prompts pinned to `adoc.diff.v0` and `adoc.review.v0` per ADR-0014 stay stable through the whole milestone.

`SnapshotWorkspaceProvider`, `ChangedFilesProvider`, the new `domain/review/` aggregate family, the promoted `domain/obligation.rs` module, the new `domain/knowledge_object/` field on `Claim` and `Decision`, and the new `infrastructure/git/` adapter module all land inside `adoc-core` — no new crate, matching the V2 patch precedent.
