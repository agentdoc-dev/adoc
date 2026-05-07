# V1 Design

This document is the implementation contract for V1: local retrieval with embeddings. It is the V0-DESIGN equivalent for the next milestone — small enough to start coding, large enough that the embedding artifact, retrieval indexes, and CLI surface are decided before any new module lands.

V1 builds directly on the V0 compiler. It does not change the parser, validator, or rendering pipeline. It adds:

- A second build artifact, `dist/docs.search.json`, that carries one embedding per **Knowledge Object** plus a content hash and a model header.
- A retrieval module inside `adoc-core` that loads the agent and search artifacts, validates them, and exposes lookup, lexical, and vector indexes behind an internal hybrid ranker.
- Two new CLI commands, `adoc explain` and `adoc search`, that read those artifacts only — they never re-run `compile_workspace()`.
- One new internal port, `EmbeddingProvider`, with a default local adapter and a deterministic in-memory adapter for tests.

## Goals

- Promote `dist/docs.agent.json` from a build byproduct to a supported read model with a stable read contract.
- Ship per-Object-ID embeddings as a first-class build output, not a deferred feature.
- Provide hybrid lexical + vector retrieval with no tunable score weights in V1.
- Keep retrieval read-only over compiled artifacts; source recompilation belongs to `adoc check` and `adoc build`.
- Keep the V0 thesis intact: typed Knowledge Objects, not chunked text, are the citation unit.
- Preserve existing public API: `compile_workspace()` remains the single entry point in `adoc-core`; new retrieval functionality is added behind it, not around it.

## Non-Goals

- No SQLite, LanceDB, or embedded ANN library. Brute-force cosine over a flat embedding list is sufficient at pilot scale.
- No hosted embedding API in V1. The pluggable port is designed so a hosted adapter can land later without API churn.
- No body chunking. One embedding per **Knowledge Object**.
- No score-weight knobs. Lifecycle, freshness, evidence quality, and authority are filters in V1, not score modifiers.
- No agent server (MCP, JSON-RPC, HTTP). `adoc search --format json` is the wire format an external server would later wrap.
- No `adoc init`, no config file, no source-aware retrieval. Those stay in V1.5 / V2.
- No incremental partial builds across runs other than the per-object embedding cache. HTML and agent JSON regenerate every `adoc build` as in V0.

## Workspace Layout

V1 adds modules; it does not move existing ones.

```text
crates/adoc-core/src/
  application/
    compile.rs                       # extended: emits search artifact too
    retrieval.rs                     # NEW: orchestrates loading + querying
  domain/
    artifact.rs                      # extended: add SearchArtifact types
    knowledge_object/                # unchanged
    ports/
      artifact_writer.rs             # unchanged (agent-json writer)
      embedding_provider.rs          # NEW: EmbeddingProvider port
      renderer.rs                    # unchanged
      source_provider.rs             # unchanged
    retrieval/                       # NEW: pure retrieval domain types
      mod.rs
      retrieval_record.rs            # the citation-shaped record
      lexical_index.rs               # BM25 index (pure, in-memory)
      vector_index.rs                # cosine index (pure, in-memory)
      hybrid_ranker.rs               # RRF fuser (pure)
      filter.rs                      # filter predicates
  infrastructure/
    artifact/
      agent_json.rs                  # unchanged
      search_json.rs                 # NEW: SearchArtifact writer/reader
    embedding/                       # NEW: embedding adapters
      mod.rs
      fastembed.rs                   # FastEmbedProvider (default)
      in_memory.rs                   # InMemoryProvider (deterministic)
crates/adoc-cli/src/
  main.rs                            # extended: explain + search subcommands
  commands/
    explain.rs                       # NEW
    search.rs                        # NEW
```

Guidance:

- `domain/retrieval/` holds pure data and pure ranking. It must not import `infrastructure/`.
- `infrastructure/embedding/` is the only side-effecting boundary at build time.
- Retrieval reads happen exclusively in `application/retrieval.rs`, which composes ports into a `RetrievalSession` value used by both CLI subcommands.

## Public Core API Additions

V0's single entry point is preserved. V1 adds three:

