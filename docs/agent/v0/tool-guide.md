# AgentDoc Tool Guide

V2.2 MCP tools are the supported local agent workflow for AgentDoc projects.

## Tool Order

1. `adoc_project_status` with `refresh: "none"` to inspect readiness.
2. `adoc_project_status` with `refresh: "check"` when source diagnostics are needed without writes.
3. `adoc_project_status` with `refresh: "build"` or `adoc_build` when artifacts are missing or stale.
4. `adoc_search`, `adoc_why`, and `adoc_graph` for evidence.
5. `adoc_patch_check` for any proposed `adoc.patch.v0` document.

`refresh: "build"` follows the same local build behavior as `adoc_build`. Embeddings honor project config unless `no_embeddings` is true. If project status returns artifact diagnostics, carry them into the answer or handoff; `search.deterministic_quality` means the project is using repeatable hash embeddings rather than semantic-model quality.

## Standalone retrieval policy (E6.1.T1)

`search` and `why` discover local retrieval policy even with an explicit
`--artifact`. For example, an operator can configure:

```yaml
retrieval_policy:
  audience: public
  allowed_visibilities: [public]
  excluded_object_ids: [billing.internal-runbook]
```

This block belongs in the existing `agentdoc.config.yaml`. It narrows the
`search` and `why` corpus before ranking; excluded IDs behave like absent IDs
on those paths. Local
policy is trusted operator configuration, not a multi-user authentication
boundary. Unknown policy keys fail closed. Existing unclassified repositories
without the block preserve their existing retrieval behavior.

This first projection conservatively matches denied ID text in complete
Knowledge Objects and prose blocks. That can also withhold namespace descendants,
similarly prefixed IDs, or records citing them (for example, excluding `billing.target`
can withhold `billing.target-rules`). It does not rewrite governed statements.
Each Knowledge Object and prose block is checked in full, including metadata
that contributes to its hash or embedding. Page nodes are not retrieval records
and are outside this scan. A withheld carrier's precomputed vector is also
removed; retained records keep their original source fields and content hashes. Vectors
are admitted only when their kind and Embedding Composition hash match the
current record; rebuild a stale search artifact to restore missing vectors.
When only some permitted vectors are stale, semantic search uses the remaining
valid vectors and exits 0 with a `search.hash_drift` warning that semantic results
may be incomplete. Hybrid search can still retrieve changed records through
lexical matching. Rebuild the search artifact to restore semantic coverage.
If stale bindings leave no usable vectors for permitted records, `--semantic`
fails with `search.artifact_missing` and exit 2; hybrid search uses lexical
results. An empty corpus caused only by withheld or absent records does not
trigger this stale-index failure.

E6.1.T1 enforces this policy on `search` and `why` only. `graph`, `stale`,
`contradictions`, and `impacted-by` still read the unfiltered artifact until
the graph-driver tracer E6.1.T3 lands. A CLI regression explicitly pins this
transitional unfiltered graph behavior; T3 must replace that assertion with
policy exclusion. MCP gateway parity follows in E6.1.T4.
T1 also withholds artifact warnings when records are excluded; diagnostic
presence/absence parity is part of T3's whole-response closure, not a T1 guarantee.
When visibility or explicit policy supplies excluded IDs, carried source errors
refuse retrieval with one generic `retrieval.visibility_unavailable` error and
`adoc check` / `adoc build` guidance. Search and why return no records and exit 2.
With no excluded IDs, legacy carried diagnostics and CLI refusal on carried
errors remain unchanged pending T3’s diagnostics compatibility decision.
An all-public corpus with carried source errors also returns no records and
exit 2; adding an internal object changes diagnostic disclosure, not availability.
Structural corruption or a carried `schema.visibility_invalid` diagnostic still
refuses retrieval, even without a configured policy.

Upgrade note: objects already authored with `visibility: internal` or
`visibility: restricted` now require an explicit policy to appear in `search`
and `why`. Without that policy, public or unclassified objects can also
disappear when their serialized content refers to denied objects. This includes
relation targets, fields, evidence references or text, and contradiction claim
IDs. The whole object is withheld; its fields are not partially redacted while
its old hash or vector remains available. Withholding is transitive: if public
object A depends on internal object B, and public object C depends on A, the
default policy withholds B, A, and C.

A trusted local operator authorized for all three classes can replace the
retrieval policy with the following block in `agentdoc.config.yaml`. It restores
internal/restricted objects and their public dependents by authorizing every
visibility class and clearing explicit exclusions. This recipe applies to valid
classifications; `schema.visibility_invalid` still requires source repair and a
rebuild:

```yaml
retrieval_policy:
  audience: restricted
  allowed_visibilities: [public, internal, restricted]
  excluded_object_ids: []
```
