# ADR-0053: Canonical Create-Only Model Proposals

- Status: Accepted
- Date: 2026-07-23
- Roadmap: V9.3.2

## Context

The Action's legacy proposal path accepts provider-authored `.adoc` blocks and
edits source with shell text processing. AgentDoc already owns the canonical
single-operation `adoc.patch.v0` contract and the `patch --check` /
`patch --apply` validation loop. A second proposal format or editor would
duplicate that authority and make model output harder to audit.

V9.3.2 must also prevent model-generated knowledge from acquiring authority,
correlate proposals to validated cited findings, and retain the exact-head
evidence boundary established by ADR-0052.

## Decision

1. Model-generated executable proposals use the existing single-operation
   `adoc.patch.v0` contract. No patch bundle or second source editor is added.
2. The Action constructs only `create_object` patches with these exact
   kind/status pairs: `claim/draft`, `decision/proposed`, `api/draft`, and
   `task/open`.
3. Generated fields may not include `verified_at`, `reviewed_by`,
   `approved_by`, `decided_by`, or `resolved_by`, nor duplicate structural
   members such as `id`, `kind`, `status`, `body`, or `placement`. AgentDoc
   remains authoritative for all other kind-specific metadata validation.
4. The provider returns one private response containing cited findings and
   patch candidates. Each finding carries one unique opaque `provider_ref`;
   candidates name it through `finding_ref`. The Action validates findings,
   assigns stable `finding_id` values, and then correlates candidates. Missing,
   duplicate, ambiguous, or rejected correlations are never repaired.
5. Executable candidates are accepted only for a validated
   `extends_existing_knowledge` finding with `proposal_expected: true`.
   Multiple creates may reference one eligible finding. Other findings and
   eligible findings without a candidate remain human-readable suggestions.
6. The provider selects placement only from an Action-built allowlist of
   existing exact-head `.adoc` pages and optional existing anchors on the same
   page. It may not invent a source path or anchor another proposed object.
7. The Action, not the provider, constructs `reason` from the deterministic
   assessment digest and final `finding_id`, and constructs `proposer` from
   the Action-owned Claude Code provider/model identity. The receipt binds the
   exact Action revision.
8. Each final patch is serialized as sorted compact JSON plus one trailing
   newline and hashed over those exact bytes. Patches are sorted by logical
   placement path, page ID, target, and patch digest. The proposal-set digest
   hashes a sorted compact JSON array of those ordered digests plus one
   trailing newline. This private array is not a new public bundle schema.
9. Before any patch is shown as executable, one disposable exact-head sandbox
   must pass initial graph/object-set digest parity and the same-date
   `check`/`build --no-embeddings` gate. Every patch then passes
   `patch --check`, `patch --apply`, `check`, and a fresh build before the next
   patch. All final targets must exist at the required non-authoritative
   status.
10. JSON Schema parity covers the parser's structural wire contract:
    operation members, required and forbidden `base_hash`, changes,
    placement, proposer, unknown members, and JSON types. Rust patch
    validation remains authoritative for Object IDs, lifecycle rules, graph
    references, placement existence, and post-apply source validity.
11. Provider/candidate rejection is advisory and may produce a partial
    proposal result. Provider-contract, exact-head, or sandbox infrastructure
    failure is an error governed by `propose-on-error`.

## Consequences

- `adoc.patch.v0`, `adoc.patch.check.v0`, and `adoc.patch.apply.v0` remain
  unchanged and single-operation.
- The published patch-input schema is tightened to the shipped parser and
  tested against the same structural accept/reject corpus.
- The Action removes legacy block extraction, direct `awk` mutation, and
  model-generated updates or expiry refreshes.
- Stable Action `v1` retains the legacy behavior. Canonical proposals ship on
  the immutable `v2` prerelease train; no floating `v2` tag moves before
  V9.3.3 completes its governed delivery gates.
