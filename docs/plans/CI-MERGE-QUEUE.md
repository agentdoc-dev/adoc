# CI merge queue implementation plan

Status: accepted for implementation; repository settings are unchanged.
Date: 2026-09-19.
Baseline: feat/hn-launch-readiness at 34e78623; launch PR #260.

## Objective and scope

Keep contributor feedback fast while requiring native platform validation before
changes enter main. Reuse the existing build and smoke checks; do not redesign
them. A PR update runs core Linux CI. A merge-group candidate runs core CI plus
Linux x64/ARM64, Apple Silicon macOS and Windows qualification. Version tags build,
validate, attest and publish releases. Manual platform qualification remains available.

This is one infrastructure slice following the launch qualification work, not a
change to AgentDoc's product/domain contracts. It supersedes the launch plan's
PR-triggered platform qualification cadence, not its platform acceptance criteria.

Exclusions: new paid runners, new caching architecture, nightly jobs, Rust behavior
changes, additional accessibility work, changing code-review policy, and publishing
a release during validation. Do not promise reduced dollar spend: this public
repository uses free standard hosted compute; storage and paid services are separate.

## Verified current state and dependencies

- CI has unrestricted pull_request, main push and manual triggers; no merge_group.
  Format, Test, Check and Supply chain feed the single required ci check.
- Check includes the real production build, first-use smoke, receipt parity,
  no-default-feature lint and documentation build. Preserve these protections.
- release.yml owns the pre-change four-platform matrix and tag publishing. Its PR trigger
  covers release workflow, smoke script, Intel runtime script and quickstart changes.
- qualification.yml runs Windows release-profile installation and real embedding
  smoke on code-related PR changes and manual dispatch.
- Release/Windows explicitly check out PR head; queue validation must instead use
  the merge-group SHA. These are distinct evidence identities.
- Rust setup saves caches only on main. Retain this policy and the main push CI
  run; it warms existing fast-check caches without another native build matrix.
- Active ruleset 23030962 targets the default branch, requires ci and rebase-only
  PR merges, protects deletion/non-fast-forward updates and linear history. It has
  no bypass actors and no merge_queue rule. Preserve every unrelated setting.
- The launch candidate's Linux x64/ARM64, Apple Silicon macOS and Windows jobs
  passed. Intel macOS remained dominated by its pinned ONNX Runtime source build;
  the user accepted deferring that platform for launch. No earlier result is
  relabelled as evidence for this changed support matrix.
- Existing project context: docs/roadmap/ROADMAP.md, HN-LAUNCH-QUALIFICATION.md,
  docs/plans/HN-M4.T2.md and HN-M4.T3.md. Current workflows and live ruleset take
  precedence over earlier planning assumptions. Prior Mem0 retrieval failed;
  verified source and explicit session decisions supply context, not recalled claims.

## Acceptance mapping

| ID | Observable acceptance | Proof |
| --- | --- | --- |
| A1 | PR updates, including forks and stacked PRs, run core CI without native release/Windows jobs | Hosted PR run and event-contract check |
| A2 | Merge groups run core CI, packaged Linux x64/ARM64 and Apple Silicon macOS, plus Windows qualification, against github.sha | Candidate SHA logs and hosted queue checks |
| A3 | ci cannot pass a queue candidate when any mandatory job fails, is cancelled, is missing or is skipped | Gate regression cases and hosted failure rehearsal |
| A4 | Only a version-tag push can publish; manual, PR and queue events cannot | Trigger/permission inspection and hosted non-publishing run |
| A5 | Required merge queue protects main without weakening existing protections | Before/after ruleset comparison and queue rehearsal |
| A6 | Superseded PR runs cancel; separate queue groups and releases do not cancel each other | Concurrency contract check and hosted PR update |
| A7 | Manual qualification remains available; artifacts retain one-day lifetime | Manual workflow and artifact metadata |

## Implementation contract

Keep one required status name, ci. Do not require individual platform job names:
those jobs intentionally do not run on PR events. Do not put path filters on the
required CI workflow; Markdown files are test inputs in this repository.

Illustrative orchestration, using existing/new workflow filenames below:

```yaml
on:
  pull_request:
  merge_group:
    types: [checks_requested]
  # Retain existing main push and workflow_dispatch definitions.
jobs:
  native:
    if: github.event_name == 'merge_group'
    uses: ./.github/workflows/native-build.yml
  windows:
    if: github.event_name == 'merge_group'
    uses: ./.github/workflows/qualification.yml
  ci:
    needs: [fmt, test, check, deny, native, windows]
    if: ${{ !cancelled() }}
    # Existing runner and one gate step; apply the contract below.
```

