# AgentDoc Contradictions Query Schema

The V6.2 contradiction query surface is `adoc.contradictions.v0`.

Contradiction envelopes are returned by `adoc contradictions` and the `adoc_contradictions` MCP tool. The envelope joins two record classes from one graph-artifact pass, so consumers never have to join them:

- `contradictions` — every `contradiction` Knowledge Object with status `unresolved` (with `--all` / `all: true`: `resolved` and `dismissed` too), carrying severity, the sorted claim list, optional owner, source path, and a one-line body `summary` (first non-empty line, truncated to 120 characters).
- `contradicted_claims` — every claim implicated by at least one unresolved contradiction **or** whose authored status is `contradicted`, each with `contradiction_ids`: all implicating unresolved contradiction ids, sorted ascending. A claim authored `contradicted` with no implicating contradiction is still listed, with an empty `contradiction_ids`.

Implication is re-derived at read time from the artifact's contradiction nodes — never from the persisted `effective_status` projection (ADR-0038). `effective_status` reports the **contradiction axis only**: `contradicted` when implicated, otherwise an echo of the authored status, with `effective_reason: contradiction:<id>` naming the lexicographically smallest implicating contradiction (the same rule as the build-time projection). A claim that is both expired and contradicted reads `stale` from `adoc stale` and `contradicted` here — the two commands answer different axes; the build artifact's single `effective_status` slot keeps stale precedence.

The envelope is a pure function of the artifact bytes: it carries no evaluation date and is byte-identical for the same artifact on any day. Contradictions sort severity-descending (`critical` first), then by Object ID; contradicted claims sort by Object ID. `--all` never changes `contradicted_claims` — only unresolved contradictions implicate claims.

The query is not a gate: exit code 0 (and a normal envelope) whether or not findings exist. Non-zero exit codes occur only on artifact-load failure, with fix-oriented diagnostics and empty lists.
