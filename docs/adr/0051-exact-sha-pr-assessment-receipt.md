# ADR-0051: Exact-SHA PR Assessment Receipt

- Status: Accepted
- Date: 2026-07-22
- Roadmap: V9.2.2

## Context

ADR-0050 made `adoc.change_assessment.v0` the deterministic local fact
boundary. A GitHub run still needs to bind that artifact to the pull request,
toolchain, Action policy, final conclusion, and retained evidence without
putting GitHub concerns into `adoc-core` or reconstructing coverage in shell.

The Action already has proposal and delivery behavior which predates the V9.3
canonical patch contract. A V9.2 receipt must describe that state honestly
without assigning a future canonical patch-set digest to legacy drafts.

## Decision

### Ownership and artifacts

AgentDoc continues to own `adoc.change_assessment.v0`. The Action invokes it
once with the exact pull-request base and head commit SHAs and one UTC
evaluation date captured during preflight. It validates that the envelope
echoes those revisions and the unique merge base before rendering or gating.

The Action owns the experimental `adoc.pr_assessment_receipt.v0` schema. It
writes two adjacent retained files:

- `assessment-<invocation-id>.json`, containing the exact validated CLI bytes;
- `receipt-<invocation-id>.json`, referencing the assessment by SHA-256.

The composite Action exposes both paths and digests. Retention is caller-owned
through a separately pinned `actions/upload-artifact` step with `if: always()`.
The Action never uploads its whole run directory and contains no artifact
client.

### Invocation identity and limits

An invocation ID contains the GitHub run ID, attempt, sanitized job ID, and
128 random bits. Private and retained directories both use that ID, so two
Action invocations in one job cannot alias.

V9 fixes these Action limits:

- at most 5,000 deterministic changed paths; larger assessments are retained
  but conclude non-green and are never truncated-complete;
- at most 60,000 characters in the final rendered report, with identity,
  outcome, remediation, revisions, receipt digest, and run link preserved.

Changing either value requires a reviewed contract revision.

### Receipt variants

The receipt schema is a closed Draft 2020-12 schema using `oneOf` on
`run_status`.

`completed` means a valid assessment was established and receipt finalization
succeeded. It does not mean the job is green or the assessment is complete.
Schema-valid nonzero `partial/not_evaluated`, `error/invalid`, and
`error/not_evaluated` assessments therefore receive completed receipts.
A complete assessment requires an available knowledge snapshot; an invalid
assessment may record it as unavailable.

`failed` means no valid assessment envelope was established. It carries a
bounded Action diagnostic and never fabricates an assessment or snapshot
digest. If receipt finalization itself fails, receipt outputs remain empty and
the job concludes non-green with `action.receipt_failed`.

The final receipt records exact GitHub revisions, evaluation date, CI identity,
Action and AgentDoc provenance, normalized inputs, assessment and knowledge
digests, final conclusion, and status-bearing knowledge-gate, semantic-review,
proposal, and delivery sections. It does not embed raw diffs, Knowledge Object
bodies, prompts, provider output, credentials, or proposal bodies.

Before V9.3, semantic review is `disabled`. Existing legacy proposal output is
`partial` with an exact count, no digest, and
`legacy_proposal_not_canonical`; disabled, skipped, and error states remain
explicit. Delivery records its actual mode and result. The knowledge gate is
`not_applicable` until V9.4.4.

### Provenance

The Action records its requested ref. A full 40-hex ref is `full_sha` and may
be recorded as the resolved commit. Tags and branches are `mutable_ref` with a
null resolved commit; local `uses: ./` execution is `local`.

AgentDoc provenance records the requested release identifier, the version
reported by the verified binary, and the binary SHA-256. The release workflow
requires the immutable Git tag to equal that binary version. V9.2.2 requires
AgentDoc `v0.3.0`; older binaries and unknown future assessment schemas fail
honestly.

### Gate semantics

Infrastructure, input, event, install, ref, path-set, version, contract, and
internal failures are non-green in every mode. `partial/not_evaluated` and
`error/not_evaluated` are also always non-green.

`error/invalid` follows existing structural policy: `advisory` remains green;
`strict/full` gates on `errors_full`; and `strict/diff` gates on
`errors_changed + errors_unattributed`. Coverage, impact, lifecycle,
contradiction, and knowledge-change findings remain advisory. Proposal and
delivery state never changes deterministic assessment bytes.

Only the final enforcement step exits nonzero. Earlier expected failures are
recorded so a safe report, outputs, and receipt can still be finalized.

## Consequences

The Action becomes a delivery adapter over one AgentDoc assessment instead of
a second knowledge-policy implementation. GitHub artifacts provide bounded
free/local retention, but they are policy-bound and deletable rather than an
organization-wide audit store. Receipt signing, central retention, semantic
classification, and canonical proposal digests remain in their named later
slices.