```rust
// V0, unchanged.
pub fn compile_workspace(input: CompileInput) -> CompileResult;

// V1 additions.
pub fn load_retrieval_session(input: RetrievalInput) -> RetrievalLoadResult;
pub fn explain_object(session: &RetrievalSession, id: &str) -> ExplainResult;
pub fn search(session: &RetrievalSession, query: SearchQuery) -> SearchResult;

pub struct RetrievalInput {
    pub artifact_path: std::path::PathBuf,        // dist/docs.agent.json
    pub search_artifact_path: Option<std::path::PathBuf>, // dist/docs.search.json
}

pub struct RetrievalLoadResult {
    pub session: Option<RetrievalSession>,
    pub diagnostics: Vec<Diagnostic>,
}

pub struct SearchQuery {
    pub text: String,
    pub mode: SearchMode,             // Hybrid | Lexical | Semantic
    pub top: std::num::NonZeroUsize,  // default 10
    pub filters: SearchFilters,
}

pub struct SearchFilters {
    pub kind: Option<String>,
    pub status: Option<String>,
    pub owner: Option<String>,
    pub source_path: Option<String>,
}

pub struct SearchResult {
    pub records: Vec<RetrievalRecord>,
    pub diagnostics: Vec<Diagnostic>,
}
```

Rules:

- `load_retrieval_session` validates both artifacts before returning a session. A missing search artifact downgrades a `Hybrid` query to lexical-only, with a `search.artifact_missing` warning attached to the search result; it is never a hard error for `adoc explain`.
- Sessions are immutable. The CLI loads once per command invocation. No global state.
- All three functions return diagnostics in the same `Diagnostic` shape used by `compile_workspace()`. CLI formatting code stays uniform.

## CLI Contract

```bash
adoc check <path>
adoc build <path> --out dist [--no-embeddings]
adoc explain <object-id>
  [--artifact <path>]               # default dist/docs.agent.json
  [--format text|json]              # default text
adoc search "<query>"
  [--artifact <path>]               # default dist/docs.agent.json
  [--search-artifact <path>]        # default dist/docs.search.json
  [--kind <kind>]
  [--status <status>]
  [--owner <owner>]
  [--source-path <substring>]
  [--top <N>]                       # default 10
  [--lexical | --semantic]          # default = hybrid
  [--format text|json]              # default text
```

Behavior:

- `adoc build` writes `dist/docs.html`, `dist/docs.agent.json`, and `dist/docs.search.json`. With `--no-embeddings`, the search artifact is skipped and a `build.embeddings_skipped` info diagnostic is emitted.
- `adoc explain` reads only the agent artifact. It exits `0` on success, `1` on argument errors, `2` on artifact errors, and `3` on object not found.
- `adoc search` reads both artifacts. Empty result sets exit `0` with an explicit `(no matches)` line; argument errors exit `1`; artifact errors exit `2`. There is no separate "no results" exit code.
- `--format json` emits a stable JSON envelope: `{ "schema_version": "adoc.retrieval.v0", "records": [...], "diagnostics": [...] }`. This is the wire format a future MCP wrapper consumes; it must not change shape inside V1.

## Search Artifact

`dist/docs.search.json` is the canonical embedding artifact.

```json
{
  "schema_version": "adoc.search.v0",
  "model": {
    "id": "bge-small-en-v1.5",
    "provider": "fastembed",
    "dim": 384
  },
  "agent_artifact_hash": "sha256:...",
  "embeddings": [
    {
      "id": "billing.credits.decrement-after-success",
      "content_hash": "sha256:...",
      "vector": [0.0123, -0.0451, ...]
    }
  ]
}
```

Rules:

- One entry per **Knowledge Object** ID. Pages are not embedded in V1.
- `content_hash` is computed over the canonical embedding input string (see *Embedding Composition* below). When `adoc build` runs and the new content hash matches the prior search artifact's entry for the same ID, the existing vector is reused.
- `agent_artifact_hash` is `sha256` of the serialized `docs.agent.json` produced in the same build. Mismatch between artifacts at retrieval time produces a `search.hash_drift` warning.
- `model` mismatch between the search artifact and the active provider produces `search.model_mismatch` (error) and the search artifact is treated as unloadable.
- Schema version mismatch produces `schema.unsupported_version`.

