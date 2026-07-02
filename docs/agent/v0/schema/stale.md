# AgentDoc Stale Query Schema

The V6.1 lifecycle-signal query surface is `adoc.stale.v0`.

Stale envelopes are returned by `adoc stale` and the `adoc_stale` MCP tool. The envelope lists Knowledge Objects whose lifecycle signals warrant attention, derived from the graph artifact **at read time** against the `evaluated_at` date — never from the artifact's build-time `effective_status` projection, so a week-old artifact still reports staleness as of the query date.

Three record categories, all from one artifact pass:

- `stale` — the object's `expires_at` is strictly before `evaluated_at`. Listed for **any** authored status (matching the compile-time `lifecycle.expired` rule); the record's `effective_status` re-derives `stale` only for verified objects and otherwise echoes the authored status.
- `review_overdue` — an `active` policy whose `effective_at + review_interval` is strictly before `evaluated_at`, with `days_overdue` counted from that due date.
- `expiring_soon` — only with a `within` horizon (CLI `--within <N>d`, MCP `within_days`): a verified object whose `expires_at` falls between `evaluated_at` (inclusive) and the horizon, with `days_remaining`.

Records sort most-overdue first, then by Object ID; one object can legitimately yield two records (an expired active policy that is also overdue for review). `reason` is machine-readable: `expired:<date>`, `review_due:<date>`, or `expires:<date>`.

The query is not a gate: exit code 0 (and a normal envelope) whether or not records exist. Non-zero exit codes occur only on artifact-load failure, with fix-oriented diagnostics and an empty `records` array.

`authored_status` is the lifecycle status only; records for kinds without a lifecycle (`warning`/`constraint`/`agent_instruction`) omit it, per ADR-0039.
