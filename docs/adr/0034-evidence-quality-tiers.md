# ADR-0034: Evidence Quality Tiers (`evidence_quality`)

**Status:** Accepted
**Date:** 2026-06-03
**Slice:** V5.10 TB3

## Context

Not all evidence kinds carry the same epistemic weight. A `test:` entry
(machine-executed) is a stronger guarantee than an `external_url:` entry (a
web page that may change or disappear). PRD §15.3 defines a quality tier
classification that enables the toolchain to surface weak evidence as a
warning rather than silently accepting it.

The tier mapping must be decided once and recorded in a discoverable location
so that future variant additions trigger a mandatory decision (exhaustive match,
no wildcard).

## Decision

### Tier enum

`EvidenceTier { Low, Medium, High }` is added to
`domain/value_objects/evidence_kind.rs`. `Ord`/`PartialOrd` are derived with
the variant declaration order Low → Medium → High so that `High > Medium > Low`.

### Tier mapping (§15.3)

| Tier   | Evidence kinds |
|--------|---------------|
| High   | `Test`, `SourceCode`, `ApiSchema`, `PolicyReference`, `AuditRecord` |
| Medium | `HumanReview`, `DesignDoc`, `PullRequest`, `Incident`, `Commit` |
| Low    | `ExternalUrl`, `Issue`, `SupportTicket`, `RuntimeMetric`, `Dataset`, `Experiment` |

The mapping is implemented as an **exhaustive match** in
`EvidenceKind::quality_tier(self) -> EvidenceTier` with no wildcard arm. Any
new `EvidenceKind` variant added in the future will produce a compile error
until a tier decision is recorded.

### Derived `evidence_quality` projection

A new field `evidence_quality: Option<String>` is added to
`GraphKnowledgeObjectNode` and mirrored on `RetrievalRecord`.

- Value: the `as_str()` of the best (highest) `EvidenceTier` across all
  evidence entries whose `kind` string parses to a known `EvidenceKind`.
- Strings: `"high"`, `"medium"`, or `"low"`.
- `None` when the object has no evidence, or all entries have an unrecognised
  kind string.
- ObjectRef entries (from `evidence_ref:`) carry the kind string of the
  referenced source object; they are treated identically to inline entries for
  the graph-node projection.

**Not authored, not hashed, purely additive.** The field is excluded from
`KnowledgeObjectHashPayload` so `content_hash` is identical regardless of the
field's value. `serde(skip_serializing_if = "Option::is_none")` ensures
existing fixtures without evidence remain byte-stable.

### Diagnostic: `claim.evidence_quality_low`

`DiagnosticCode::ClaimEvidenceQualityLow` (wire: `"claim.evidence_quality_low"`,
severity: **Warning**) is emitted by `ClaimEvidenceQualityLowRule` when:

1. The claim's status is exactly `"verified"`.
2. The claim has at least one inline evidence entry (no
   `ClaimVerifiedMissingEvidence` double-warning).
3. The claim has **no** `ObjectRef` evidence (`evidence_ref:` entries).
4. The best tier across all inline evidence kinds is `Low`.

### ObjectRef counts as ≥ Medium

`evidence_ref:` entries point to `source` Knowledge Objects that have passed
full schema validation (structural review). We conservatively treat any
`ObjectRef` as implicitly ≥ Medium quality for the purpose of this warning.
This means a verified claim that has at least one `evidence_ref:` will never
trigger `claim.evidence_quality_low`, even if the referenced source's kind is
Low-tier (e.g. `external_url`).

**Rationale:** The alternative (counting the referenced source's kind tier)
would require cross-object resolution at validation time and would make the
warning fragile to source object renaming. The simple guard is conservative,
documented, and can be tightened in a future slice.

## Consequences

- `EvidenceKind::quality_tier()` must be updated for every new variant.
- `GraphKnowledgeObjectNode` gains `evidence_quality: Option<String>` after
  `effective_reason` (consistent with ADR-0033 field ordering).
- `RetrievalRecord` gains the same field as a clone-through.
- `KnowledgeObjectHashPayload` is **unchanged** — no hash churn.
- Existing fixtures without evidence remain byte-identical.
- New verified claims backed only by low-quality inline evidence will receive a
  Warning. Authors can silence it by adding a high/medium-tier evidence entry or
  an `evidence_ref:` to a structured source object.

## Alternatives considered

- **Per-kind weight score (float).** More granular but harder to explain to
  authors and harder to evolve. Three discrete tiers match the PRD language.
- **Counting ObjectRef tier from the referenced source's kind.** Rejected for
  now; requires cross-object resolution and adds fragility. Documented for
  future consideration.
- **Error instead of Warning.** Rejected: existing verified claims with only
  Low-tier evidence are not wrong, just suboptimal. A warning is actionable
  without being a gate.
