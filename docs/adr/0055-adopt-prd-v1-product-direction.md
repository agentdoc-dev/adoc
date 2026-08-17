# ADR-0055: Adopt PRD v1.0 as the Accepted Product Direction

- Status: Accepted
- Date: 2026-08-11
- Superseded in part by: ADR-0056 (V1 boundary amendments, `docs/product/PRD-v1.1-amendment.md`)

## Context

PR #141 merged `docs/product/PRD-v1.0.md`: the v1.0 Cloud-first product
direction (Part I, carried unchanged from the 1.0 draft) with the v0.2
capability inventory subsumed under it (Part II, Appendices A–D). Before
merge the document was red-teamed by seven adversarial lenses with
refute-first verification; all twenty-four confirmed findings were
dispositioned — accuracy defects fixed and re-verified against `adoc`
0.3.4, governance holes closed by maintainer rulings now recorded in
§12/§14/§15, and the remainder resolved through the document's own
machinery (§34 risk register, §35 open decisions 16–20, §36 follow-up
items 12–13).

Seventeen plausible strategy-level findings remained: the Cloud V1 bet is
the product's only ungated bet, typed-knowledge cold-start, Free-tier
absorption of the wedge, absent competitive analysis, and the
required-gate trust sequencing. These are direction-level judgment calls,
not defects the document can self-resolve. The document's own README
gated further work (roadmap rework, citation migration, canonical rename)
on an explicit acceptance decision.

## Decision

1. The v1.0 product direction in `PRD-v1.0.md` is accepted. Part I is the
   normative product-direction contract; the locked V1 boundary stands
   unchanged.
2. The seventeen strategy-level findings were reviewed at acceptance and
   do not block it. They are standing strategy inputs, summarized in the
   Context above, not contract content; revisiting any of them is a new
   decision against the accepted boundary.
3. Acceptance is recorded in the repository: the PRD status line and the
   product README reference this ADR.
4. The README migration plan advances past step 1. Step 2 — reworking the
   implementation roadmap for the Cloud-first V1 boundary, including the
   §36 item 12 restage of RET-003 permission-aware retrieval and §27.1
   sensitive-access audit into V1 — is the next cycle of work.

## Consequences

- `PRD-v1.0.md` now holds precedence slot 3 (forward product direction, V1
  scope, capability reference) as an accepted contract, no longer a
  proposal. Shipped behavior and active roadmaps still win on conflict.
- ROADMAP-V9 is in known contradiction with the accepted contract on
  RET-003/§27.1 staging (V10 there, V1 here); the next roadmap must
  resolve §36 item 12 rather than inherit the V9 staging.
- The §36 item 13 direction claims (authored-carriers-only content
  hashing, closed per-kind field schemas) require their own ADRs before
  implementation.
- `PRD.md` v0.2 remains frozen as the bare `PRD §N` citation target; the
  canonical rename of the v1.0 document stays gated on the Appendix D
  citation migration (README steps 3–5).
