# Managed retrieval runtime input

`adoc managed-retrieve` accepts the closed
[`adoc.managed_retrieval_input.v0`](schema/adoc.managed_retrieval_input.v0.schema.json)
input and emits the existing `adoc.retrieval.v1` JSON envelope. This is the
E6.1.T5 runtime boundary. It does not authenticate a Cloud caller, select current
managed versions, or establish source access. The trusted Cloud caller performs
those checks and constructs the input; an HTTP client cannot supply it.

```sh
adoc managed-retrieve --input input.json --manifest-out contributors.json search billing --mode lexical --top 20
adoc managed-retrieve --input input.json --manifest-out contributors.json why billing.credit
```

The input contains one Workspace, an explicit `RetrievalPolicy`, exact selected
canonical/version bindings, and deduplicated retained graph receipts. The
`graph_bytes` and `content_bytes` fields are UTF-8 strings containing the exact
stored JSON bytes. Their digests use SHA-256 over those string bytes. A node's
authored `content_hash` is a separate semantic identity and is preserved.
Opaque managed identities must be nonempty and unpadded; Cloud's UUID-backed
identities are a narrower admissible set. Every binding must name the same
Workspace and the exact node in its named receipt. Ambiguous identities, changed
bytes, foreign bindings, unsupported graphs and unused receipts fail closed.

Each retained receipt supplies the reference context for its selected objects.
An absent, inactive, unauthorized or mismatched receipt target is non-admitted.
Core applies its existing permission predicate and whole-carrier closure before
indexing. Missing structured references also withhold their owner. Withholding
propagates across selected owners until stable. An old receipt's target cannot
stand in for a different current version with the same Object ID, nor globally
exclude that current version. Historical page prose is never made searchable by
attachment to a selected object. Surviving objects retain their complete authored
fields, body and hash; this is not partial-field redaction or text classification.

The optional manifest file is a private JSON array of
`{canonical:{workspace_id,canonical_id},version_id,receipt_id}` bindings. It
includes every surviving index contributor, including nonhits, and contains no
object bodies. It is separate from stdout and must never be returned to an API
client. Cloud rechecks the current session, scoped authorization, source ACL and
active version for every contributor immediately before releasing the buffered
response. A changed contributor discards that response. Unrelated new or hidden
objects do not invalidate the initial corpus snapshot through a global digest.

The runtime reads only the explicit input (at most 64 MiB). It does not discover
project configuration, consult environment authority, or initialize an embedding
provider. The manifest path must be fresh; an existing path is never replaced.
On Unix, its file mode is `0600`. Use an isolated private directory and remove
both files after the invocation. Input/output failures return zero records and
`retrieval.visibility_unavailable`, without private bytes or filesystem details.

There is no admitted managed vector index in this tracer. Lexical search uses
the existing core ranking. Semantic mode refuses with `search.artifact_missing`;
hybrid mode returns lexical results with a fixed missing-index warning. No vector
or semantic coverage is implied. `why` uses the existing core lookup and treats
non-admitted and absent IDs identically. Exit codes are 0 for success (including
hybrid fallback), 1 for invalid Object ID, 2 for unsafe input/index unavailability,
and 3 for an absent/non-admitted `why` target.

Cloud's first managed route binds a public-only policy plus current native
authorization and source access. Wider audiences, sensitive classification and
auditing belong to E6.1.T6/E6.3. This runtime prerequisite alone does not claim
that the Cloud route is deployed or that sensitive-access delivery is complete.

## Sensitive classification capability (E6.1.T6)

Trusted callers requiring sensitive auditing pass `--require-sensitive-classification`
before the `search` or `why` subcommand. Older runtimes reject the unknown flag.
Every returned KO with `internal` or `restricted` visibility carries that exact
`classification`; absent/public visibility omits it, preserving public bytes.
Plain and styled local retrieval show `Sensitive: internal|restricted`. The one
shared projection supplies CLI and MCP classification; this adds no permission.

The unreleased [`adoc.sensitive_access.v0`](schema/adoc.sensitive_access.v0.schema.json)
event carries real managed workspace and authenticated caller/session UUIDs,
command (`search` or `why`), policy revision UUID, a positive safe-integer scoped
sequence, and nonempty returned-sensitive subjects ordered by Object ID. Object,
canonical and version identities are distinct. The domain validator enforces
these cross-item constraints in addition to the closed schema. Hashes are lowercase
SHA256; bodies, paths, query text, vectors and timestamps are forbidden. Cloud
owns emission/persistence; validation alone does not establish authorization.
MCP authenticated delivery and local CLI exemption are E6.3 work; no local audit
or spool is introduced here.

## Partial fields and direct-source attribution (E6.2.T2)

`--require-field-projection` requires the compatible runtime. The optional
`field_projection` on each selected object carries current native T1 metadata:
exact workspace/canonical/version/content-digest coordinates and 1–100 distinct
`fields[{selector,classification}]` rows. Classification is required and may be
null. An absent extension means no manifest; a present empty projection refuses.
Core validates the object-only shape and exact string targets before filtering.

Null/unreadable body becomes the required empty output string; generic fields
and their aliases are omitted. Authored floors still apply without a manifest.
A protected actual non-null top-level member other than body is a dedicated
carrier: unreadable means whole-object exclusion, readable means its floor joins
the direct classification. No aliases are invented; a name present in both the
generic map and top-level node protects both. Unknown/absent member names do not
create fictional payload. Canonical bytes and content hashes never change.

Hidden body/resolution edges and copied expiry/evidence metadata are removed or
recomputed before reference closure and indexing. Each retained private binding
carries its field projection for current revalidation, even when it is a nonhit.
When surviving visibility metadata or provenance exists, an optional
`accessed_object: {object_id,content_hash,classification}` marks every returned
root and the direct sources of its copied reverse-question, evidence-quality or
contradiction metadata. Public classification is an explicit null. Nonhits retain
bindings without access markers; empty results create no access subjects.

The displayed record classification includes the strictest copied source; its
direct source class can be lower. Native finalization audits the marked actual
source objects, independently verifies direct classes and current authority,
and releases no bytes until the audit commits. It must not substitute a derived
record label for an audited source class. Corpora with neither visibility metadata
nor field provenance preserve their prior envelope and binding bytes.