The gate receives EVENT_NAME plus NEEDS through environment variables, never
interpolated shell code. Reuse its existing Python implementation, extracting it
to scripts/check-ci-gate.py only to make its decision table directly testable.
Core jobs must be success on every event. Native and Windows must be success on
merge_group; they must be skipped on other CI events. Missing entries, unknown
results and unexpected skipped mandatory jobs fail closed. A cancelled whole run
must never emit a successful gate. Keep optional FastEmbed Retrieval separate.

Extract the supported entries of the existing release matrix into native-build.yml, with
workflow_call and workflow_dispatch, contents: read, no secrets or publishing.
release.yml invokes this reusable workflow and retains its existing publish job
and explicit version-tag push condition. This avoids granting queue callers
release-write permissions. Keep checksums, extracted-package smoke, real embeddings,
architecture verification, licenses and one-day artifact retention for supported platforms.

qualification.yml gains workflow_call and retains workflow_dispatch; remove its
pull_request trigger. Both reusable workflows use github.sha for checkout and log
it. A local reusable workflow reference uses the caller revision; no main-branch
workflow reference that could test a different implementation. Keep permission
scopes read-only and checkout persist-credentials: false. No pull_request_target.

Remove release.yml's pull_request trigger; version tags and manual validation
continue to use the same native implementation. Keep existing tag-version parity,
attestation and publishing behavior. Windows remains source-install qualification,
not an invented Windows release artifact.

Concurrency: include workflow and ref in each group; cancel superseded PR runs
only. Never use one global queue/main group. Give reusable workflows distinct
prefixes if they retain their own concurrency, avoiding caller/callee group
collisions. Existing queue groups finish independently. Retain current supported
platform job timeouts initially; queue status timeout starts at 90 minutes to
cover the measured Windows/package jobs and runner waiting. A timeout remains a
failure, not a skip.

## Owned files and execution order

One builder owns these tightly coupled changes sequentially:

1. .github/workflows/native-build.yml (new): extract existing release build job.
2. .github/workflows/release.yml: call extracted build; remove PR trigger; preserve
   publishing permissions, tag guard and artifact download contract.
3. .github/workflows/qualification.yml: reusable/manual Windows qualification.
4. .github/workflows/ci.yml and scripts/check-ci-gate.py (new): merge_group trigger,
   conditional reusable jobs and event-aware required ci gate.
5. scripts/test_ci_gate.py (new): small stdlib unittest decision-table regression;
   wire it into existing Format lane alongside script checks.
6. Current installation, release, launch product/roadmap and qualification docs:
   explain contributor, queue, manual and release behavior and the Intel deferral.
   Do not rewrite historical qualification evidence as though it used this new
   pipeline. Link this plan from the roadmap.

Coordinator owns live ruleset changes and evidence. Do not split shared workflow
files among concurrent builders. Allowed scope excludes Rust crates, Cargo.lock,
model/runtime build internals, unrelated workflows and unrelated worktrees.

## Migration, live qualification and rollback

1. Refresh source HEAD and ruleset before editing. Save the complete ruleset JSON
   locally. Review the minimal intended settings diff, preserving current protections.
2. Land workflow support before requiring a queue on main. The implementation PR
   still uses the existing ci rule. Its merge is a separate publication action;
   do not infer permission to merge from this planning request.
3. Rehearse on a temporary protected validation branch with these workflows and
   an exact-branch queue ruleset. Use synthetic PRs only. Explicitly authorize
   automatic test-PR merges before enrolling them: queues merge green candidates.
   Prove one failing gate blocks and a corrected candidate passes on the actual
   merge-group SHA. Never substitute workflow_dispatch for merge_group evidence.
4. After rehearsal and workflow arrival on main, add Require merge queue to the
   existing main ruleset. Use rebase, build concurrency 1 initially, minimum and
   maximum merge count 1, only non-failing PRs, and 180-minute status timeout.
   No artificial batching delay. These are conservative starting settings; increasing
   merge limits does not itself batch CI builds. Recheck supported API/UI values
   at execution and record the actual accepted settings.
5. Verify ci remains the required check and old protections remain unchanged.
   Document that merge now means enqueue and a green queue automatically merges.
6. Remove only task-owned temporary rules/branches after preserving evidence and
   confirming their test PRs are closed. Do not disturb PR #259 or its frozen base.

