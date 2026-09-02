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
   must not mint a version. The digest binds the patch set, not the
   bindings, finding correlation, or placement metadata, so a same-digest
   re-delivery that differs only there (a placement move, a new head on the
   same change request) is a duplicate Cloud acknowledges
   (`ingest.duplicate_delivery`) while the first-stored record stays
   authoritative — never a merge; a same-digest delivery whose patch set
   differs is a conflict Cloud rejects (`governance.proposal_conflict`).
   Cloud recomputes this
   value from the embedded patches in this digest order (an obligation on
   the E5.1 consumer, checked by its ingestion tests against `adoc`
   output); the Action reads `proposals.sha256` from the record once it
   produces one. ADR-0053 §8 carries a superseded-in-part note pointing
   here.
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
   is visible before submission. A byte-identical revision is not a version
   and fails with `proposal_record.revision_unchanged`.
6. The create-only floors remain: every patch `target` and entry `page_id` is
   an Object ID, and every `placement_path` follows the scanner-strict logical
   page-path grammar (project-relative, slash-normalized, no empty or dot
   components; `proposal_record.patch_invalid` otherwise); operations
   are closed to `create_object`, `update_fields`, and `replace_body`; creates use the
   ADR-0053 §2 kind/status pairs and their `fields` never duplicate a
   structural member (ADR-0053 §3 — a nested `status` would bypass the
   pair); updates leave the object at a reviewable status (ADR-0054 §3) and
   must say so — the record cannot see the object's current lifecycle, so
   every existing-object edit carries an `update_fields` that sets `status`
   to `draft`, `proposed`, or `open` (ADR-0054 §3's explicit
   status-preservation stays an Action delivery option that never forms a
   proposal record); the ADR-0053 §3 authority fields are never proposable.
   Every embedded patch reason is semantic text: non-empty, without surrounding
   whitespace or control characters (`proposal_record.patch_invalid`
   otherwise). This proposal-record floor is stricter than generic patch
   validation, which rejects only blank reasons.
   An embedded patch contains no explicit JSON `null`: the patch reader treats
   a null optional member as absent, while canonical bytes and identity do not
   (`proposal_record.patch_invalid` otherwise).
   Every embedded patch declares an `agent` proposer with a non-empty
   identifier constructed by the trusted Action rather than the provider
   (ADR-0053 §7).
   Violations fail with `proposal_record.authority_rejected`, so a
   model-originated submission can only create a proposal record and never
   touches active state. Every create carries placement whose embedded
   `page_id` equals the patch entry's `page_id`; an object created by the same
   proposal set cannot be its `after` anchor.
7. A target is created at most once and is never both created and edited in
   one proposal set. An existing object is edited by at most one
   `update_fields` followed by at most one `replace_body` (ADR-0054 §5). Each
   patch carries the object's `base_hash` at its point in that sequence, so
   the body patch binds the
   hash re-derived after the field patch (PRD §51.5). When the field patch
   is only the §6 status write on an object that is already reviewable it
   changes nothing, the object is not re-hashed, and the body patch
   legitimately binds the same `base_hash`; the record cannot see the
   object, so it never judges the second hash — that is the apply-time
   check in the Action sandbox and Cloud's exact-head preflight.
   `content_bindings` records the exact-head hash the first patch carries.
   Application order is fixed by the operations, not by the digest-ordered
   `patches` array. A second patch of one operation for one target fails
   with `proposal_record.patch_invalid`.
8. Every embedded patch passes the graph-independent half of `adoc patch
   --check` before the record exists: draft requirements, field key and shared
   value rules, source-splice-safe body and field text, create-object
   evidence-reference list syntax, impacts list syntax and source-parser
   repository-relative path validity (distinct from §6's scanner-strict page
   paths), placement ID syntax, and required create placement. Graph
   existence, target-kind field and value compatibility, evidence-reference
   syntax and resolution for updates, placement resolution, and post-apply
   source validity remain exact-head
   preflight concerns. Because `update_fields` only inserts or replaces
   fields, exact-head preflight also refuses an edit whose reviewable
   prospective state would require removing an existing field. This includes
   an answered question carrying authority-owned `resolved_by` and an API
   representation switch between mutually exclusive fields; a trusted source
   edit must first establish a valid reviewable shape. The record layer cannot
   see this head state and may accept the patch; exact-head refusal uses
   `patch.validation_failed`. Existing `warning`, `constraint`,
   `agent_instruction`, and `source` objects are likewise outside the proposal
   surface because their closed schemas intentionally have no lifecycle
   `status`; their mandatory status write is refused at exact head with
   `schema.unknown_field`. A glossary likewise has no lifecycle: its legacy
   metadata field named `status` cannot satisfy an agent proposal's mandatory
   write, and exact-head preflight refuses that agent-proposed write with
   `patch.validation_failed`; trusted non-agent patches may still maintain the
   metadata. Intrinsic failures use `proposal_record.patch_invalid`.

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
  `proposal_record.patch_invalid`, `proposal_record.authority_rejected`, and
  `proposal_record.revision_unchanged` ship.
- The record travels as a JSON value inside
  `agentdoc.cloud.proposal_command.v0`, so transport whitespace and key
  order carry no meaning: the validator re-derives every digest and the
  ordering from the parsed value, and Cloud stores the canonical
  re-serialization (declared field order, pretty, one trailing newline —
  the bytes `adoc proposal-record` emits) next to `proposal_records` and
  recomputes the set digest from the embedded patches; approval (E5.2)
  binds to `proposal_set_digest` and `supersedes` drives invalidation.
- The E5.3.T3 typed per-finding no-change disposition record joins this
  envelope as an additive field.