## Embedding Composition

Each Knowledge Object is reduced to one canonical input string before embedding:

```text
{kind}: {body_plain_text}
[id: {id}] [status: {status_or_unknown}] [owner: {owner_or_unknown}]
```

Rules:

- `body_plain_text` is the inline-aware body projected to plain text via the existing `domain::inline::plain_text` projection. Object reference markers (`[[object.id]]`) are kept as literal text so semantic matches surface objects that mention other objects by ID.
- The bracketed metadata trail is appended exactly once. Missing fields render as `unknown`.
- Whitespace is trimmed; line endings normalize to `\n`.
- Relations are not concatenated into the embedding input in V1. They are filter targets, not semantic signal. Revisit during pilot evaluation.
- The composition formula is part of the contract. Changing it requires bumping `adoc.search.v0` to `adoc.search.v1` and invalidating prior search artifacts.

## EmbeddingProvider Port

```rust
// domain/ports/embedding_provider.rs

pub(crate) trait EmbeddingProvider {
    fn model_id(&self) -> &ModelId;
    fn dim(&self) -> usize;
    fn embed_passages(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError>;
    fn embed_query(&self, query: &str) -> Result<Vec<f32>, EmbeddingError>;
}

pub(crate) struct ModelId {
    pub id: String,                   // "bge-small-en-v1.5"
    pub provider: String,             // "fastembed"
}

pub(crate) enum EmbeddingError {
    ModelLoad(String),
    Compute(String),
    DimensionMismatch { expected: usize, actual: usize },
}
```

Rules:

- The port stays `pub(crate)` per ADR-0006. Promoted to `pub` only when an external consumer (LSP, MCP server, alternate CLI) exists.
- `embed_passages` and `embed_query` are split because most modern embedding models distinguish passage and query encoding (asymmetric retrieval). For models that do not, a single implementation can dispatch both.
- The default adapter is `FastEmbedProvider` wrapping `fastembed-rs` with `bge-small-en-v1.5`. First run downloads weights via the crate's built-in HF fetcher; subsequent runs read the cached file.
- `InMemoryProvider` produces deterministic pseudo-embeddings via a small hashing scheme. It is the only provider used in unit and integration tests so the test suite remains hermetic and the bge weights are not pulled in CI.
- Adding a hosted provider later means adding one file in `infrastructure/embedding/` and one CLI flag (`--embedding-provider hosted`); the port does not change.

## Retrieval Pipeline

```text
adoc.agent.json -+
                 +--> ArtifactReader --> RetrievalSession {
                 |        exact_lookup,                      (id -> object)
                 |        LexicalIndex,                      (BM25, rebuilt on load)
                 |        VectorIndex (when search.json ok)  (cosine, brute-force)
                 +-> [filters]
                 |
adoc.search.json +
                              |
                              v
                       HybridRanker (RRF, k=60)
                              |
                              v
                       Vec<RetrievalRecord>
```

Rules:

- BM25 corpus is rebuilt from object metadata on each `load_retrieval_session` call. The corpus is small (target pilot ~30 objects, plausible scale to ~10k); load-time rebuild keeps the search artifact small and avoids a lexical schema versioning headache.
- BM25 fields: `body`, `id`, `kind`, `owner`. Field weights are uniform in V1.
- Vector retrieval is brute-force cosine. At ~10k objects the per-query cost is well under 50ms on a single core.
- `HybridRanker` combines BM25 ranks and vector ranks via Reciprocal Rank Fusion: `score = Σ 1 / (k + rank_i)` with `k = 60`. The fused list is truncated to `top`.
- Exact Object ID match always pins to position 1, regardless of fusion score. ID prefix matches (caller typed `billing.credits` and `billing.credits` is a real ID prefix of multiple objects) pin all matching objects above the fused list, ordered by length asc, then lex asc.
- Filters apply before ranking for `--lexical` and `--semantic`, and post-rank for `--hybrid` (so RRF can compare a comparable candidate pool from both indexes). All filters are lowercase substring matches on the relevant field.
- Tie-breaker on ranks and scores: ascending Object ID lex order. Result ordering is fully deterministic.

