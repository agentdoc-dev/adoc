# Use a single V0 core compile API

AgentDoc V0 will expose one high-level `compile_workspace()` entry point from `adoc-core`, while parser, validator, renderer, and artifact emission stay as internal modules. This keeps the first public library contract focused on the CLI's vertical workflow; lower-level APIs can be exposed later when language-server, web preview, semantic diff, or integration needs are concrete.

## Addendum (v0.x): DiagnosticCode joins the pinned surface

`DiagnosticCode` is promoted from `pub(crate)` to `pub` and `Diagnostic.code` becomes the typed enum (was `String`). Hosts can now pattern-match on diagnostic codes instead of comparing strings; the wire format (`parse.raw_html`, `io.unreadable_file`, …) remains byte-identical via a manual `Serialize` impl that emits `DiagnosticCode::as_str()`. The new public items are pinned in `tests/public_surface.rs` next to the existing `Diagnostic`, `Severity`, `CompileResult`, etc. Adding a new variant is a deliberate v0.x widening — it changes the pinned set and so requires updating both this addendum and the test.
