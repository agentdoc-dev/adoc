# HN-M5.T2: Performance engineering baseline

Status: planned; implementation explicitly authorized on 2026-09-18.
Requirements: HN-14.
Dependencies: HN-M4.T1; fixed candidate after applicable security repairs.
Parent: [qualification roadmap](../roadmap/HN-LAUNCH-QUALIFICATION.md).

## Acceptance and exclusions

Reproducible actual-release-binary timings and memory observations for fixed small/medium/large corpora, while validating result IDs/citations. Report samples and limits rather than a fabricated SLA or certification.

## Ownership and existing interfaces

scripts/qualification/benchmark.py, one small driver regression/self-check, docs/audits/2026-09-18-performance.md.
Reuse the four-crate workspace, existing strict source/graph/search contracts,
`--bin-dir` smoke interface and draft refund fixture. No new schemas or product
features solely for qualification. Evidence is scoped to the exact candidate;
source repairs require affected earlier checks to be rerun.

## Implementation order

Use Python stdlib and existing subprocess CLI/MCP contracts. Generate deterministic corpora at 100/1000/10000 objects with bounded total bytes; preserve known graph IDs and query hits. Measure check, no-embedding build, first/cached embedding build, lexical/semantic/hybrid query, MCP startup and repeated retrieval. Separate network/model fetch from inference and process startup from steady requests. Record seed, content digests, CPU/OS/toolchain, model, warmup/repetitions and timeouts. Use native /usr/bin/time memory observations on macOS with units recorded; mark unmeasured dimensions. At least five warm samples; cold repeats disclosed and bounded. Assert result correctness every sample. Store raw JSON.

## Required checks

Small driver regression for failed command/invalid result; deterministic corpus digest; at least five warm samples per scenario; correctness assertions; raw timing/memory data and robust summary; inspect largest corpus for OOM/timeout; independent evidence review.
Record each actual command/interaction and result in the slice evidence ledger;
these are acceptance checks, not a claim that any has already executed.

## Risks, failures and rollback

No percentile guarantees from tiny samples, cross-host ranking or hard performance budget without evidence. Cap each sample and dataset to protect host. Profile only measured bottlenecks; do not optimize speculatively. Revert harness/docs or scoped measured repair.

## Review and delivery

User explicitly authorized implementation of all follow-up tasks after planning.
Stay on `feat/hn-launch-readiness`, one coherent commit per validated slice.
Preserve unrelated work. Coordinator owns scope, evidence, Git and remote actions.
At most two isolated workers; aw-builder gpt-5.6-terra/medium (24 turns) for an
owned bounded implementation, aw-refuter gpt-5.6-sol/high and real Claude
Opus/high (16 turns each) independently review frozen change/evidence. These
models were used successfully in this session; recheck availability at dispatch.
Two repair rounds then diagnose; incomplete review is not approval. Security
review partitions have explicit file manifests and keep manual vs automated
coverage separate. Workers do not expand scope or dispatch children.

Behavior changes require a failing regression first and affected Rust tests,
Clippy and other applicable repository gates. Evidence-only changes use link/diff
checks and reviewed exact-SHA logs; no redundant full rebuild without cause.
All evidence includes source/binary identity, environment, command/interaction,
exit/result and limitations. Retain sensitive raw data locally; commit sanitized
reports. No blanket claims from partial platform/UI/security coverage.