## Retrieval Record

```json
{
  "id": "billing.credits.decrement-after-success",
  "kind": "claim",
  "status": "verified",
  "owner": "backend-platform",
  "body": "Credits are decremented only after generation completes successfully.",
  "source": {
    "path": "examples/billing-pilot/02-claims.adoc",
    "line": 12,
    "column": 1
  },
  "evidence": {
    "source": "apps/backend/src/features/credits/consume.use-case.ts",
    "test": "apps/backend/src/features/credits/consume.test.ts",
    "reviewed_by": "backend-lead"
  },
  "relations": {
    "depends_on": ["billing.credits.ledger"],
    "supersedes": [],
    "related_to": []
  },
  "match": {
    "mode": "hybrid",
    "rrf_score": 0.0312,
    "result_rank": 1,
    "lexical_rank": 2,
    "vector_rank": 1
  }
}
```

Rules:

- Records are a projection of `AgentJsonObject` plus a `match` block. The projection lives in `domain/retrieval/retrieval_record.rs` so HTML and agent JSON renderers stay format-specific.
- The `match` block is the only field that varies between `explain` (no match block) and `search` outputs.
- Per-object fields that the agent artifact omits stay omitted in the record. No null padding, no empty strings.

## New Diagnostic Codes

```rust
pub enum DiagnosticCode {
    // ... existing V0 codes unchanged ...

    // Build-time embedding diagnostics.
    EmbedModelLoadFailed,         // embed.model_load_failed
    EmbedComputeFailed,           // embed.compute_failed
    EmbedUnexpectedDimension,     // embed.unexpected_dim

    // Read-time artifact diagnostics.
    IoArtifactMissing,            // io.artifact_missing
    IoArtifactUnreadable,         // io.artifact_unreadable
    IoArtifactMalformed,          // io.artifact_malformed
    SchemaUnsupportedVersion,     // schema.unsupported_version
    IdDuplicateInArtifact,        // id.duplicate_in_artifact

    // Retrieval-time diagnostics.
    RetrievalObjectNotFound,      // retrieval.object_not_found
    SearchArtifactMissing,        // search.artifact_missing (warn)
    SearchModelMismatch,          // search.model_mismatch (error)
    SearchHashDrift,              // search.hash_drift (warn)

    // Build-time info diagnostic.
    BuildEmbeddingsSkipped,       // build.embeddings_skipped (info)
}
```

All diagnostic codes follow ADR-0007: emission sites take typed values, not free-form strings, and the registry remains stable across V1 patch releases.

## Tracer-Bullet Slices

V1 ships in six vertical slices. Each slice ends with runnable CLI behavior, fixtures, golden artifacts, and documentation, per the roadmap rules.

### V1.1 — `adoc explain <object-id>`

Goal: make Object IDs immediately useful for humans and for any agent that has already learned IDs from the agent artifact.

Scope:

- Add `domain/retrieval/retrieval_record.rs` and `application/retrieval.rs` skeleton.
- Add `infrastructure/artifact/agent_json.rs` reader (currently the file only contains a writer).
- Add a `RetrievalSession`-owned exact lookup keyed by Object ID with duplicate detection inside the artifact.
- Add diagnostics: `io.artifact_missing`, `io.artifact_unreadable`, `io.artifact_malformed`, `schema.unsupported_version`, `id.duplicate_in_artifact`, `retrieval.object_not_found`.
- Implement `adoc explain <id>` with `--artifact <path>` and `--format text|json`.
- Pretty text output mirrors PRD §21.5: kind, status, owner, verified date, body, evidence, source, relations.
- Integration tests against a fixture agent artifact; CLI tests for the four error paths.

Acceptance:

- `adoc explain billing.credits.decrement-after-success --artifact examples/billing-pilot/dist/docs.agent.json` prints the object.
- `--format json` emits an `adoc.retrieval.v0` envelope with one record.
- Unknown ID exits `3` with a fix-oriented message that does not implicate the source.
- Missing or malformed artifact exits `2` with guidance to run `adoc build`.

Deferred to later slices: search, embeddings, hybrid ranking, pilot evaluation harness.

