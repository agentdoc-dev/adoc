# AgentDoc Migration Report Schema

The V8.1.2 migration report surface is `adoc.migrate.report.v0`.

Migration report envelopes are emitted by `adoc migrate --format json` — there is no MCP migrate tool; migration is a human onboarding act, not an agent loop step. The envelope carries the PRD §28.3 counts, per-file entries, a suggested-next-steps list, and every diagnostic the run emitted.

Counts reconcile 1:1 with diagnostics (ADR-0043 §4): `raw_html_quarantined`, `broken_links`, and `unrecognized_extensions` each equal the number of diagnostics in the envelope with the matching `migrate.*` code — the report never claims what the diagnostics don't show. `compat.*` diagnostics travel in `diagnostics` but belong to no count bucket. Note that `unrecognized_extensions` is a diagnostic tally, not a quarantine count: it covers quarantined constructs (preserved verbatim in fenced code blocks) and diagnosed drops (front matter, task-list checkbox markers, empty parser artifacts) alike; the quarantine subset is identifiable by the "preserved verbatim in a fenced code block" phrase in the diagnostic message. `files_imported` is the migrated-file count and `pages_created` equals it (one prose-mode page per source); `prose_blocks` sums the per-file serialized fragment counts — every rendered block (headings, lists, code and quarantine fences included), not only prose paragraphs; `suggested_typed_blocks` equals the length of the top-level `suggestions` array — the same reconcile-by-construction discipline.

Per-file entries are `{source, target, written, prose_blocks}` — `written` is one value across all files (`--write` is all-or-nothing, ADR-0043 §3). Diagnostic source spans live on the top-level `diagnostics` array, keyed by `span.file`: a single reconciliation truth, not a per-file copy.

`suggested_next_steps` holds one deterministic rule per nonzero count in a fixed order — rules, not weights; empty when nothing fires.

`suggestions` (V8.1.3) holds the PRD §28.4 typed-block candidates as records `{span, suggested_kind, matched_rule, excerpt}` — rules, not weights: a suggestion either matches a named rule (`todo_line` → `task`, `numbered_step_list` → `procedure`, `warning_phrase` → `warning`, `definitional_paragraph` → `glossary`, `assertive_modal` → `claim`) or does not exist; there are no confidence scores. Rules run first-match-wins per block in that fixed order, so each block yields at most one suggestion. Suggestions are report records only — migrated output never contains a typed block regardless of suggestion count; the human types the block.

Exit codes: 0 for a clean or warning-only run (dry-run and `--write` alike), 1 when any ERROR diagnostic is present (`migrate.source_not_committed`, `migrate.target_exists`, unrepresentable content) — the envelope is still emitted, carrying the refusal diagnostics. An I/O failure while executing `--write` surfaces as a CLI error without an envelope.

The envelope is experimental until the V8.1 milestone closes at V8.1.4 (V8.1.3 added suggestion records additively).
