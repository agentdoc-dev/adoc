# Cloud writeback record bindings

`agentdoc.cloud.writeback_record.v0` describes a projection originating in the
canonical Cloud store. Schema publication does not implement Cloud admission,
dispatch, authorization, or loop suppression. The references below pin existing
Cloud behavior at reviewed commit
`b853b48cb7596374698c632b22a722845a088333`; they are not new identity decisions.
GitHub links provide review provenance; MCP consumers need not fetch them to
interpret this note.

## Identity values

Portable `adoc` managed-subject types accept opaque strings. Cloud UUID strings
are valid members of that value space. Local `ManagedWorkspace` identifiers such
as `mo-1` are not canonical Cloud records and cannot be used as Cloud writeback
origins without the existing Cloud ingestion and identity binding process.

| Writeback field | Existing Cloud owner |
| --- | --- |
| `origin.subject.canonical.workspace_id` | [`workspaces.id`, UUID](https://github.com/agentdoc-dev/cloud/blob/b853b48cb7596374698c632b22a722845a088333/supabase/migrations/20260722000000_create_workspaces.sql#L1) |
| `origin.subject.canonical.canonical_id` | [`managed_object_identities.canonical_id`, UUID](https://github.com/agentdoc-dev/cloud/blob/b853b48cb7596374698c632b22a722845a088333/supabase/migrations/20260822000000_create_managed_object_identities.sql#L6) |
| `origin.subject.version_id` | [`managed_object_versions.version_id`, UUID](https://github.com/agentdoc-dev/cloud/blob/b853b48cb7596374698c632b22a722845a088333/supabase/migrations/20260824980000_create_canonical_governance_store.sql#L645) |
| `target.connector_id` | [`source_connectors.id`, UUID](https://github.com/agentdoc-dev/cloud/blob/b853b48cb7596374698c632b22a722845a088333/supabase/migrations/20260824800000_create_source_acl_freshness.sql#L84) |
| `target.source_binding_id` | [`source_bindings.source_binding_id`, exported text](https://github.com/agentdoc-dev/cloud/blob/b853b48cb7596374698c632b22a722845a088333/supabase/migrations/20260824975000_create_source_provenance_store.sql#L1056) |
| `target.source_binding_digest` | [`source_bindings.record_digest`, SHA-256 of retained `record_bytes` (bytea)](https://github.com/agentdoc-dev/cloud/blob/b853b48cb7596374698c632b22a722845a088333/supabase/migrations/20260824975000_create_source_provenance_store.sql#L1094) |

The binding digest is `'sha256:' || encode(extensions.digest(record_bytes,
'sha256'), 'hex')`. The admission port retains the exact supplied UTF-8
`adoc.source_binding.v0` envelope bytes. Verification hashes those retained
bytes directly, without JSON reserialization. It does not hash the database row,
the textual binding ID, or the source-revision content.

The connector instance and provider label are distinct. Cloud's existing
[`source_binding_record_is_bound`](https://github.com/agentdoc-dev/cloud/blob/b853b48cb7596374698c632b22a722845a088333/supabase/migrations/20260824975000_create_source_provenance_store.sql#L1005)
requires the Source Record's `connector_id` to equal the instance UUID and its
`source_provider` to equal `binding.connector`. Thus a binding naming provider
`github` can refer to a UUID-backed GitHub connector instance without changing
either identity. The same check binds source identity, observed revision when
the binding declares one, and source-content digest.

## Event identity and digest

`origin.event_ordinal` is the zero-based position in the origin Workspace's log.
The [append guard](https://github.com/agentdoc-dev/cloud/blob/b853b48cb7596374698c632b22a722845a088333/supabase/migrations/20260822200000_create_managed_state_events.sql#L170)
assigns zero at genesis and increments within that Workspace. The store-global
`seq` is not exported. Admission must match Workspace, ordinal, exact managed
subject and chained `record_digest` on the same retained event.

The [existing digest construction](https://github.com/agentdoc-dev/cloud/blob/b853b48cb7596374698c632b22a722845a088333/supabase/migrations/20260822200000_create_managed_state_events.sql#L249)
first verifies that `event_bytes::jsonb` equals the exact managed-state object,
then computes:

```sql
'sha256:' || encode(
  extensions.digest(convert_to(head_digest || new.event_bytes, 'utf8'), 'sha256'),
  'hex'
)
```

Despite its name, `event_bytes` is stored JSON **text**, not arbitrary binary.
`head_digest` is empty at genesis or the preceding 71-character
`sha256:<lowercase hex>` string. A proposed genesis preimage starting with
`sha256:<digest>` followed by another event is not a JSON object and is rejected
before hashing. Non-genesis prefixes have a fixed length. This contract reuses
that retained digest; it does not introduce or reinterpret a hashing scheme.

## Refusals and request identity

The [existing Cloud transport](https://github.com/agentdoc-dev/cloud/blob/b853b48cb7596374698c632b22a722845a088333/supabase/migrations/20260825000000_create_api_v1_transport.sql#L1)
uses `^[!-~]{1,200}$`: 1–200 visible ASCII characters, without spaces. Its key
scope is Workspace plus operation. The planned writeback operation adopts this
key format and scope; it does not assume that the existing request table accepts
writeback records unchanged. The same key retries the same complete record and
`writeback_id`; different lineage or payload under that key uses
`api.idempotency_conflict`, already registered in the AgentDoc contract registry
(`docs/roadmap/v10/CONTRACT-REGISTRY.md`).

Missing or malformed lineage, including a missing target revision precondition,
is `api.invalid_request`. The schema does not prove stored joins or payload
bytes. Cloud admission must reject impossible ordinals before database conversion
and apply its bounded request admission to source identifiers and native
revisions; this schema does not invent new per-field source identifier limits.
