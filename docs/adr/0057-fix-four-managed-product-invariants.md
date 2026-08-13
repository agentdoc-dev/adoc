# ADR-0057: Fix Four Managed-Product Invariants

- Status: Accepted
- Date: 2026-08-13
- Depends on: ADR-0056

## Context

The V10 red-team found four implementation choices that could diverge even after the product boundary amendment: multi-source Object ID collisions, immutable content versus changing managed state, overlapping authorization rules, and independence of human semantic review. The founder approved the recommended defaults on 2026-08-13.

## Decision

1. **Managed Object identity is workspace-qualified.** Human-readable Object IDs stay stable, but matching IDs, titles, hashes, or semantic similarity across sources never auto-merge objects. Suspected duplicates create an explicit reconciliation candidate; governed actions may keep distinct, link/alias, supersede, or explicitly merge/re-home while preserving provenance.

2. **Content versions are immutable; managed state changes are append-only events.** Governance, verification, effectivity, freshness, integrity, and connector synchronization transitions attach to immutable content versions. A state-only transition does not create a new semantic content version. The managed graph/read model must be reproducible from immutable versions, recorded state events, and recorded policy/contract versions.

3. **Authorization uses one deterministic precedence.** Validate identity/session/grant freshness first; apply system-level restrictions; enforce the current source-ACL ceiling; evaluate scoped AgentDoc grants and explicit restrictions; apply object/field visibility; then apply action-specific policy. A source ACL may narrow but never widen AgentDoc authority. More-specific explicit restriction wins over an allow. Expired grants/membership observations do not authorize. Consequential uncertainty fails closed.

4. **Human semantic review can require independence.** Low-risk/advisory policy may permit author self-assessment. When policy requires independent semantic review, the change author/requesting principal and the qualifying semantic reviewer must be distinct. Human semantic review and proposal approval remain separate authorities; self-assessment cannot satisfy an explicit independent-review obligation.

## Consequences

Cloud contracts must distinguish workspace identity, logical Object ID, immutable version ID, and Source Binding. State-machine tests must prove historical reconstruction. Authorization tests must cover conflicts, expiry, ACL freshness, and scope specificity. Human semantic-assessment records must carry reviewer principal and independence result so gate evaluation remains deterministic.
