# ADR-0064: Repository Baseline Contract True-Up

- Status: Accepted
- Date: 2026-09-05
- Obligation: O-01
- Historical implementation: PR #140

## Context

PR #140 shipped the `adoc baseline` command and its
`adoc.repository_baseline.v0` envelope before Product V1 required every wire
contract to have an ADR, a registry entry, a published schema, and executable
producer/schema parity evidence. The command was subsequently registered, but
O-01 correctly kept the missing decision and schema evidence visible before
Cloud ingestion could consume the envelope.

The shipped contract already has a production consumer. Action v2 runs
`adoc baseline` against the exact assessed head during repository bootstrap,
validates the envelope, retains its exact bytes and SHA-256 digest, and uses
its path/object inventory to drive bounded bootstrap proposals. This true-up
records that existing boundary; it does not redesign it.

## Decision

1. `adoc.repository_baseline.v0` is unchanged. The authoritative producer
   remains the `adoc baseline --ref <exact-head> --as-of <date> --format json`
   path introduced by PR #140. This ADR and the published schema describe the
   shipped wire; they add no field, status, fallback, or new version.
2. The envelope is the repository-wide, head-side projection of the validated
   change-assessment machinery. It carries `evaluation_date`, the immutable
   head `snapshot`, `knowledge_snapshot`, `assessment_config`, `summary`,
   `validation`, `paths`, `objects`, and `diagnostics`. Pull-request comparison
   and review-only projections are not baseline fields.
3. Readiness is deterministic and uses this precedence:
   `invalid_source` when the assessment is incomplete or full validation has
   errors; otherwise `provisional_paths` when any path is provisional;
   otherwise `uncovered_paths` when any path is uncovered; otherwise `ready`.
   `readiness.ready` is true only for the final case. In particular,
   provisional coverage takes precedence over uncovered coverage when both
   counts are non-zero.
4. Replay identity comes from explicit deterministic inputs and projections:
   the evaluation date; the requested and resolved immutable head revision;
   graph and Object-set SHA-256 digests in `knowledge_snapshot`; configuration
   and effective/proposed policy SHA-256 digests in `assessment_config`; and
   each projected Object's content hash. The envelope has no self-asserted
   digest field. Consumers such as Action hash the exact serialized bytes they
   retain.
5. Action bootstrap remains the first consuming workflow. Its validation and
   exact-byte retention are consumer behavior, not additional baseline
   authority. Cloud may consume the envelope only through a version-matched
   validated boundary; it must not reinterpret readiness or replace the
   producer's digest-bearing facts.
6. The published
   [`adoc.repository_baseline.v0.schema.json`](../agent/v0/schema/adoc.repository_baseline.v0.schema.json)
   is transport/schema evidence for this exact version. Producer parity is
   pinned by `crates/adoc-mcp/tests/contract_schemas.rs`, including all four
   readiness reasons and rejection of unknown versions/reasons. The real CLI
   surface remains covered by `crates/adoc-cli/tests/assess_changes_cli.rs`;
   Action's consumer validation and exact-byte digest retention remain covered
   by `test/baseline.sh` in `agentdoc-dev/action`.

## Consequences

- O-01 is satisfied by this ADR, the published schema, and the executable
  parity evidence. Its permanent decision-register row remains as provenance
  for the PR #140 registration gap.
- Existing `adoc.repository_baseline.v0` producers and Action v2 consumers do
  not migrate. Additive changes remain subject to the registered v0-additive
  posture; a breaking semantic or structural change requires a new registered
  version and decision.
- This true-up does not authorize Cloud ingestion by itself. Cloud still owns
  its transport, authorization, persistence, and version-matching checks.
