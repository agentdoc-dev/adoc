# AgentDoc Retrieval Schema

The V2.2 retrieval answer surface is `adoc.retrieval.v0`.

Retrieval envelopes are returned by `adoc_why` and `adoc_search`. Records are projections of Knowledge Objects from the graph artifact and include Object ID, kind, content hash, optional status/evidence fields, body, source location, relations, and optional match metadata. Fields skipped by serde when absent or empty are optional in the schema.

Agents should cite retrieval records instead of private graph/search DTOs.
