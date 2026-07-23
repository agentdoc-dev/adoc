# ADR-0052: Action-Owned Cited Semantic Review

- Status: Accepted
- Date: 2026-07-23
- Roadmap: V9.3.1

## Context

ADR-0047 requires new product capability to land in the AgentDoc CLI before
the composite GitHub Action. ADR-0050 and ADR-0051 subsequently established a
stronger boundary: `adoc.change_assessment.v0` owns deterministic local facts,
while `adoc.pr_assessment_receipt.v0` binds those facts to an exact GitHub run.

V9.3.1 adds an optional model-assisted classification of bounded pull-request
diff hunks against selected Knowledge Objects. A model cannot make a
deterministic compiler guarantee, and putting provider, prompt, or model code
in `adoc-core` would weaken rather than extend the Change Assessment contract.
The capability is specific to the GitHub review workflow and must remain
visibly separate from the compiler result.

## Decision

V9.3.1 is a deliberate exception to ADR-0047 Decision 1:

1. The Action owns the experimental `adoc.semantic_review.v0` artifact,
   provider invocation, prompt, validation, rendering, and retention boundary.
2. AgentDoc continues to own all deterministic semantics. The Action may use
   an exact-head Graph Artifact and lexical `adoc search` results as bounded
   model context, but it may not add semantic findings to
   `adoc.change_assessment.v0`.
3. The only provider is the existing pinned Claude Code integration. V9.3.1
   adds no provider abstraction.
4. Findings are advisory and use exactly four classifications:
   `consistent`, `extends_existing_knowledge`,
   `contradicts_existing_knowledge`, and `insufficient_evidence`. They carry
   exact diff-hunk citations and zero or more exact Knowledge Object
   ID/`content_hash` citations. Numeric confidence is forbidden.
5. Semantic review is an explicit opt-in, disabled by default. It never runs
   for forks, Dependabot, unsupported events, missing credentials, or an
   invalid deterministic assessment.
6. The provider receives only the bounded exact-revision diff and selected
   exact-head Knowledge Object bodies. Repository content is untrusted data.
   Raw prompts, diffs, responses, stderr, credentials, and temporary context
   are private runner state and are deleted on every exit path.
7. The V9 resource ceilings in `ROADMAP-V9.md` are contract limits. Changing a
   ceiling requires a reviewed contract revision.
8. Semantic failure never changes deterministic assessment bytes or meaning.
   A repository may use the existing `propose-on-error: fail` policy to make
   failure of the optional operation non-green, but that is an Action policy
   result rather than a compiler or compliance verdict.

## Consequences

- `adoc-core`, AgentDoc schemas, and the Local CLI gain no model concepts.
- The Action publishes and validates `adoc.semantic_review.v0` independently
  of the Change Assessment.
- Existing proposal behavior remains unchanged until V9.3.2 replaces it with
  canonical `adoc.patch.v0` proposals.
- Existing stable `v1` Action behavior is not moved to the V9.3 implementation.
  V9.3 is dogfooded through a `v2` prerelease until governed proposals and
  delivery are complete.
