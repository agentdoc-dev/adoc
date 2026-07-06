# AgentDoc Retrieval Schema

The retrieval answer surface is `adoc.retrieval.v1` (V1.7.1, ADR-0040). The
legacy `adoc.retrieval.v0` schema stays published at
`adoc://agent/v0/schema/retrieval-envelope.v0.json`.

Retrieval envelopes are returned by `adoc_why` and `adoc_search`. Every record
carries a `record_type` discriminator:

- `record_type: "knowledge_object"` — a projection of a Knowledge Object from
  the graph artifact: Object ID, kind, content hash, optional
  status/evidence fields, body, source location, relations, and optional
  match metadata. Field-identical to the v0 record apart from the
  discriminator. `adoc_why` returns these only.
- `record_type: "prose"` — a prose block hit from blended search: positional
  block id (`<page-id>#block-NNNN`), `page_id`, `block_kind`
  (`heading | paragraph | list | code_block`), `text`, optional
  `heading_context` breadcrumb, source location, and match metadata. Prose
  records carry no `content_hash`, no `status`, and no `relations`, and their
  ids cannot be fed to `adoc_why`. Block ids are positional and renumber when
  a page is edited — do not persist them.

Fields skipped by serde when absent or empty are optional in the schema.

Agents should cite Knowledge Object retrieval records instead of private
graph/search DTOs. Prose records are orientation context, never citable
verified knowledge — see `adoc://agent/v0/answer-contract`.
