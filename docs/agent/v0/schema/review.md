# AgentDoc Review Report Schema

The V3 review report surface is `adoc.review.v0`.

Review envelopes are returned by `adoc review` and the V3.6 `adoc_review` MCP tool. The envelope wraps the V3.1 object diff with three enriched projections layered on top: source-path impact, required reviewers, and proof obligations.

The envelope's `diff` field embeds a full `adoc.diff.v0` envelope and validates independently against `adoc.diff.v0.schema.json`. The `impact` field lists verified claims, accepted decisions, and verified APIs from the complete head graph whose declared `impacts:` or path-bearing evidence exactly matches the changed file set. Each entry carries the impacted Object ID and deduplicated matching paths; the legacy envelope does not expose reason kinds. The `required_reviewers` field aggregates and deduplicates owners across changed trusted subjects and all impacted objects. The `proof_obligations` field carries re-verify, re-evidence, reassign, demotion, and kind-correct impact-review obligations triggered by the diff's `field_changes` and the impact list; deduplicated by `(object_id, reason)`.

Before producing the envelope, AgentDoc resolves both requested revisions to exact commits, requires one merge base, and validates each snapshot's own `agentdoc.config.yaml`. Each graph is compiled from that snapshot's configured `docs_path`; a config or documentation-root transition is not interpreted using the other side's settings. Missing or invalid configuration and snapshot materialization mismatches fail the review without emitting a partial envelope. These revision and configuration facts remain internal in `adoc.review.v0`, so this is a behavioral correction with no JSON shape change.

The schema stays `v0` across V3.1 through V3.7. New fields land as additive optional JSON keys (e.g. V3.7 will add `patch_check` when the optional `--patch` parameter is supplied).

The review report is read-only: producing it never applies source edits or patches. Patch application is a separate, explicitly invoked surface (`adoc.patch.apply.v0`, V6.4) and is never reachable through `adoc_review`.
