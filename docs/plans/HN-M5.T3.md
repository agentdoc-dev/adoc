# HN-M5.T3: Accessibility engineering evaluation

Scope update, 2026-09-19: further accessibility qualification is no longer a
launch gate, per the user. Retain the verified checkbox fix and completed
browser evidence. Do not enable VoiceOver.


Status: planned; implementation explicitly authorized on 2026-09-18.
Requirements: HN-15.
Dependencies: HN-M4.T1; generated representative HTML.
Parent: [qualification roadmap](../roadmap/HN-LAUNCH-QUALIFICATION.md).

## Acceptance and exclusions

Automated and manual evidence for applicable WCAG 2.2 A/AA generated-HTML criteria and separate CLI usability. Record pass/fail/not-applicable/not-tested per criterion; no formal certification or blanket sample-based conformance claim.

## Ownership and existing interfaces

docs/audits/2026-09-18-accessibility.md; representative fixture; crates/adoc-core/src/infrastructure/render/html.rs and renderer tests only for reproduced issues; CLI output code only if demonstrated.
Reuse the four-crate workspace, existing strict source/graph/search contracts,
`--bin-dir` smoke interface and draft refund fixture. No new schemas or product
features solely for qualification. Evidence is scoped to the exact candidate;
source repairs require affected earlier checks to be rerun.

## Implementation order

Generate public fixture covering prose, headings, links, lists/tables/images, typed objects, quarantined markup, restricted/excluded content. Use permitted browser/native app control to inspect rendering, keyboard navigation, focus and zoom/reflow. User explicitly excluded VoiceOver on 2026-09-18; do not enable or use it. Use a pinned accessibility engine when available without adding production dependencies; reconcile findings manually. CLI: NO_COLOR/plain mode, readable errors and citations. Preserve renderer escaping/visibility invariants. For each defect, red regression, minimal semantic fix, targeted renderer/policy tests then broader gates if code changed.

## Required checks

Fixture build and source/citation correctness; automated accessibility scan; keyboard focus/link traversal; zoom/reflow/contrast; screen-reader testing excluded by user instruction; per-criterion evidence table; CLI plain/NO_COLOR; renderer/policy and full gates for Rust changes.
Record each actual command/interaction and result in the slice evidence ledger;
these are acceptance checks, not a claim that any has already executed.

## Risks, failures and rollback

Computer-use restrictions or unavailable assistive technology are concrete incomplete items, not bypassed. Screenshots and DOM scans cannot establish screen-reader behavior. Synthetic public content only. Restore only task-changed UI/config state, never relax security controls.

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
