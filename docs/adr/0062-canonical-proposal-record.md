# ADR-0062: Canonical Proposal Record

- Status: Accepted
- Date: 2026-09-02
- Slice: E5.1
- Depends on: ADR-0053, ADR-0054, ADR-0058, ADR-0059

## Context

ADR-0053 fixed how the Action serializes and digests a model proposal set,
but the set itself lived only in Action-private files and the receipt's
`proposals.sha256`. Cloud governance (E5.2 approval, E5.3 gate) needs one
record that binds those exact patch bytes to the exact revisions, deterministic
assessment, semantic context, semantic assessment, and Knowledge Object
content they were produced against — and that record must be identical
whether the Action delivers it through Git or an agent submits it through the
API.

## Decision

1. `adoc.proposal.v0` is an `adoc`-owned domain contract in `adoc-core`
   (`ProposalRecord`). The E0.3 registry reservation named `cloud` as owner;
   the owner moves to `adoc` at slice start because the Action and the CLI
   produce the record and Cloud consumes it as the
   `agentdoc.cloud.proposal_command.v0` payload.
2. Identity is the proposal-set digest of ADR-0053 §8: each patch is hashed
   over its exact bytes (sorted compact JSON plus one newline) and the set
   digest hashes the compact JSON array of the ordered patch digests plus one
   newline. Patches are ordered by patch digest alone — not by the
   placement-first key ADR-0053 §8 used for Action-private files — because
   identity must be placement-blind (E1.1; MILESTONES §E5.1 acceptance): a
   source-placement move that reorders a placement-sorted multi-patch set
   must not mint a version. The digest binds the patch set, not the finding
   correlation or placement metadata; two records with one digest and
   different bytes are a delivery conflict Cloud rejects
   (`governance.proposal_conflict`), never a merge. Cloud's
   `private.proposal_set_digest` already computes this value over the ordered
   bytes; the Action reads `proposals.sha256` from the record once it
   produces one.
3. Bindings are mandatory and closed: exact base and head revisions,
   change-request system and identifier, deterministic assessment digest,
   semantic-context digest, semantic-assessment digest, and the exact
   `content_hash` of every existing object an update or body replacement
   targets. A record with any binding missing is unconstructible; bytes become
   a record only through a validator that re-derives every digest and the
   ordering.
4. Cross-links are identifiers plus digests, never branch names or titles.
   The record has no wall-clock field. Any producer therefore yields
   byte-identical records for the same inputs (`adoc proposal-record` is the
   shared adapter).
5. Editing is superseding: any byte change to any patch mints a new record
   whose `supersedes` names the prior digest, so the invalidation consequence
   is visible before submission. A byte-identical revision is not a version.
6. The create-only floors remain: operations are closed to `create_object`,
   `update_fields`, and `replace_body`; creates use the ADR-0053 §2 kind/status
   pairs; updates leave the object at a reviewable status (ADR-0054 §3); the
   ADR-0053 §3 authority fields are never proposable. Violations fail with
   `proposal_record.authority_rejected`, so a model-originated submission can
   only create a proposal record and never touches active state.

## Refuted alternatives

- Embedding patches as base64 bytes: exact but opaque; the sorted compact
  serialization is already the exact byte contract, so the record embeds the
  patch object and re-derives the bytes.
- A Cloud-private proposal shape: would force the Action and any API client
  to agree with Cloud out of band; the shared domain contract keeps one
  digest implementation authoritative.

## Consequences

- Registry: `adoc.proposal.v0` moves to the shipped `adoc` table; diagnostic
  codes `proposal_record.invalid_document`, `proposal_record.binding_invalid`,
  `proposal_record.patch_invalid`, `proposal_record.authority_rejected` ship.
- Cloud stores the record bytes verbatim next to `proposal_records` and
  recomputes the set digest from the embedded patches; approval (E5.2) binds
  to `proposal_set_digest` and `supersedes` drives invalidation.
- The E5.3.T3 typed per-finding no-change disposition record joins this
  envelope as an additive field.
