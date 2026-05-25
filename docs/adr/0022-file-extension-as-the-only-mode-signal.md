# ADR-0022: File Extension as the Only Mode Signal

## Status

Accepted.

## Context

V4 introduces a second validation mode — **Compatibility Mode** — alongside the existing **Strict Mode**. The CONTEXT.md glossary has carried "Strict Mode is the only validation mode in v0" as a load-bearing constraint since V0; introducing a second mode demands an explicit decision about how the mode is selected, and what guarantees `.adoc` authors keep.

Three signals could plausibly select the mode: a CLI flag (`adoc check --compat`), a config block (`compatibility.enabled: true` in `agentdoc.config.yaml` per PRD §43.3), or the file extension (`.md` vs `.adoc`). Each has a different invariant cost.

A CLI flag makes the mode invocation-scoped — the same file behaves differently across runs. A config block makes the mode project-scoped — the same file behaves the same across runs but differently across projects, and the controlling state is invisible to the author of any individual file. The file extension is the only signal that makes the mode local to the file itself and visible at the point of authorship.

The PRD frames compatibility mode as a transition aid for teams importing Markdown docs (§28.5: "make compatibility mode a transition aid, not a permanent second dialect"). The progressive-formalization workflow in §28.4 is described as paragraph-to-claim, heading-to-doc, code-to-example transformations — operations that naturally map onto a rename from `docs/billing.md` to `docs/billing.adoc` once the typed structure has been authored. The mode change *is* the file rename.

PRD §43.3 imagines a `compatibility:` config block (`enabled: true`, `preserve_raw_html_as_quarantine: true`) but does not require it. The PRD's intent is satisfied by extension-based dispatch; the config block is an artifact of an earlier design exploration.

The cost of choosing wrong is high. Once teams write `.md` files expecting compatibility-mode behavior, any change that decouples mode from extension breaks them. Once `.adoc` files can be opted into compat via flag or config, the "Verified Knowledge" guarantee for `.adoc` content slowly erodes — a reviewer can no longer look at a `.adoc` file and know it passed strict validation, because the validation depended on invisible config state.

## Decision

The file extension is the only signal that selects validation mode. `.adoc` files are always parsed and validated under Strict Mode. `.md` files are always parsed and validated under Compatibility Mode. No CLI flag, no config block, no front-matter directive, no per-project toggle can change this mapping in V4.

The file discovery glob in `compile_workspace()` expands from `*.adoc` to `*.{adoc,md}`. Both extensions feed the same downstream pipeline; only the parser and validator selection differ.

The PRD's `compatibility:` config block from §43.3 is explicitly out of scope for V4. If a measured need emerges — for example, a team wanting to silence compat warnings in CI without deleting source — it lands in V4.6+ with its own ADR.

## Consequences

Authors and reviewers can look at any file in the project and know with certainty which validation regime applies. A `.adoc` file is strict; a `.md` file is compat; there is no third option and no hidden toggle.

The progressive-formalization workflow is mechanically grounded: extracting durable structure from `docs/billing.md` means writing `docs/billing.adoc` with typed blocks. The rename is the mode upgrade. Tools (editors, CI dashboards, PR review) can rely on the extension as a stable mode signal.

Teams that want a `.adoc` file with relaxed validation during a transition window have no escape hatch in V4. That is intentional pressure — the alternative (relaxable `.adoc` validation) hollows out what "Verified Claim" means across a real codebase. If the transition-window pain proves real and measured, the escape hatch lands as a separate slice with its own ADR, not as a silent behavior change to this one.

There is no way in V4 to ingest a file with a non-standard extension (e.g. `.markdown`, `.mdx`). Discovery is strictly `*.{adoc,md}`. Extending to other extensions is a future decision driven by measured demand.

The constraint "Strict Mode is the only validation mode" in the CONTEXT.md glossary is updated rather than removed: Strict Mode remains the only mode that applies to AgentDoc Source. Compatibility Mode applies to Markdown Source. Both modes coexist in the same project; neither leaks across the file-extension boundary.
