# AgentDoc Review Report Schema

The V3 review report surface is `adoc.review.v0`.

Review envelopes are returned by `adoc review` and the V3.6 `adoc_review` MCP tool. The envelope wraps the V3.1 object diff with three enriched projections layered on top: source-path impact, required reviewers, and proof obligations.

The envelope's `diff` field embeds a full `adoc.diff.v0` envelope and validates independently against `adoc.diff.v0.schema.json`. The `impact` field lists verified Knowledge Objects whose declared `impacts:` paths intersect the changed file set returned by `git diff --name-only <base>...` — the entry carries the impacted Object ID and the changed paths that matched. The `required_reviewers` field aggregates and deduplicates owners across changed verified objects and impacted objects. The `proof_obligations` field carries V3.4 re-verify, re-evidence, reassign, demotion, and impact-review obligations triggered by the diff's `field_changes` and the impact list; deduplicated by `(object_id, reason)`.

The schema stays `v0` across V3.1 through V3.7. New fields land as additive optional JSON keys (e.g. V3.7 will add `patch_check` when the optional `--patch` parameter is supplied).

The review report is read-only: producing it never applies source edits or patches. Patch application is a separate, explicitly invoked surface (`adoc.patch.apply.v0`, V6.4) and is never reachable through `adoc_review`.
