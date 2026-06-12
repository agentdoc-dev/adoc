# ADR-0037: MCP `adoc_patch_apply` Opt-In via Project Config

**Status:** Accepted
**Date:** 2026-06-12
**Slice:** V6.4 (TB4; recorded at slice start per the [ROADMAP-V6.md](../ROADMAP-V6.md) ADR inventory)

## Context

ADR-0013 and ADR-0014 promised that the MCP gateway "does not apply patches"
and ADR-0012 framed the Agent Patch as never rewriting AgentDoc Source. V6.4
ships patch application (ADR-0036), and the cycle rule is explicit:
agent-initiated writes are opt-in; the CLI apply path is human-initiated and
ungated. The question is how an MCP write surface coexists with those promises
without either silently breaking them or hiding the capability from agents.

## Decision

1. **Config gate.** A new optional block in `agentdoc.config.yaml`:

   ```yaml
   mcp:
     patch_apply: enabled   # or: disabled
   ```

   Absent block ⇒ disabled. Any value other than `enabled`/`disabled` is a
   loud `ConfigInvalid` error. `adoc init` does not write the key — opting in
   is always a deliberate human edit.
2. **Registered-but-refusing posture.** `adoc_patch_apply` is registered
   always, so the tool list is stable and agents can learn the gate exists.
   When disabled it returns a normal `adoc.patch.apply.v0` envelope with
   `applied: false` and exactly one fix-oriented diagnostic
   (`mcp.patch_apply_disabled`) naming the config key and noting that
   `adoc_patch_check` remains available. Never a protocol error.
3. **Identical preconditions over MCP.** The MCP path builds the same
   project-root-sandboxed context as every other tool; writes route through
   the same `WorkspaceWriter` containment, and `base_hash` plus the
   `patch.source_drift` gate apply unchanged. The envelope is the same as the
   CLI's modulo `trace.interface: "mcp"`.
4. **Readiness surface.** `adoc.project.status.v0` gains an additive
   `readiness.patch_apply_enabled` boolean so agents check the gate before
   constructing a patch for apply.
5. **Guidance.** A new resource `adoc://agent/v0/patch-apply-guide` records
   the loop: propose → check → apply → re-check → cite the post-check.
6. **Prompt policy.** The pinned `adoc_propose_patch_v0` prompt and its
   unversioned alias stay byte-stable per ADR-0014; an apply-aware
   `adoc_propose_patch_v1` is added alongside. The unversioned alias keeps
   pointing at v0 — v1 is addressable only by its versioned name.
7. **Supersedes in part.** This ADR supersedes the "MCP does not apply
   patches" promises of ADR-0013/ADR-0014 and the ADR-0012 "does not rewrite
   AgentDoc Source" framing, scoped: validation and review remain read-only;
   application exists only behind this gate. The migration checklist is the
   V6.4 TB4 doc inventory: `docs/agent/v0/usage-contract.md`,
   `docs/agent/v0/review-workflow.md`, `docs/agent/v0/schema/patch.md`,
   `docs/agent/v0/schema/review.md` (promise re-scoped to review),
   `docs/mcp-agent-gateway.md`, and the root `CONTEXT.md` entries for
   MCP Agent Gateway and Agent Patch.
8. **Config back-compat, documented not versioned.** `agentdoc.config.yaml`
   parsing uses `deny_unknown_fields`, so a project that opts into
   `mcp.patch_apply` becomes unreadable by pre-V6.4 binaries. The failure is a
   loud config-parse error and only bites projects that opted in; the config
   `version` is deliberately not bumped for it.

## Consequences

- Agents can close the editing loop only where a human edited the project
  config — a reviewable, diffable artifact, not an environment flag.
- The stable tool list means a disabled project still teaches agents the
  capability exists and exactly how to unlock it.
- Old binaries fail loudly (config parse error) on opted-in repos rather than
  silently ignoring the gate.
- The v0 prompt remains byte-stable; agent harnesses pinned to it see no
  change until they adopt `adoc_propose_patch_v1`.

## Alternatives considered

- **Register the tool only when enabled.** Rejected: tool-list instability
  breaks client caching, and agents on disabled projects cannot discover that
  a gate exists or how to request it.
- **Environment-variable or CLI-flag gate.** Rejected: not a reviewable
  project artifact; opt-in must live where the knowledge lives and travel
  with the repo.
- **Default-enabled with a deny-list.** Rejected: violates the cycle rule
  that agent-initiated writes are opt-in.
- **Bumping `version: 2` in the config for the new key.** Rejected: punishes
  every project that did not opt in; the loud parse failure already protects
  the only projects affected.
- **Repointing the unversioned `adoc_propose_patch` alias at v1.** Rejected:
  ADR-0014 pins the alias's bytes; harnesses that selected the alias chose
  stability.
