# HN-M4.T4: Pristine OS installation

Status: planned; implementation explicitly authorized on 2026-09-18.
Requirements: HN-12; HN-2/4.
Dependencies: HN-M4.T1; disposable clean OS VM.
Parent: [qualification roadmap](../roadmap/HN-LAUNCH-QUALIFICATION.md).

## Acceptance and exclusions

Boot a fresh official Linux OS VM with its own kernel and empty user caches. Record image digest/provenance, initial inventory, exact prerequisite/Rust install and candidate source digest. Both source/Cargo installation and real installed-binary journeys must run without host registry/build/model caches.

## Ownership and existing interfaces

scripts/qualification/pristine-linux.sh or equivalent retained recipe; docs/guides/launch-qualification.md; installation prerequisites only when evidence shows omissions.
Reuse the four-crate workspace, existing strict source/graph/search contracts,
`--bin-dir` smoke interface and draft refund fixture. No new schemas or product
features solely for qualification. Evidence is scoped to the exact candidate;
source repairs require affected earlier checks to be rerun.

## Implementation order

Prefer a local ARM64 QEMU/HVF VM if available on this macOS host; inspect availability before installing tooling. Verify official image checksums/provenance, create task-owned disk overlay and local-only SSH forwarding with an ephemeral key. No host home/credential/cache mounts. Install only declared prerequisites. Transfer the public candidate snapshot, build/install both binaries, test lexical and real embeddings, then disable VM network and rerun the installed offline journey. Retain logs and remove owned credentials/VM resources after evidence. Containers may supplement but cannot replace the VM check.

## Required checks

Image checksum and provenance; guest OS/kernel/architecture and initial package inventory; exact prerequisite install; cargo installs/source build at recorded SHA; both smoke modes; disconnected-network offline smoke; source and binary digests; no shared caches/mounts.
Record each actual command/interaction and result in the slice evidence ledger;
these are acceptance checks, not a claim that any has already executed.

## Risks, failures and rollback

Clean Linux VM evidence covers that image only, not pristine macOS/Windows. If virtualization/verified image/network unavailable, preserve concrete blocker and continue other slices. Never change the users OS, firewall or global credentials. Rollback destroys only task-owned VM after evidence retention.

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
