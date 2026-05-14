# Promote graph retrieval with a JSON graph artifact

V1 now emits `dist/docs.graph.json` with schema version `adoc.graph.v0`. The artifact is derived from `docs.agent.json`: one node per Knowledge Object, one directed edge per `depends_on`, `supersedes`, or `related_to` relation, plus `agent_artifact_hash` for drift warnings. It is not an authoring source of truth.

This moves graph retrieval earlier than the older roadmap because the relation fields already exist, are validated, and are useful without waiting for includes, nested objects, or a graph database. Deferring traversal made the relation model harder to test in real workflows. Shipping the small read-side artifact now lets `adoc graph` and `adoc search --related-to` prove whether the current relation set is enough before larger graph features are designed.

JSON is the storage choice for this stage. It matches the existing artifact style, is inspectable in Git, needs no new runtime service, and is enough for the current object counts. SQLite, sqlite-vec, or a graph database would add migration, dependency, and operational questions before there is evidence that JSON is too slow or awkward. Those stores remain future options if measured workflows outgrow `docs.graph.json`.

Graph search is explicit candidate filtering, not a default ranking signal. `adoc search --related-to <object-id>` computes the reachable candidate set using optional `--relation` and `--direction` filters, then normal lexical, semantic, or hybrid ranking ranks inside that set. Unfiltered search results stay stable, and there is no implicit graph proximity boost until retrieval evaluation data shows that relation distance should affect score.

The implementation follows ADR-0006 boundaries: pure graph types and traversal live under `domain/graph`, JSON read/write lives under `infrastructure/artifact/graph_json.rs`, and loading/traversal orchestration lives in `application/graph.rs`. Future graph visualization, includes, nested typed blocks, custom schemas, and SQLite-backed traversal remain out of scope for this decision.
