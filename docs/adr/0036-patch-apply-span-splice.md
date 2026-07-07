# ADR-0036: Patch Application as Formatting-Preserving Span Splice

**Status:** Accepted
**Date:** 2026-06-12
**Slice:** V6.4 (TB1–TB3; recorded at slice start per the [ROADMAP-V6.md](../roadmap/ROADMAP-V6.md) ADR inventory)

## Context

V2 closed the propose half of the agent contract: `adoc patch --check` validates
all five op families (`update_fields`, `replace_body`, `create_object`,
`supersede`, `revoke`) against the graph artifact and emits
`adoc.patch.check.v0`. Nothing applies a validated patch; agents and humans
hand-edit `.adoc` source, losing the validation discipline at exactly the step
that mutates knowledge.

Two ground truths shape any apply design:

1. **The graph artifact cannot drive splicing.** `ParsedTypedBlock.span` is the
   open-fence line only — the close-fence span is discarded at parse time — and
   `GraphSourceSpan` carries a start position with no byte length. There is no
   byte range for a block anywhere in `dist/docs.graph.json`.
2. **Parser columns are char-based; only `SourcePosition.offset` is a byte
   offset.** Reconstructing positions from columns corrupts multibyte files.

The roadmap rules for this cycle bind the design further: apply writes to the
working tree only (never Git), is formatting-preserving (byte-identical outside
the edited spans, no reformat-on-write), and never auto-reverts.

## Decision

Patch application is a **formatting-preserving span splice over a fresh parse
of current source**, orchestrated in `application/apply.rs` and emitted as a
new envelope `adoc.patch.apply.v0`.

1. **Pure splice engine in `domain/source_edit/`.** `SpanEdit { byte_range,
   replacement }` and `SourceEditPlan` (sorted, non-overlapping; the factory
   rejects overlap) with `splice()` copying every byte outside the edited
   ranges verbatim — formatting preservation holds by construction, not by
   test. All byte math uses `SourcePosition.offset`; char columns never enter.
2. **Parser span extension, behavior-preserving.** `ParsedTypedBlock` retains
   the close-fence span and the `--` body-separator span so a block's full
   byte range and body region are recoverable from a fresh parse. Splicing
   never consumes artifact spans.
3. **Two-layer freshness precondition.**
   - `base_hash` vs graph (existing V2 check, unchanged): proves the proposer
     saw the latest artifact.
   - Graph vs source (new, apply-time): apply recompiles the working tree in
     memory and refuses with the new diagnostic `patch.source_drift` unless
     the recompiled `content_hash` for the target equals the artifact's.
     `base_hash` alone cannot catch a stale artifact over moved-on source. The
     recompile also supplies the fresh spans the planner needs, so it is not
     extra cost.
4. **Atomicity without locking.** Writes go through a `WorkspaceWriter` port:
   per-file temp file in the same directory, write, fsync, then rename; the
   on-disk file is re-hashed immediately before the rename and apply refuses
   on mismatch (TOCTOU guard). Cross-process locking is an explicit non-goal.
5. **Post-check reported, never acted on.** After the rename, apply re-runs
   the compile/check pipeline in memory and embeds every resulting diagnostic
   in the envelope. AgentDoc never auto-reverts — the human and Git undo.
   Exit codes: `0` applied and post-check clean; `1` refused, nothing written;
   `2` applied but post-check reports new errors (agents must stop and surface
   to a human).
6. **Artifacts go stale loudly.** Apply never rewrites `dist/` artifacts; the
   envelope carries `artifacts_stale: true` and agents rebuild.
7. **`create_object` placement semantics** (resolving V2's open question):
   `placement.page_id` resolves to a file via the page node's `source_path`;
   `after: <id>` inserts immediately after that block's close fence; absent
   `after` appends at end of file. Created blocks render with deterministic
   sorted field order and one separating blank line. `placement` becomes
   optional on the wire — `patch.create_missing_placement` is a WARNING on
   `--check` and an ERROR on `--apply`; `patch.placement_not_adoc` rejects
   `.md` placement pages. New-file creation is deferred. `adoc.patch.v0`
   stays at v0 — these are additive.
8. **Refusals are data.** Validation failure, source drift, and placement
   errors return the same `adoc.patch.apply.v0` envelope with
   `applied: false`, empty `written_files`, and fix-oriented diagnostics —
   never a protocol error.

## Consequences

- Reviewers see a minimal Git hunk: the patch's target spans change,
  everything else is byte-identical, pinned by property tests (empty-plan
  identity, outside-range preservation, `ObjectDiff` exactness, multibyte
  boundary).
- Every apply costs an in-memory recompile (drift gate) plus a post-check
  recompile. Accepted: correctness over latency, and the drift recompile
  supplies the splice spans anyway.
- The drift gate compares `content_hash` values whose payload includes
  `source_span` paths, so apply must resolve the docs root through the same
  helper chain `check`/`build` use; a differently-spelled root refuses in the
  safe direction (rebuild and re-propose).
- `dist/` is stale by design after an apply; agents that keep reading without
  rebuilding get loud `base_hash` mismatches on their next proposal — the
  designed behavior.
- A second apply of the same patch fails on `base_hash` after rebuild (or
  `patch.source_drift` before one) and writes nothing — idempotence through
  refusal, not through detection.

## Alternatives considered

- **Splice from graph-artifact spans.** Rejected: spans are start-only with no
  byte length, and an artifact is stale the moment source moves on —
  positional staleness would corrupt files silently.
- **Re-serialize the file from the AST.** Rejected: reformat-on-write is
  banned by the cycle rules; authors own their formatting, and diffs must show
  only the semantic change.
- **Auto-revert when the post-check reports errors.** Rejected: AgentDoc does
  not decide to undo; `applied: true` + exit 2 is unmissable and Git is the
  rollback mechanism.
- **Rewrite artifacts after apply.** Rejected: `adoc build` is the only
  artifact producer; a half-updated `dist/` would break the artifact-readiness
  contract (ADR-0016).
- **Refuse when the target file has uncommitted Git changes.** Deferred per
  the roadmap: `base_hash` + source-drift + fresh compile is enough until real
  corruption reports say otherwise.
- **Cross-process file locking.** Rejected as a non-goal: temp+rename plus the
  pre-rename re-hash bounds the race window; locks add platform-specific
  failure modes for a single-writer workflow.