### V1.2 — `adoc search` (lexical-only)

Goal: ship structured search before introducing the embedding pipeline.

Scope:

- Add `LexicalIndex` (BM25, rebuilt on session load) over `body`, `id`, `kind`, `owner`.
- Add `Filters` value object covering `--kind`, `--status`, `--owner`, `--source-path`.
- Implement `adoc search "<query>"` with all CLI flags except `--semantic` and `--search-artifact`.
- ID-prefix exact match pinned to top.
- Stable lex tie-breaker.
- Empty result is `0` exit with `(no matches)` line.
- `--format json` envelope identical to V1.1's, plus `match.mode = "lexical"`, `match.result_rank`, and `match.lexical_rank` when the record has a BM25 hit.
- Integration tests cover ranking determinism, every filter, exact ID pin, empty results, and invalid filters.

Acceptance:

- Lexical queries against the billing pilot return at least the obvious matches in the top three for every benchmark query authored alongside this slice.
- All filter combinations resolve correctly; unknown filter values produce a fix-oriented error and exit `1`.

Deferred: embeddings, hybrid ranking, search-artifact diagnostics.

### V1.3 — Embedding Build Pipeline

Goal: make `adoc build` produce a deterministic, content-hashed search artifact.

Scope:

- Add `EmbeddingProvider` port in `domain/ports/`.
- Add `FastEmbedProvider` adapter wrapping `fastembed-rs` with `bge-small-en-v1.5`. First-run weight download cached under the platform user data dir; subsequent runs are offline.
- Add `InMemoryProvider` adapter that deterministically maps inputs to fixed-dim vectors via a hashing scheme. Used by every test that does not specifically exercise the fastembed adapter.
- Extend `compile_with_provider` with an `EmbeddingProvider` argument plumbed via the application layer; `compile_workspace()` defaults to `FastEmbedProvider`.
- Add `infrastructure/artifact/search_json.rs` writer/reader.
- Add per-Object-ID embedding cache: when an existing `dist/docs.search.json` matches the current model and a content hash agrees, the prior vector is reused.
- Add `--no-embeddings` to `adoc build`. Add diagnostics: `embed.model_load_failed`, `embed.compute_failed`, `embed.unexpected_dim`, `build.embeddings_skipped`.
- Golden tests over the InMemoryProvider so the search artifact has a stable shape under CI.
- A separate integration test exercises the FastEmbedProvider path and is gated behind a feature flag (`cargo test --features fastembed-it`) so default `cargo test` stays hermetic.

Acceptance:

- `adoc build examples/billing-pilot --out examples/billing-pilot/dist` produces all three artifacts.
- A second `adoc build` run with no source changes reuses every prior vector (verifiable via build log or a `build.embeddings_cached` count line).
- `--no-embeddings` skips writing the search artifact, leaves prior search artifact untouched, and emits `build.embeddings_skipped`.
- Model load failures fall through with a fix-oriented diagnostic and a non-zero exit.

Deferred: semantic queries, hybrid ranking, pilot evaluation.

### V1.4 — `adoc search --semantic`

Goal: surface semantic recall behind an explicit flag before changing default ranking behavior.

Scope:

- Add `VectorIndex` (cosine, brute-force, brute-force is fine at pilot scale).
- Add search-artifact loading at session start; missing artifact emits `search.artifact_missing` (warn) and disables semantic mode.
- Add `search.model_mismatch` (error) and `search.hash_drift` (warn).
- Implement `--semantic` flag on `adoc search`. JSON output adds `match.vector_rank` and a `match.cosine_score`.
- Integration tests cover paraphrase recall (pilot queries that lexical loses), model mismatch error, and hash-drift warning produced when search artifact is older than agent artifact.

Acceptance:

- A paraphrase query that fails under `--lexical` succeeds in the top three under `--semantic` for at least three pilot examples.
- A search artifact built with a different model is rejected with `search.model_mismatch` and exit `2`.

Deferred: making hybrid the default; pilot evaluation harness.

### V1.5 — Hybrid Default

Goal: make `adoc search` hybrid by default, with the lexical and semantic flags as escape hatches.

