# ADR-0047: Composite Action Packaging

**Status:** Accepted
**Date:** 2026-07-17
**Slice:** V8.3.3

## Context

V8.3.2 fixed the CI packaging decision as "a documented workflow snippet plus
a live job in this repo — not a composite GitHub Action", with the un-gating
threshold reserved for ADR-0044. ADR-0044 was never recorded as a file; the
threshold existed only as a roadmap reservation ("partners measurably fail to
adopt the snippet as-is").

The MVP scope has since changed the calculus: a Marketplace-listed action is
part of the product surface itself, not post-pilot packaging polish.
Advisory-first adoption — the §24.3 rollout the roadmap already mandates —
needs a maintained artifact users pin (`agentdoc-dev/action@v1`), not a
copy-paste snippet that drifts in every consumer repo. This ADR supersedes
the V8.3.2 packaging paragraph and deliberately jumps the reserved ADR-0044
gate, recording the decision instead of waiting for adoption-failure
evidence.

## Decision

1. **A composite GitHub Action lives in its own public repository,
   `agentdoc-dev/action`.** It is a thin wrapper — bash steps, a problem-matcher
   JSON, no Node, no Docker. All behavior stays in the adoc CLI presenters
   (`check --format markdown`, `impacted-by --format markdown|json`); the
   action is glue. New capability lands in the CLI first, never in the
   action.
2. **The action installs prebuilt binaries from this repository's GitHub
   Releases**, sha256-verified. A tag-triggered `release.yml` (Linux x86_64 +
   arm64) is therefore load-bearing product infrastructure.
3. **Version coupling:** each action release pins a tested adoc tag as the
   `adoc-version` input default. `latest` is accepted but documented as
   unsupported for pinning.
4. **Enforcement is a consumer decision, defaulting to advisory** — the
   `enforcement: advisory | strict` input carries §24.3's advisory-first
   rollout; `scope: full | diff` bounds the strict gate to PR-changed files
   when asked.
5. **Proposed Knowledge Objects are deterministic:** changed paths minus the
   paths any Impacted Query hit claims (`adoc.impacted.v0`). The comment's
   "Proposed Knowledge Objects" section is the reserved seam for a later
   Agent-Patch drafting tier (`create_object` ops); that tier arrives behind
   a then-new input, so no dead inputs exist today.

## Consequences

- `.github/workflows/adoc-pr.yml` dogfoods the released surface (released
  action + released binary), exactly what a partner gets; HEAD coverage
  stays in `ci.yml`.
- `docs/guides/ci-integration.md` leads with the action; the raw workflow
  snippet survives as an appendix for non-GitHub CI. (V8.3.2 named this file
  `docs/ci-integration.md`, but a docs-root `.md` derives a one-segment page
  ID the Object ID grammar rejects; `guides/` supplies the second segment.)
- The V8.3.2 snippet-is-the-product stance is superseded; its internals live
  on verbatim as the action's steps (marker comment, advisory-first, one
  in-place-updated comment).
- Marketplace listing requires the action repo to hold its own releases and
  a floating `v1` major tag; that maintenance cost is accepted.