If queue checks do not report, remove the newly added queue requirement first,
restoring the saved ruleset while keeping ci mandatory. Retain native workflows
for manual use. Revert workflow edits separately if necessary. Never create a
bypass actor or drop required ci to make rollout succeed.

## Required checks and reviewers

Before coding, record the baseline SHA and frozen review scope. Builder:
aw-builder / gpt-5.6-terra / medium, maximum 24 turns. Independent reviewer:
aw-refuter / gpt-5.6-sol / high, maximum 16 turns; inspect event routing, exact-SHA
checkout, skipped/failed gate behavior, least privilege, reusable artifact flow
and ruleset rollout. These roles/models are exposed in this session; recheck at
dispatch. No substitution currently planned. Coordinator owns architecture and
live evidence; a second reviewer is not necessary for this bounded infrastructure
change. Review incomplete means incomplete, not approval.

Required checks:
- python3 -B -m unittest discover -s scripts -p test_ci_gate.py
- python3 scripts/check-doc-links.py
- git diff --check
- actionlint on the affected workflows, using an available executable or pinned
  temporary installation; no new project dependency. Resolve findings, including
  reusable call permissions and expression syntax, before hosted validation.
- Gate cases: core failure/skip/missing; queue native or Windows failure, skip,
  cancellation or missing result; all-success queue; PR with both native jobs
  intentionally skipped; unexpected result fails. Test the actual gate function.
- Hosted updated PR and superseding push, manual package smoke, and the protected
  synthetic merge-queue failure/success rehearsal described above. Record event,
  SHA, job results and absence of Publish. Existing core CI supplies Rust coverage;
  do not repeat unrelated local full Rust builds for YAML-only changes.

## Risks and decisions

Native failures arrive later, at queue entry. Manual dispatch gives maintainers
an early diagnostic path for platform-sensitive PRs. Queue reordering, failures
and candidate changes can require rebuilds; do not claim one build per merged PR
or automatic compute batching. Tune concurrency only from observed queue wait
times. Cache work is a separate change if measurements justify it.

Intel macOS support is deferred for launch (see below). Live queue activation and
automatic test merges require their explicit operational authorization; this plan
alone does not supply it. Next action: implement this slice on the verified launch
baseline, then review and stage the rollout. No CI or GitHub settings changed by
writing this plan.

## Sources verified 2026-09-19

- https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/configuring-pull-request-merges/managing-a-merge-queue
- https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows
- https://docs.github.com/en/actions/how-tos/reuse-automations/reuse-workflows
- https://docs.github.com/en/billing/concepts/product-billing/github-actions
- https://github.com/agentdoc-dev/adoc/rules/23030962

## Accepted decision: defer Intel macOS and ONNX Runtime

The user accepted deferring Intel macOS for launch. A custom runner image is
excluded: GitHub custom images support Linux/Windows larger runners, not macOS,
and larger runners are billed even for public repositories.

Implementation: remove Intel macOS from the supported release and mandatory queue
matrix. Remove the Intel matrix entry and its exclusive build/packaging branches,
retire its dedicated build script, update
installation/release/support documentation, and retain historical test evidence.
This affects macOS x86_64 only, not Linux or Windows x86_64. Do not describe source
builds on Intel as tested/supported. Existing release assets remain untouched.
The queue requires three packaged platforms plus Windows qualification.

Future option if Intel support is required: build ONNX once per dependency/build
recipe revision on a standard hosted Intel macOS runner and publish a versioned
runtime archive. The archive contains the shared libraries, headers needed by the
Rust build, licenses, source SHA and build metadata. Consumers pin its exact asset
identity and SHA-256 in reviewed source, verify before extraction, and retain the
existing final-package native smoke test. Rebuild when the ONNX SHA, architecture,
SDK/deployment target or relevant compiler/build flags change. Missing or mismatched
assets fail with a clear error; do not silently rebuild for an hour in each queue
job. Only a trusted manual producer may publish runtime assets; no untrusted PR
may overwrite them. Publication and retention belong to that accepted alternative.

An Actions cache can accelerate rebuilds but is evictable and therefore cannot
promise a fast queue. A pinned runtime archive is the more predictable reuse
mechanism; it does not require a custom image, paid runner or self-hosted machine.
Do not implement this future option in the launch slice. Restoring Intel support
requires a separately accepted plan and fresh platform evidence.

Additional sources verified 2026-09-19:
- https://docs.github.com/en/actions/how-tos/manage-runners/larger-runners/use-custom-images
- https://docs.github.com/en/actions/concepts/runners/larger-runners