Scope:

- Add `HybridRanker` (RRF, `k = 60`).
- Default mode for `adoc search` becomes hybrid when both indexes are available; degrades to lexical-only with `search.artifact_missing` warning when the search artifact is absent.
- ID-prefix pin moves out of `LexicalIndex` into `HybridRanker` so it applies in every mode.
- Filters apply post-rank in hybrid mode; pre-rank in lexical and semantic modes.
- Integration tests cover RRF determinism, ID-prefix pin in every mode, empty intersection of two non-empty result lists, and filter behavior in each mode.

Acceptance:

- The benchmark queries from V1.2 and V1.4 still pass; new hybrid-only benchmark queries (where neither lexical nor semantic alone suffices) exist and pass.
- Removing `dist/docs.search.json` reduces search to lexical with one warning, with no other behavior change.

Deferred: pilot evaluation harness, multi-factor scoring.

### V1.6 — Pilot Evaluation Harness

Goal: prove V1 retrieval is "good" and stays good.

Scope:

- Grow `examples/billing-pilot` to at least 30 Knowledge Objects across all four V0 kinds, with a meaningful share of verified claims.
- Add `examples/billing-pilot/retrieval-set.yaml`:
  - 15-20 manually authored queries with `expected_ids` and `must_appear_in_top` thresholds.
  - Coverage: paraphrase, exact ID, owner, kind filter, evidence path, broken filter, empty.
- Add a property-based test suite over the artifact: every object's verbatim body must appear in `--lexical` top 1; every Object ID must appear in `--lexical` top 1 for that ID; every owner query surfaces every claim with that owner.
- Both suites run in CI on the pilot.
- Document the workflow in `docs/v1-retrieval.md`: build, explain, search, citation pattern, hybrid versus lexical versus semantic, model swap consequences.
- Document the embedding cache semantics and the fastembed integration path for power users.

Acceptance:

- The retrieval-set integration test passes deterministically against `InMemoryProvider`.
- The same test passes against `FastEmbedProvider` under the gated CI run.
- The property suite passes against both providers.
- Every later change to ranking, embedding composition, or model selection must keep both suites green or update them with a recorded rationale.

Deferred from V1 entirely: agent-server surface (V1.5/V2), `adoc init` and config (V1.5), stale-by-expiration diagnostics (V1.5), source-aware retrieval (V2+), graph artifact (V6), permissioned retrieval (V7).

## Tests and Fixtures

Fixture additions:

```text
fixtures/
  v1_1_explain/
    valid_artifact.agent.json
    missing_object.agent.json
    malformed_artifact.agent.json
    unsupported_version.agent.json
    duplicate_id.agent.json
  v1_2_search/
    pilot_subset.agent.json
    empty.agent.json
  v1_3_embed/
    in_memory_baseline.search.json
  v1_4_semantic/
    model_mismatch.search.json
    hash_drift.agent.json
    hash_drift.search.json
```

Test guidance:

- Hermetic by default: every unit and integration test uses `InMemoryProvider`. The fastembed path runs only under `cargo test --features fastembed-it`.
- CLI tests that need the production `build_workspace()` boundary without model downloads enable the test-only `test-embedding-provider` feature and set `ADOC_TEST_EMBEDDING_PROVIDER=in-memory`. Production builds do not use this seam.
- Golden-test the JSON envelope produced by `--format json` for both `explain` and `search`. Schema regressions must update the golden file plus the schema version explicitly.
- Property suite generated from any agent artifact: every body verbatim → top 1 lexical, every Object ID → top 1 lexical, every owner query covers every claim with that owner.

## Open Questions Before Scaffolding

None. The next step is scaffolding `domain/retrieval/`, the `EmbeddingProvider` port, and the `InMemoryProvider` adapter.

## Related ADRs

- ADR-0006 (internal hexagonal ports) governs the new `EmbeddingProvider` port.
- ADR-0007 (validation as a separate pass) governs the new diagnostic codes.
- ADR-0009 (tactical DDD layout) governs the placement of `domain/retrieval/` and `infrastructure/embedding/`.
- ADR-0010 (V1 retrieval architecture) records the V1 architecture choices summarized here.
