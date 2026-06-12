# AgentDoc Impacted-By Query Schema

The V6.3 source-path impact query surface is `adoc.impacted.v0`.

Impacted envelopes are returned by `adoc impacted-by` and the `adoc_impacted_by` MCP tool. They answer "this code changed — which knowledge is now suspect?" over the current graph artifact: no recompile, no snapshot worktree, no globs. Exactly one input shape per query:

- explicit changed paths (`adoc impacted-by <path>...` / MCP `paths`), repo-relative as emitted by `git diff --name-only`;
- a git ref (`adoc impacted-by --ref <git-ref>` / MCP `ref`), deriving the changed set as the base ref against the working tree — the same selector shape as `adoc review <ref>`.

Only **verified subjects** appear: claims with status `verified` and decisions with status `accepted` (the V3.3 impact scope — a draft is already untrusted, so flagging it adds nothing). Each record carries deduplicated `reasons`:

- `impacts_path` — the object's declared `impacts:` list contains the changed path.
- `evidence_path` — an inline `source_code`/`test` evidence value equals the changed path, or the `path` of a referenced `source` object does; the latter carries `via_source_object` naming the source object. The same path reached via `impacts:` and via evidence yields two reasons on one record.

Every impacted record carries one impact-review proof obligation (`required_evidence: ["source_code"]`) in the top-level `proof_obligations`, merged and sorted — the same rule `adoc review` applies to its impact list.

The envelope is a pure function of the artifact bytes and the changed-path set: no evaluation date, byte-identical output for the same inputs on any day. `changed_paths` echoes the normalized query (sorted ascending, deduplicated); records sort by Object ID; reasons sort by `(matched_path, kind, via_source_object)`.

The query is not a gate: exit code 0 (and a normal envelope) whether or not anything is impacted. Exit 1 marks user-input errors (`impacted.invalid_path`, `impacted.ref_unresolvable`); exit 2 marks environment errors (`impacted.git_unavailable`, artifact-load failure) — in every case the envelope still ships with fix-oriented diagnostics and empty listings.
