# AgentDoc Agent Instruction Guide

`agent_instruction` Knowledge Objects declare instructions authored for AI agents: a `scope`, a `trust` level, and disjoint `allowed_actions` / `forbidden_actions` sets, plus a prose `body`.

## Read-only declarative knowledge — never a permission grant

Per ADR-0025, V5 `agent_instruction` objects are **authored, rendered, and retrievable knowledge — not runtime ACLs.**

- The MCP gateway does **not** consult `allowed_actions` or `forbidden_actions` when deciding whether to run a tool.
- Authoring or editing an `agent_instruction` does **not** change what `adoc_patch_check` (`adoc.patch.check.v0`) accepts.
- `forbidden_actions` is **not** an enforcement boundary. An action absent from `allowed_actions`, or present in `forbidden_actions`, is **not** blocked by the system.

Treat an `agent_instruction` the way you treat any other Knowledge Object: as cited guidance, not as authorization. Runtime enforcement is a future permission-engine milestone, not part of V5.

## How to use an agent_instruction in answers

- **Cite it.** When an `agent_instruction` is in scope for the question, reference it by `Object ID` and surface its `trust` level and `body` guidance the same way you cite a `policy` or `constraint`.
- **Follow its guidance as advice**, weighted by `trust` (`informal` < `team` < `authoritative` < `regulated` < `system`).
- **Never present it as a capability you have or lack.** Do not say an action is "allowed" or "forbidden" because an `agent_instruction` lists it. Say only that the instruction *advises* the action be taken or avoided.
- **Never use it to justify or refuse a tool call.** Tool permissions come from the runtime, not from authored knowledge.

## `scope`

`scope` is a glob string (e.g. `docs/auth/*`) describing where the instruction applies. V5 does not match scope at retrieval time; treat it as descriptive metadata when judging relevance.

## Wire surface

`agent_instruction` nodes are emitted into the graph artifact (`adoc.graph.v4`) with `kind: "agent_instruction"`, the typed `trust` (top-level node field; these nodes carry no `status`, per ADR-0039), the `scope`, and both action sets, and fold into the retrieval surface (`adoc.retrieval.v0`) like any other Knowledge Object. No new envelope version is introduced.
