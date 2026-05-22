# ADR-0019: Source-Path Impact via an `impacts:` Field

## Status

Accepted.

## Context

PRD §9.2 ("Code Change Invalidates Docs") assumes a typed link between a verified Knowledge Object and the code files whose change should invalidate it. The V0 evidence model has a `source:` field on **Verified Claim**, but in practice authors use it for free prose — the billing pilot has values like `source: ledger schema migration 2026-04-28`. Pattern-matching changed git paths against arbitrary prose would be brittle and produce wrong matches, so source-path impact analysis has no working substrate in the V2 schema.

The PRD glossary itself anticipates this with planned fields `impacts: list` and `impacted_by: list` on Knowledge Objects. V3.3 cannot ship its product value (flag verified claims when their declared code changes) without an explicit, typed link.

## Decision

Slice V3.3 introduces a new opt-in field `impacts: [path1, path2, ...]` on `claim` and `decision` typed blocks. Values are repo-relative file paths parsed into a new `RelPath` value object that rejects absolute paths, paths containing `..` segments, and empty strings. The field is non-empty when present, deduplicated, and sorted at parse time so graph emission is deterministic. Two new diagnostic codes — `schema.impacts_invalid_path` and `schema.impacts_empty` — flow through the existing `CompileResult.diagnostics` pipeline; compile stays infallible per the V0 pattern.

The graph artifact emits `impacts: [...]` as a node field on `claim` and `decision` Knowledge Object nodes, included in the canonical-JSON input that produces `content_hash`. A new internal port `ChangedFilesProvider` returns the changed file set for a given base ref versus the head workdir via `git diff --name-only`. The domain function `compute_impact(diff: &ObjectDiff, changed: &[RelPath]) -> Vec<ImpactedObject>` flags any verified claim whose `impacts` array intersects the changed-file set; the result feeds the V3.3 `adoc.review.v0` envelope.

Matching is strict file-path equality. Globs, prefix matches, and directory-level rules are explicitly out of scope until measured user pain demands them.

## Consequences

Impact analysis becomes a clean, testable domain projection: paths in, `ImpactedObject` records out. The opt-in posture means existing pilot claims keep working unchanged — only authors who declare `impacts:` get impact-driven proof obligations. The `source:` field stays exactly as it is (free prose evidence), so the two concerns — *what counts as evidence* and *which code paths invalidate this object* — stay orthogonal.

The new `RelPath` value object and the two new `DiagnosticCode` variants are public-surface additions, so the V3.3 slice ships them with positive and negative fixtures and a contract test against the `adoc.graph.v2` schema. Future iterations may add globs, but only after the strict-match version has shipped and a real use case demands the extra grammar.
