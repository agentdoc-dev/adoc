# ADR-0071: The Proposal Review Read Grades the References It Returns

**Status:** Accepted
**Date:** 2026-10-02
**Slice:** E8.3 (registry row `agentdoc.cloud.proposal_review.v0`)
**Supersedes:** [ADR-0069](0069-proposal-review-read-rt08-exceptions.md) Decision 1

## Context

ADR-0069 Decision 1 let the E8.3.T1 proposal review read return two kinds of
reference ungraded, and kept the route out of every release stage until it
graded them:

- the proposed `evidence_ref` Source Object ids inside the patch changes;
- the `no_change_required` dispositions, whose findings' affected objects can
  lie outside the graded targets.

A third reference of the same kind was found while closing it: a
`create_object` patch's placement anchor (`placement.after`), which the write
guard requires to be an existing Knowledge Object on the target page.

E6.1 is implemented. Its visibility predicate has three halves: the authored
visibility class against the caller's retrieval audience, the `knowledge.read`
grant, and the source ACL of a managed version. A Proposal Record binds no
Source Record or ACL snapshot, so only the first two halves apply to it. The
read already grades its targets against the Graph Artifact Cloud stored at
admission, never against the live graph.

## Decision

1. **Source Objects and anchors.** Every `evidence_ref` item, whatever the
   patch kind, that names a `knowledge_object` node of the admission Graph
   Artifact is a referenced Source Object, and every placement anchor is
   graded the same way. A referenced node passes only when its authored
   `visibility` class (absent means `public`) is within the caller's retrieval
   audience (the caller's latest `managed_retrieval_policy_revisions` row,
   else `public`), and the caller holds `knowledge.read` on the node's object
   id and knowledge kind. The source-ACL half stays `not_applicable`, as the
   registry row already records for the read's other grades. Cloud's E6.1
   retrieval input derives `allowed_visibilities` from the audience and sends
   no `excluded_object_ids`, so the audience is the whole class half today; a
   policy that adds exclusions must apply them to this grade too.
2. **Dispositions.** Each disposition's finding is resolved in the semantic
   assessment stored with the same ingestion. Every affected object is graded
   with the read's own grade, `proposal.read` and `obligation.read`, as a
   target is. It gets no class check, because targets carry none.
3. **One answer.** Any failure withholds the whole envelope with the same
   codeless 404 as a miss. A reference the caller may not see logs nothing.
   An unknown visibility class, a node without a kind, an id naming more than
   one node, an unresolved finding or affected object, a missing evidence row,
   or bytes that do not decode also read as the 404, and Cloud logs
   `governance.proposal_record_integrity_failed` with a log-only location.
4. **Order.** On the requested set, the grade runs after the Proposal Record
   verifies, so the envelope it grades is the verified record, and the 409,
   which carries no envelope, keeps its place. A disclosed set's envelope has
   only passed its own snapshot re-derivation. The Graph Artifact and
   assessment it resolves against are the stored admission bytes, whose
   digests are write-time constraints that this read does not re-check. So the
   grade never raises on decodable JSON and treats any shape it cannot resolve,
   or bytes that do not decode, as a failure.
5. **Disclosed sets.** A successor's digest, or the digest of the set behind an
   invalidation, is disclosed only when that set also passes this grade. So no
   successor or invalidating digest names a set that the same caller reads as
   404. The envelope's own `supersedes` digest is part of the record bytes and
   stays ungraded, as before. ADR-0069 Decisions 2 and 3 stand: the read never
   answers 404 because of another set.
6. **Not references.** An `evidence_ref` item that names no node is not graded.
   On evidence-bearing patch kinds the write guard makes such an item
   impossible; on the other kinds the field is free text, like `reason`. Page
   ids and placement paths name source pages and paths, not Knowledge Objects,
   and are not graded; neither are `impacts` paths. An inline `[[object.id]]`
   reference in a proposed body is body text: Cloud does not resolve it at
   admission, so it confirms nothing the author did not write, and it is not
   graded. This is provenance-bound enforcement, not text classification, and
   it makes no DLP claim.
7. **No sensitive-access event: a route-scoped RT-08 exception.** RT-08 says
   sensitive authorized results carry a classification and produce
   `adoc.sensitive_access.v0`. This read returns a graded reference id inside
   the proposal's own bytes, to a caller who passed the predicate for it,
   without a classification and without that event. It returns no body or
   field value of the referenced object. The read stays write-free. This
   exception is scoped to this route; a read that returned the referenced
   object's content would need the classification and the event.

## Consequences

- The ADR-0069 Decision 1 waiver is closed, and the route can ship once the
  rest of E8.3 is accepted. The Decision 7 exception has no release bound.
- A reviewer whose audience is below a cited source's or anchor's class, or who
  lacks `knowledge.read` on it, cannot open the proposal, and the 404 does not
  say why. That is the fail-closed answer RT-08 asks for.
- A `knowledge.read` grant scoped by connector or source does not match the
  graded resource, which carries only the workspace, kind and object id. The
  read's target grade has the same limit.
- `contract_registry_guard::e8_3_review_contracts_are_registered_as_planned`
  pins the registry clauses, the RT-08 and §A4 pointers and this ADR;
  `scripts/roadmap-authority.py` in `agentdoc-dev/cloud` pins the
  `MILESTONES.md` §E6.1 and §E8.3 citations. Rolling the grade
  back reopens the ADR-0069 waiver and must supersede this ADR.
