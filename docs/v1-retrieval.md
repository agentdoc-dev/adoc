# V1 Retrieval

V1 retrieval is a local read-side workflow over build artifacts. `adoc build`
creates the human HTML, the graph artifact, and the optional search artifact.
`adoc why`, `adoc graph`, and `adoc search` read those artifacts; they do not
compile source files.

## Build

```bash
adoc build examples/billing-pilot --out dist
```

Successful embedding-enabled builds write:

- `dist/docs.html`
- `dist/docs.graph.json`
- `dist/docs.search.json`

`docs.graph.json` is the canonical retrieval record and relation graph source.
`docs.search.json` is the sidecar vector index. By default search uses the local FastEmbed
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
`docs.graph.json`, and `docs.search.json` unless exact output paths override
them. Exact config outputs need `html` and `graph`; `search` is required only
when embeddings are enabled.

Config embedding mode is:

```yaml
embeddings:
  provider: local # or none
```

Missing `embeddings` defaults to `local`. `none` is equivalent to skipping
embedding generation for config-backed builds. Hosted embedding adapters remain
deferred; the shipped provider is local.

Use `--no-embeddings` when you only need HTML and graph JSON:

```bash
adoc build examples/billing-pilot --out dist --no-embeddings
```

That skips model loading and leaves any prior `docs.search.json` untouched.
Config-backed skipped-embedding builds can omit `outputs.search` when exact
HTML and graph JSON paths are configured.

If a Knowledge Object has a parseable `expires_at` date before the local build
date, `check` and `build` emit warning `lifecycle.expired`. The warning does
not block artifacts and does not mutate source.

## Why

```bash
adoc why billing.credits.decrement-after-success --artifact dist/docs.graph.json
adoc why billing.credits.decrement-after-success --artifact dist/docs.graph.json --format json
```

Use `why` when you already have an Object ID and need the authoritative
record: kind, status, owner, evidence, source span, body, and relations.

## Graph

```bash
adoc graph billing.credits.decrement-after-success \
  --artifact dist/docs.graph.json
```

Use `graph` when you need relation traversal from one Object ID. The command
loads compiled artifacts only. It includes the root node at distance `0`,
traverses the full reachable graph by default, and marks revisit edges instead
of recursively revisiting nodes.

Traversal flags:

- `--direction outgoing|incoming|both`; default is `both`.
- `--relation depends_on|supersedes|related_to`; default is all three current
  relation kinds.
- `--format json` emits `adoc.graph.traversal.v0`.

## Search

```bash
adoc search "when are credits decremented" \
  --artifact dist/docs.graph.json \
  --search-artifact dist/docs.search.json
```

Default search is hybrid when both artifacts are present. Hybrid uses
Reciprocal Rank Fusion over lexical BM25 and vector cosine ranks. Exact Object
ID and ID-prefix queries are pinned in every mode.

Mode flags:

- Default: hybrid when `docs.search.json` loads; lexical fallback with one
  `search.artifact_missing` warning when it is absent.
- `--lexical`: deterministic text and Object ID search over `docs.graph.json`.
  Use it for exact IDs, filters, regression tests, and offline operation.
- `--semantic`: vector-only ranking from `docs.search.json`. Use it to inspect
  paraphrase recall or isolate embedding behavior.

Filters:

```bash
adoc search "credit" --kind claim --status verified --owner team-billing
adoc search "" --lexical --owner team-billing --top 20
adoc search "credit" --related-to billing.credits --relation depends_on
```

`--kind`, `--status`, `--owner`, and `--source-path` filter results. Empty
lexical query text lists the filtered candidate set in deterministic Object ID
order, which is useful for ownership audits.

`--related-to` enables graph retrieval. It loads `docs.graph.json`, computes
the reachable candidate set from the requested Object ID, then lets the normal
lexical, semantic, or hybrid ranking run inside that candidate set. `--relation`
and `--direction` narrow traversal. Graph flags are opt-in; absent
`--related-to`, search ranking is unchanged and there is no graph proximity
boost.

JSON output:

```bash
adoc search "refund audit" --format json
```

Search and why both emit `adoc.retrieval.v1`:

```json
{
  "schema_version": "adoc.retrieval.v1",
  "records": [],
  "diagnostics": []
}
```

Every record carries `record_type: "knowledge_object" | "prose"` (V1.7.1,
ADR-0040). `adoc search` blends both types in one RRF-ranked list —
`--objects-only` and `--prose-only` restrict it, and any Knowledge Object
metadata filter implies `--objects-only`; `adoc why` returns Knowledge Object
records only. Each search record includes a `match` block with `mode`,
`result_rank`, and mode-specific rank metadata. Hybrid records include
`rrf_score` and may include `lexical_rank` and `vector_rank`. Semantic records
include `vector_rank` and `cosine_score`; since V1.7.2 (`adoc.search.v1`)
prose blocks carry vectors too, so semantic and hybrid ranking cover both
record types, and `--prose-only --semantic` is a valid combination.

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
search, why, source spans, and relations.

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
Prose entries (V1.7.2, ADR-0040) reuse by content hash and model alone, never
by block id — order-derived `#block-NNNN` ids renumber when a block is
inserted mid-page, and hash-keyed reuse makes renumbering free. Build output
reports:

