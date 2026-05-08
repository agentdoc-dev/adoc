# V1 Retrieval

V1 retrieval is a local read-side workflow over build artifacts. `adoc build`
creates the human HTML, the agent artifact, and the optional search artifact.
`adoc explain` and `adoc search` read those artifacts; they do not compile
source files.

## Build

```bash
adoc build examples/billing-pilot --out dist
```

Successful embedding-enabled builds write:

- `dist/docs.html`
- `dist/docs.agent.json`
- `dist/docs.search.json`

`docs.agent.json` is the canonical retrieval record source. `docs.search.json`
is the sidecar vector index. By default it uses the local FastEmbed
`bge-small-en-v1.5` provider. The first build may download model weights through
`fastembed-rs`; later builds reuse the local model cache.

With `agentdoc.config.yaml`, the same workflow can omit paths:

```bash
adoc init
adoc check
adoc build
```

`check` and `build` use config `docs_path` when no source path is passed.
`build` uses config outputs when `--out` is omitted. Config paths are resolved
relative to the config file; `outputs.dir` fills `docs.html`,
`docs.agent.json`, and `docs.search.json` unless exact output paths override
them.

Config embedding mode is:

```yaml
embeddings:
  provider: local # or none
```

Missing `embeddings` defaults to `local`. `none` is equivalent to skipping
embedding generation for config-backed builds. Hosted embedding adapters remain
deferred; the shipped provider is local.

Use `--no-embeddings` when you only need HTML and agent JSON:

```bash
adoc build examples/billing-pilot --out dist --no-embeddings
```

That skips model loading and leaves any prior `docs.search.json` untouched.

If a Knowledge Object has a parseable `expires_at` date before the local build
date, `check` and `build` emit warning `lifecycle.expired`. The warning does
not block artifacts and does not mutate source.

## Explain

```bash
adoc explain billing.credits.decrement-after-success --artifact dist/docs.agent.json
adoc explain billing.credits.decrement-after-success --artifact dist/docs.agent.json --format json
```

Use `explain` when you already have an Object ID and need the authoritative
record: kind, status, owner, evidence, source span, body, and relations.

## Search

```bash
adoc search "when are credits decremented" \
  --artifact dist/docs.agent.json \
  --search-artifact dist/docs.search.json
```

Default search is hybrid when both artifacts are present. Hybrid uses
Reciprocal Rank Fusion over lexical BM25 and vector cosine ranks. Exact Object
ID and ID-prefix queries are pinned in every mode.

Mode flags:

- Default: hybrid when `docs.search.json` loads; lexical fallback with one
  `search.artifact_missing` warning when it is absent.
- `--lexical`: deterministic text and Object ID search over `docs.agent.json`.
  Use it for exact IDs, filters, regression tests, and offline operation.
- `--semantic`: vector-only ranking from `docs.search.json`. Use it to inspect
  paraphrase recall or isolate embedding behavior.

Filters:

```bash
adoc search "credit" --kind claim --status verified --owner team-billing
adoc search "" --lexical --owner team-billing --top 20
```

`--kind`, `--status`, `--owner`, and `--source-path` filter results. Empty
lexical query text lists the filtered candidate set in deterministic Object ID
order, which is useful for ownership audits.

JSON output:

```bash
adoc search "refund audit" --format json
```

Search and explain both emit `adoc.retrieval.v0`:

```json
{
  "schema_version": "adoc.retrieval.v0",
  "records": [],
  "diagnostics": []
}
```

Each search record includes a `match` block with `mode`, `result_rank`, and
mode-specific rank metadata. Hybrid records include `rrf_score` and may include
`lexical_rank` and `vector_rank`. Semantic records include `vector_rank` and
`cosine_score`.

## Citation Pattern

When citing a retrieved record, cite the Object ID first. Include kind, status,
owner, and evidence when present:

```text
billing.credits.decrement-after-success (claim, verified, owner team-billing)
Evidence: source=billing service credit application trace 2026-05-05;
test=cargo test billing_credit_decrement_after_success;
reviewed_by=qa-billing.
```

Prefer the Object ID over prose titles. It is the stable handle that connects
search, explain, source spans, and relations.

## Model Swaps

`docs.search.json` records the embedding provider, model ID, and vector
dimension. If the active provider does not match the artifact, retrieval emits
`search.model_mismatch` and disables semantic/hybrid vector use. Rebuild the
project with the active provider to regenerate embeddings.

Changing embedding input composition or model identity invalidates old search
artifacts. Treat that as a retrieval contract change: update the tests, update
the retrieval set rationale, and rebuild the artifact.

## Embedding Cache

Each search entry has a content hash. During `adoc build`, unchanged Object IDs
reuse prior vectors from the previous output directory's `docs.search.json`.
Build output reports:

```text
info[build.embeddings_cached] embeddings: cached N, computed M
```

No action is required. It is a visibility diagnostic for cache reuse.

## Retrieval Set Updates

The billing pilot golden set lives at
`examples/billing-pilot/retrieval-set.yaml`.

Add or change entries when ranking behavior, embedding composition, corpus
content, or model selection changes. Each entry should include:

- `query`
- `mode`: `hybrid`, `lexical`, or `semantic`
- `expected_ids`
- `must_appear_in_top`
- filters when the case exists to cover a filter path

Keep the set at 15-20 high-signal queries. Cover paraphrase behavior, exact ID
pins, ID prefixes, owner filters, kind filters, evidence-field queries, status
filters, empty results, and broken filters. When changing expected IDs because
the intended behavior changed, add a short YAML comment next to that case with
the rationale.

Run the hermetic suite before committing:

```bash
cargo test -p adoc-cli --test retrieval_pilot --locked
```

The gated production-model suite runs with:

```bash
cargo test -p adoc-cli --test retrieval_pilot --features fastembed-it --locked
```
