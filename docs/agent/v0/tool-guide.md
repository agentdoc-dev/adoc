# AgentDoc Tool Guide

V2.2 MCP tools are the supported local agent workflow for AgentDoc projects.

## Tool Order

1. `adoc_project_status` with `refresh: "none"` to inspect readiness.
2. `adoc_project_status` with `refresh: "check"` when source diagnostics are needed without writes.
3. `adoc_project_status` with `refresh: "build"` or `adoc_build` when artifacts are missing or stale.
4. `adoc_search`, `adoc_why`, and `adoc_graph` for evidence.
5. `adoc_patch_check` for any proposed `adoc.patch.v0` document.

`refresh: "build"` follows the same local build behavior as `adoc_build`. Embeddings honor project config unless `no_embeddings` is true. If project status returns artifact diagnostics, carry them into the answer or handoff; `search.deterministic_quality` means the project is using repeatable hash embeddings rather than semantic-model quality.

## Standalone retrieval policy (E6.1.T1–T2)

The configuration discovery below applies to the CLI. The MCP gateway uses a
public-only default or an explicit startup `adoc-mcp --config PATH` binding.
Its operator-owned file must contain a valid `retrieval_policy`; selecting another
project or artifact cannot widen that audience. Restart the gateway after changing
its authority configuration. See the [MCP Agent Gateway guide](../../guides/mcp-agent-gateway.md#bind-retrieval-authority).

For connected sensitive MCP retrieval, start the gateway from its served repository:

```sh
adoc-mcp --config gateway.yaml --audit-config audit.json
```

The separate operator-owned `audit.json` has exactly these fields:

```json
{
  "cloud_url": "https://cloud.example",
  "workspace_id": "10000000-0000-4000-8000-000000000001",
  "repository_id": "10000000-0000-4000-8000-000000000002",
  "bearer_token_file": "human-session-token"
}
```

The token file is resolved relative to `audit.json` and contains the authenticated
human session bearer token. Cloud binds the registered repository and actual human,
auth session, gateway session, and effective local-policy digest. The gateway fixes
one canonical served root; tool arguments cannot select another root or identity.
Only HTTPS origins are accepted, except literal loopback HTTP for local integration.

`search`, `why`, `graph`, `stale`, `contradictions`, and `impacted_by` record exact
sensitive Object IDs, original content hashes and classes before returning content.
Each attempt first checks current Cloud audit-metadata egress permission. Successful
sensitive results append `Sensitive (internal).` or `Sensitive (restricted).` without
changing their existing envelopes. Public, no-hit and denied results emit no event.
Unavailable recording returns the machine-readable `retrieval.audit_sink_unavailable`
refusal and withholds sensitive output. Ambiguous acknowledgement is retried with the
same event bytes before a newly prepared response can receive its own event.
This connected cut has no durable offline spool; direct single-user CLI retrieval
requires no gateway auditor.

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

An unreadable config, malformed YAML, or malformed policy block returns
`retrieval.policy_invalid`. A dangling config symlink cannot select a parent or
default policy.
A present policy with a missing or invalid audience returns
`retrieval.audience_unresolved`. Explicit null policy is invalid; omit the block
to use the public/unclassified default. Invalid present object or field visibility,
including null, returns `retrieval.visibility_unavailable`. Search and why return
no records and exit 2 with these typed diagnostics, in JSON or plain output.
Repair the config or classification and rebuild invalid artifacts before retrying.

Malformed policy also refuses non-retrieval commands that load config, including
`build` and `check`, with a `retrieval.policy_invalid` or
`retrieval.audience_unresolved` error and exit 1. This includes null policy
placeholders: remove the block instead. Existing explicit-input paths that bypass
config still do so; search/why always load policy and use exit 2 as above.

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

The same core projection enforces this policy on `search`, `why`, `graph`,
`stale`, `contradictions`, and `impacted-by` before indexes or response
projections are assembled. Explicit CLI artifact paths still discover project
policy; MCP uses its explicit startup binding instead.
Retrieval omits carried artifact warnings even when no records are excluded.
Carried source errors and graph-index validation failures refuse retrieval with
one generic `retrieval.visibility_unavailable` error and `adoc check` / `adoc build`
guidance, without original details or counts. Search and why return no records
and exit 2. Structural corruption or a carried `schema.visibility_invalid`
diagnostic still refuses retrieval, even without a configured policy.

E6.1.T3 intentionally changes legacy retrieval diagnostics to avoid disclosing
hidden-record presence through conditional warnings or error text. Full source
diagnostics remain available from `adoc check` and `adoc build`. Whole-corpus
manifest hash differences no longer produce retrieval warnings; stale permitted
vectors still do. Successful plain/styled `why` keeps its artifact/trust footer
and omits execution time. This does not replace the separate coarse timing check.

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

For MCP, pass this file explicitly with `adoc-mcp --config PATH` and restart
the gateway; changing a tool-selected project's file alone does not change its audience.
