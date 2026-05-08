# Use a single V0 core compile API

AgentDoc V0 will expose one high-level `compile_workspace()` entry point from `adoc-core`, while parser, validator, renderer, and artifact emission stay as internal modules. This keeps the first public library contract focused on the CLI's vertical workflow; lower-level APIs can be exposed later when language-server, web preview, semantic diff, or integration needs are concrete.

## Addendum (v0.x): DiagnosticCode joins the pinned surface

`DiagnosticCode` is promoted from `pub(crate)` to `pub` and `Diagnostic.code` becomes the typed enum (was `String`). Hosts can now pattern-match on diagnostic codes instead of comparing strings; the wire format (`parse.raw_html`, `id.invalid`, `io.unreadable_file`, …) remains byte-identical via a manual `Serialize` impl that emits `DiagnosticCode::as_str()`. The public set is pinned in `tests/public_surface.rs` next to the existing `Diagnostic`, `Severity`, `CompileResult`, etc. Adding a new variant is a deliberate v0.x widening — it changes the pinned set and so requires updating both this addendum and the test.

V0.5 adds `DiagnosticCode::RefBroken` with wire code `ref.broken` for object references and relation targets that use valid Object ID grammar but do not resolve to any declared Knowledge Object in the scanned workspace.

V0.5 also adds `DiagnosticCode::IoUnsupportedSourceExtension` with wire code `io.unsupported_source_extension` for existing single-file compile roots whose extension is not exactly `.adoc`. Directory scans continue to ignore non-`.adoc` files silently; missing paths and ordinary read failures remain `io.unreadable_file`.

V0.5 also adds `DiagnosticCode::IoUnreadableDirectory` with wire code `io.unreadable_directory` for directories that exist but cannot be traversed during recursive source discovery. Missing roots and ordinary file read failures remain `io.unreadable_file`.

V1.5 adds `DiagnosticCode::LifecycleInvalidExpiresAt` with wire code `lifecycle.invalid_expires_at` for Knowledge Objects whose `expires_at` field is present but not parseable as `YYYY-MM-DD`.
