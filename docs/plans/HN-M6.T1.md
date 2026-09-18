# HN-M6.T1: Claude Desktop MCP setup through UI

Status: planned; implementation explicitly authorized on 2026-09-18.
Requirements: HN-16; HN-3.
Dependencies: HN-M4.T1 installed binary; actual installed Claude Desktop.
Parent: [qualification roadmap](../roadmap/HN-LAUNCH-QUALIFICATION.md).

## Acceptance and exclusions

Configure the installed gateway through the named desktop clients real supported UI/configuration flow; visibly confirm connection, tool status and a cited answer from a disposable public-only project. Existing headless Claude Code evidence is not reused as UI proof.

## Ownership and existing interfaces

docs/guides/mcp-agent-gateway.md, docs/guides/launch-qualification.md; minimal reversible per-client server registration.
Reuse the four-crate workspace, existing strict source/graph/search contracts,
`--bin-dir` smoke interface and draft refund fixture. No new schemas or product
features solely for qualification. Evidence is scoped to the exact candidate;
source repairs require affected earlier checks to be rerun.

## Implementation order

Inspect installed app/version and current official client setup docs. Preserve all existing MCP registrations; add only AgentDoc qualification with absolute installed path. Use a persistent task-owned install root until testing/cleanup, not a removed temp binary. Through UI ask status then billing.refund-window with explicit project_root. Observe actual tool calls/results, source docs/index.adoc:3:1 and patch apply disabled. Use only synthetic fixture text. Record client version/config redacted/interaction evidence, then retain or remove only own registration according to tested setup intent.

## Required checks

Client UI connection/tools status; actual project_status and why tool calls; exact cited answer; default write-disabled observation; clean restart/reconnect if supported; setup guide command/path validation.
Record each actual command/interaction and result in the slice evidence ledger;
these are acceptance checks, not a claim that any has already executed.

## Risks, failures and rollback

Unexpected auth/account or security-sensitive consent is a user handoff when policy requires; do not bypass it. If UI/client capability is unavailable, record the exact blocked step and continue other tasks. No deletion/overwrite of existing client configuration; no real user documents sent to provider.

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
