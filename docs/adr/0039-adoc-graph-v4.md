# ADR-0039: `adoc.graph.v4` — Kind Expansion and Status-Slot Cleanup

**Status:** Accepted
**Date:** 2026-07-02
**Slice:** V6.5.1 (TB1); the version covers all four V6.5 kinds (`api`,
`observation`, `question`, `task`) per the ADR-0028 bump-once precedent

Fills the number reserved by ROADMAP-V6.md and noted in ADR-0041; recorded
after ADR-0041 by design — V7.1 landed before V6.5.1.

## Context

ADR-0028 established the graph-artifact versioning discipline: one constant
(`SUPPORTED_GRAPH_SCHEMA_VERSION` in `infrastructure/artifact/graph_json.rs`),
written on emit, exact-matched on read, bumped once per cycle covering all new
kinds. ADR-0035 documented the `status`-slot overload (Severity riding
`status` on `warning`/`constraint`, Trust on `agent_instruction`,
contradiction severity living in `fields["severity"]`), shipped a dual-emit
stopgap within v3 (derived, unhashed top-level `severity`/`trust`), and
planned a v4 cleanup in its §5.

V6.5 adds four kinds. New `kind` strings in the payload demand a loud version
bump, and a bump forces a full rebuild and re-embed anyway — so this is the
cheapest moment to execute the planned cleanup.

## Decision

**Bump to `adoc.graph.v4`. The `status` slot is lifecycle-only.
`severity`/`trust` become the sole, authored, hashed carriers.**

1. `status` is absent for kinds without a lifecycle: `warning`, `constraint`,
   `agent_instruction` (and `source`, unchanged). The `MetadataDiscriminant`
   projection loses its `Severity`/`Trust` variants; the four V6.5 kinds are
   born under the lifecycle-only rule.
2. Top-level `severity` (on `warning`/`constraint`/`contradiction`) and
   `trust` (on `agent_instruction`) switch from derived dual-emit to sole
   carriers of the authored values, and **enter `KnowledgeObjectHashPayload`**
   as optional fields serialized only when present — so the hash payload (and
   therefore `content_hash`) of every kind that carries neither is
   byte-identical to v3.
3. Contradiction severity collapses from three locations to one: the
   `fields["severity"]` copy disappears from the artifact; the top-level
   `severity` is the authored, hashed home.
4. **v4 is not purely additive the way v3 was.** Node contents change for
   exactly four kinds: `warning`, `constraint`, `agent_instruction` (status
   slot vacated, carrier fields now hashed) and `contradiction` (severity
   carrier collapsed). Their `content_hash`/`base_hash` values regenerate on
   first build; in-flight patches against old hashes fail loudly via the
   existing base-hash mismatch — designed behavior. The additive invariant is
   scoped to untouched shapes, proven by goldens.
5. The review/diff projection re-keys its reads from the `status` slot to the
   dedicated `severity`/`trust` node fields: three independent diffs
   (`status` → `FieldChange::Status`, `severity` → `FieldChange::Severity`,
   `trust` → `FieldChange::Trust`), retiring the kind-keyed re-labeling.
   **The envelope half of ADR-0035 §5 is explicitly re-deferred**: typed
   `FieldChange` payloads do not ship; `adoc.diff.v0` and `adoc.review.v0`
   keep their string payloads and stay at v0. ADR-0035 §5 coupled the re-key
   "alongside typed `FieldChange` payloads"; this ADR resolves that coupling
   in favor of the split, so the deferral is a decision, not an accident.

## Behavior deltas (enumerated, all deliberate)

- **Warning re-label quirk fixed.** A `warning` severity delta now emits
  `FieldChange::Severity` (v3 emitted `FieldChange::Status`, the ADR-0035
  known quirk). The re-key makes the three carriers symmetric.
- **Contradiction severity becomes diff-visible.** In v3 it lived only in
  `fields` and the projection has no generic-fields diff, so a severity edit
  was invisible to diff/review. It now emits `FieldChange::Severity` — and
  consequently trips the existing any-field-change-on-unresolved-contradiction
  re-assert obligation. That is a bug fix, not a regression.
- **`--status` no longer matches severity/trust strings.** `adoc search
  --status critical` matched warnings in v3 by accident of the overload; in
  v4 it matches nothing for the three kinds. This is the designed PRD §18.4
  lifecycle semantics. No severity filter is added; add one when a real
  consumer asks.
- **Embedding composition is unchanged.** `embedding_input` keeps its
  `[status: …]` line by falling back status → severity → trust, so embedding
  text is byte-identical to v3 and the forced full re-embed comes only from
  the `graph_artifact_hash` change, not from a composition change.
- **Envelopes stay v0 with visible record-content deltas.** Retrieval, diff,
  review, and stale records embed graph nodes; fresh records lose the `status`
  key for the three kinds and lose `fields["severity"]` on contradictions.
  All four envelopes are tolerant-reader by contract (the ADR-0035 precedent
  in reverse: keys may disappear as well as appear); the versioned artifact
  underneath is what bumped. The published envelope schemas gain the optional
  `severity`/`trust` keys they were missing since ADR-0035.
- **The patch-draft status slot keeps its own overload.** `adoc.patch.v0`
  `create_object` drafts still map a draft `status` change onto Severity for
  warning drafts (`domain/knowledge_object/draft.rs`). That is the patch
  envelope's seam, not the graph's; it stays v0 and is untouched. Noted so
  the asymmetry is a recorded decision.

## Consequences

- First `adoc build` after upgrading rejects nothing but regenerates
  everything: `graph_artifact_hash` changes, forcing a full re-embed; old
  artifacts on disk fail the exact-match version gate with
  `SchemaUnsupportedVersion`, per ADR-0028.
- Golden fixtures for the four affected kinds change contents, not just the
  version string; goldens for untouched kinds change only the version string —
  both facts are asserted, which is what scopes the additive invariant.
- The scoped grep — `grep -r adoc.graph.v3 crates/ examples/ README.md
  docs/agent/` — returns zero hits after TB1; `docs/adr/`, `V*-DESIGN.md`,
  and ROADMAP history keep their historical mentions.
- V6.5.2–V6.5.4 add their kinds within v4. A later kind slice discovering it
  needs a graph shape change is a design failure to escalate, not a second
  bump.
