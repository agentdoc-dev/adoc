# ADR-0027: Source Objects Coexist With Inline Evidence

**Status:** Accepted
**Date:** 2026-06-02
**Slice:** V5.7

## Context

The V5 Expanded Knowledge Model roadmap introduces a `source` Knowledge Object — a reusable evidence pointer that names a single external artefact (a source file, test, URL, commit, incident, dataset, etc.) and explains, in prose, what it contains. Two design questions arise:

1. **Does `source` replace inline evidence?** Existing `claim` and `decision` objects already carry inline V0 evidence fields (`source:`, `test:`, `reviewed_by:`) that hold literal strings. Introducing a first-class `source` object could be read as deprecating those fields and forcing a migration.

2. **Does the evidence kind constrain the target?** A `source` requires exactly one of `path` (repo-relative) or `url` (absolute). Some evidence kinds are intrinsically file-shaped (`source_code`, `test`) while others are intrinsically web-shaped (`pull_request`, `issue`, `external_url`). Should the kind restrict which target is permitted, or is the bare path-XOR-url invariant enough?

## Decision

**1. `source` objects coexist with inline evidence; inline evidence is NOT deprecated in V5.7.** Inline V0 evidence fields on `claim` and `decision` continue to parse byte-identically and remain fully supported. A `source` object is an *additional* way to model evidence, not a replacement. Referencing a `source` object by Object ID from inline evidence is an opt-in upgrade whose resolution lands in **V5.8** (`evidence_ref:`); V5.7 ships only the standalone object. Whether inline evidence is eventually superseded is an open V5.10+ question, deliberately left open here.

**2. The evidence kind constrains the target (kind↔target correlation).** Beyond the bare path-XOR-url invariant, `EvidenceKind` classifies each kind as path-only, url-only, or either, and a mismatch emits `schema.source_kind_target_mismatch`. This catches authoring mistakes (e.g. an `external_url` source given a repo path) at compile time. The taxonomy is a V5.7 judgment call and is **revisable** — the "either" bucket is intentionally large so only unambiguous kinds are constrained.

| Target requirement | Evidence kinds |
| --- | --- |
| Path only | `source_code`, `test` |
| URL only | `pull_request`, `issue`, `external_url`, `runtime_metric`, `incident`, `support_ticket`, `experiment` |
| Either | `commit`, `design_doc`, `human_review`, `api_schema`, `audit_record`, `policy_reference`, `dataset` |

## Consequences

- The `source` aggregate has required fields `id`, `kind` (an `EvidenceKind`), exactly one of `path` (`RelPath`) or `url` (`Url`), and `body`. Optional fields `owner`, `symbol`, `commit`, `last_seen_at`, `hash` pass through unchanged.
- The path-XOR-url invariant is enforced in the constructor: both present emits `schema.source_conflicting_path_and_url`; neither emits `schema.source_missing_path_or_url`.
- The `Url` value object parses with the `url` crate and rejects schemes outside the existing `url_safety` allowlist (`http`, `https`, `mailto`).
- `source` has **no lifecycle status** — its graph node `status` slot is null. The evidence kind and the path/url are projected into the graph `fields` map under keys `kind`, `path`, `url`. This is additive within `adoc.graph.v3`; no envelope version bump (ADR-0028).
- The renderer emits an evidence-kind badge, the path (as code) or URL (as a safe link), and the prose body.
- New diagnostic codes: `schema.source_missing_kind`, `schema.source_invalid_kind`, `schema.source_missing_path_or_url`, `schema.source_conflicting_path_and_url`, `schema.source_invalid_path`, `schema.source_invalid_url`, `schema.source_kind_target_mismatch`. Invalid id and missing body reuse the shared `IdInvalid` / `SchemaMissingField` codes.
- New Agent Guidance Resource `adoc://agent/v0/source-guide`.
- Source-object reference resolution in inline evidence (V5.8), source-object impact analysis, and the V5 Pilot fixture (V5.9) are deferred.