```text
info[build.embeddings_cached] embeddings: cached N, computed M
```

No action is required. It is a visibility diagnostic for cache reuse.

### Migrating a pre-V1.7.2 search artifact

V1.7.2 bumped the search artifact schema from `adoc.search.v0` to
`adoc.search.v1` (prose entries, `entry_kind` discriminator). The version
check is exact-match, so an existing `docs.search.json` from V1.7.1 or
earlier behaves as follows after upgrading:

- `adoc build` warns that the prior cache is ignored and recomputes all
  vectors once; warm rebuilds are cached again afterwards.
- `adoc search --semantic` (and hybrid) reject the stale artifact with
  `schema.unsupported_version`; the diagnostic's help points at the
  fix — rebuild with `adoc build`.

### V1.7.2 build-time and size measurements

Recorded on the pilots when prose entries landed in `adoc.search.v1`
(post-`adoc.graph.v4`, per the ROADMAP-V7 sequencing note), cold builds into a
fresh output directory, debug binary, Apple Silicon. "Before" is `origin/main`
with V1.7.1 (Knowledge-Object-only artifact); "after" is V1.7.2. The Markdown
Pilot's config pins the deterministic provider; the Expanded Pilot uses local
fastembed (`bge-small-en-v1.5`, cached model).

| Pilot | Provider | Build before | Build after | Artifact before | Artifact after | Entries before (KO) | Entries after (KO + prose) |
| --- | --- | --- | --- | --- | --- | --- | --- |
| markdown-pilot | deterministic | 1.8 s | 2.1 s | 49 KiB | 664 KiB | 6 | 6 + 75 |
| expanded-pilot | fastembed | 0.5 s | 2.0 s | 221 KiB | 377 KiB | 27 | 27 + 19 |

Warm rebuilds with an unchanged tree fully reuse the cache on both pilots
(`cached 81, computed 0` / `cached 46, computed 0`, ~0.25 s). The size growth
is proportional to indexed prose volume (vectors dominate; 384 dims per
entry); code blocks and sub-threshold blocks are never embedded. Chunking and
ANN indexes remain the named next steps if real corpora push these numbers up
(measured, per the V1 rule).

## Retrieval Set Updates

Two golden sets exist (V1.7.3): the Knowledge-Object-heavy billing set at
`examples/billing-pilot/retrieval-set.yaml` and the mixed prose-plus-KO set at
`examples/markdown-pilot/retrieval-set.yaml`.

Add or change entries when ranking behavior, embedding composition, corpus
content, or model selection changes. Each entry should include:

- `query`
- `mode`: `hybrid`, `lexical`, or `semantic`
- `scope`: `objects_only` (default — reproduces the pre-V1.7 Knowledge-Object
  sequences), `blended`, or `prose_only` (V1.7.3)
- `top`: the requested result budget, when it must exceed
  `must_appear_in_top` — hybrid RRF fuses the per-mode top-k lists, so rank
  assertions against a one-element pool prove nothing; blended cases use
  `top: 10`
- `expected_ids`
- `must_appear_in_top`
- filters when the case exists to cover a filter path
- `expected_diagnostics` when the corpus compiles with warnings — the
  harness asserts the envelope's distinct code set equals the declared set
  exactly (the markdown pilot's compat budget rides every envelope)

Keep the billing set at 15-25 and the markdown set at 8-20 high-signal
queries. Cover paraphrase behavior, exact ID pins, ID prefixes, owner filters,
kind filters, evidence-field queries, status filters, empty results, broken
filters, and (V1.7.3) blend honesty: KO-first queries where a citable object
must beat prose competition, and legitimately prose-first queries. When
changing expected IDs because the intended behavior changed, add a short YAML
comment next to that case with the rationale.

Two conventions keep blended cases honest and hermetic:

- Rank-1 blend assertions use `mode: lexical` — BM25 rank is deterministic
  and identical across embedding backends. Hybrid cases pin only what fusion
  guarantees regardless of the model: exact Object ID pins rank first, and
  strong matches stay inside a `must_appear_in_top: 5` window of a `top: 10`
  pool.
- The `.adoc`/`.md` symmetry property (identical prose ranks identically;
  only `source.path` differs) is pinned twice: at the session level in
  `crates/adoc-core/tests/retrieval.rs` across all four prose block kinds,
  and end-to-end (compile → build → search) in
  `crates/adoc-cli/tests/retrieval_pilot.rs`.

Run the hermetic suite before committing:

```bash
cargo test -p adoc-cli --test retrieval_pilot --locked
```

The gated production-model suite runs with:

```bash
cargo test -p adoc-cli --test retrieval_pilot --features fastembed-it --locked
```

## Migration Hint (downgraded at V1.7.3)

`retrieval.no_knowledge_objects_consider_migration` still fires when a search
returns zero records against a project that has Markdown prose but no
Knowledge Objects. V1.7.3 downgraded its framing, not its trigger: prose
retrieval works for `.md`-only projects since V1.7.1, so the hint no longer
describes a dead end ("wait for `adoc migrate`") — it now says prose is
searchable as-is and migration is what makes findings citable. The code,
WARNING severity, and trigger are unchanged; V8.1.1 renames the "future
`adoc migrate`" phrasing to the shipped command.
