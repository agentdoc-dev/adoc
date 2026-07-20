# ADR-0048: Deterministic Evidence Anchoring via `source` hash

**Status:** Accepted
**Date:** 2026-07-20
**Slice:** V8.5.1

## Context

A `source` Knowledge Object documents `hash`, `symbol`, `commit`, and
`last_seen_at` as optional fields (`docs/agent/v0/source-guide.md`), but
nothing consumes them — they are inert metadata. The only code-to-knowledge
drift signal the system has is path-level: `adoc impacted-by` (ADR-0019,
V6.3) reports that a path a claim cites appeared in a git diff. It cannot
say whether the *content* of a cited file changed since the claim was
verified — a drift merged weeks ago is invisible to any reasonable `--ref`
window.

The V8.3 CI surface posts `adoc check --format markdown` on every pull
request, so a check-time diagnostic reaches the whole team with zero new
workflow. Partners asking for LLM-free contradiction detection get, honestly,
the deterministic ceiling: detect that cited bytes changed and force a human
re-verification — never a semantic judgment.

## Decision

1. **The `hash` field on a path-target `source` object becomes an Evidence
   Anchor**: a `sha256:` + 64-lowercase-hex hash of the cited file's full
   raw bytes at verification time — the exact format `sha256_prefixed`
   already emits for **Base Hash** values. `shasum -a 256 <file>` produces
   it with standard tooling.
2. **Anchors are verified at check time** by an I/O-bearing application pass
   (`application/evidence_anchor.rs`) behind a new internal port
   (`EvidenceFileReader`, fs adapter in `infrastructure/source/`). Only the
   check entry point threads the anchor root; build, review, diff, and
   patch-apply recompiles stay anchor-free, so review worktree snapshots and
   pinned fixtures are unaffected.
3. **Four warning codes**, never errors, never gates (ADR-0038 posture —
   signals are data):
   - `evidence.hash_drift` — file readable, computed hash differs; help
     carries expected and actual so the fix is copy-paste.
   - `evidence.hash_target_missing` — anchored path absent or unreadable.
   - `evidence.hash_invalid` — value not `sha256:` + 64 lowercase hex; help
     carries the file's actual hash when readable (the bootstrap path:
     author a placeholder, copy the real value from the diagnostic).
   - `evidence.hash_unverifiable` — `hash` on a url-target source; URLs are
     not verifiable offline, and silent inertness is the disease this
     feature cures.
4. **Opt-in per source object.** No `hash` field means no diagnostics and no
   file reads — existing corpora compile byte-identically.
5. **Anchor root**: the discovered project-config directory, else the
   context start directory. `adoc check` runs config discovery even when an
   explicit docs path is passed (the explicit path still wins for docs
   resolution), so anchors resolve identically however check is invoked —
   the same walk-up seam `impacted-by` uses for git discovery. A malformed
   config found during the walk fails check loudly rather than being
   ignored. Cited paths are repo-relative in the same sense
   `git diff --name-only` output is.

## What this can and cannot claim

An anchor mismatch means *the cited bytes changed since anchoring* — changed,
not wrong. The semantic judgment (does the change invalidate the claim?)
stays with a human or an explicitly model-assisted step outside the
deterministic core, extending the ADR-0026 posture: no NLP, no model, in the
compile pipeline, ever. Whole-file granularity also flags unrelated edits to
the cited file; that noise is priced in (see Rejected).

## Rejected

- **Line-span anchors** — positionally brittle: any edit above the span
  drifts it, demanding constant maintenance for zero added truth.
- **`symbol` resolution** — needs language parsing. A naive byte-search
  presence check never false-positives but false-negatives when the name
  survives in a comment or string; recorded as a possible later warning, not
  shipped.
- **`commit` verification** — would drag git into `adoc check`, which is
  git-free by design.
- **URL anchoring** — offline-unverifiable; warned, not checked.
- **A dedicated signal command/envelope** — the actor is the *author* (the
  remedy is a source edit), and check is the source-facing surface already
  on every PR. The graph artifact already carries source-object fields, so
  a future artifact-side consumer needs no schema bump. Revisit on agent
  demand.

## Relationships

- **ADR-0034 (evidence quality)**: an anchor does not change an evidence
  tier in this slice; "anchored ⇒ stronger" is named, not shipped.
- **ADR-0036 (`patch.source_drift`)**: distinct axis — that code is
  artifact-vs-`.adoc`-source freshness during patch apply;
  `evidence.hash_drift` is cited-code-bytes-vs-authored-anchor.
- **ADR-0038**: why these are check warnings and not a fifth read command.
- **ADR-0019**: anchoring complements `impacts:`/evidence-path matching —
  the path answers "is it in this diff", the anchor answers "did the bytes
  move since verification".

## Consequences

- `adoc check` reads non-source repo files for the first time — opt-in only,
  deterministic, bounded by the count of anchored sources (one read per
  distinct path).
- Every legitimate change to a cited file costs a hash update. That friction
  is the feature: it *is* the re-verification prompt.
- Warning budgets in pilots gain a new possible contributor; fixtures pin
  exact budgets, so any change is a visible test edit.
