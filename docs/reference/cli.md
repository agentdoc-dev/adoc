# CLI reference

```bash
adoc init
adoc migrate [path] [--write] [--force] [--export]
adoc check [path] [--as-of <YYYY-MM-DD>]
adoc build [path] [--out <directory>] [--no-embeddings] [--as-of <YYYY-MM-DD>] [--audience <public|internal|restricted>]
adoc why <object-id> [--artifact <path>] [--format auto|plain|styled|json]
adoc graph <object-id> [--artifact <path>] [--relation depends_on|supersedes|related_to] [--direction outgoing|incoming|both] [--format auto|plain|styled|json]
adoc stale [--artifact <path>] [--within <Nd>] [--format auto|plain|styled|json]
adoc contradictions [--artifact <path>] [--all] [--format auto|plain|styled|json]
adoc impacted-by [path]... [--ref <git-ref>] [--artifact <path>] [--format auto|plain|styled|json|markdown]
adoc patch (--check <patch-json> | --apply <patch-json|@->) [--artifact <path>] [--as-of <YYYY-MM-DD>] [--format auto|plain|styled|json]
adoc diff <base-ref> [--format auto|plain|styled|json|markdown]
adoc review <base-ref> [--patch <patch-json>] [--format auto|plain|styled|json|markdown]
adoc assess-changes --base <git-ref> [--head <git-ref>] [--as-of <YYYY-MM-DD>] [--format auto|plain|styled|json|markdown]
adoc baseline --ref <git-ref> [--as-of <YYYY-MM-DD>] [--format auto|plain|styled|json|markdown]
adoc search <query> [--artifact <path>] [--search-artifact <path>] [--lexical | --semantic] [--kind <value>] [--status <value>] [--owner <value>] [--source-path <value>] [--related-to <object-id>] [--relation depends_on|supersedes|related_to] [--direction outgoing|incoming|both] [--top <n>] [--format auto|plain|styled|json]
```

`<path>` can be:

- a single `.adoc` file
- a directory, scanned recursively for `.adoc` files

Config discovery walks upward from the current directory, checks for
`agentdoc.config.yaml` in each directory, and stops after checking the first
ancestor containing `.git` or `$HOME`. It never treats `/agentdoc.config.yaml`
as global config.

`adoc init`:

- creates `agentdoc.config.yaml` and `docs/index.adoc` in the current directory
- refuses to overwrite either target if it already exists
- configures strict mode, `docs_path: docs`, `outputs.dir: dist`, and `embeddings.provider: local`

`adoc migrate`:

- converts Markdown to prose-mode `.adoc`; the default is a dry run that writes nothing
- accepts a source file or directory; without a path, uses project configuration
- `--write` creates each target `.adoc` and removes its source `.md` to avoid duplicate page IDs
- refuses a write when any source is not committed and clean in Git; `--force` overrides this recovery safeguard
- `--export` reverses the conversion, with the same dry-run and write semantics; typed blocks refuse the entire export with `migrate.export_typed_blocks_present` to prevent lossy conversion
- exits `0` for a clean dry run or conversion, `1` for refusal or migration errors, and `2` for usage errors

Start with `adoc migrate docs`, inspect the diagnostics, and commit your sources before using `adoc migrate docs --write`. Use `adoc migrate docs --export` to preview a return to Markdown.

`adoc check`:

- uses explicit `[path]` when passed
- otherwise discovers the nearest `agentdoc.config.yaml` from the current directory upward and uses `docs_path`
- compiles the input in strict mode
- prints diagnostics and a summary
- exits `0` when there are no errors
- exits `1` when any error diagnostic exists

`adoc build`:

- uses explicit `[path]` and `--out` when passed
- otherwise discovers config defaults; without `--out`, config must provide `outputs.dir` or exact `outputs.html` and `outputs.graph`; `outputs.search` is also required when embeddings are enabled
- with `--out <directory>`, writes `<directory>/docs.html`, `<directory>/docs.graph.json`, and, when embeddings are enabled, `<directory>/docs.search.json`
- with config outputs, paths are resolved relative to the config file; `outputs.dir` fills omitted artifact paths as `docs.html`, `docs.graph.json`, and `docs.search.json`; exact artifact paths override the `outputs.dir` defaults
- runs the same compile path as `check`
- creates the output directory when it does not exist
- fails if the output path exists as a file
- writes `docs.html` and `docs.graph.json` when source compilation is clean
- loads the local FastEmbed `bge-small-en-v1.5` model by default through the default-on `embeddings` feature; first run may download model weights into the platform cache
- uses the deterministic hash-based provider instead when config sets `embeddings.provider: deterministic`
- reads the prior output directory's `docs.search.json` when present and reuses vectors whose model header and content hash still match, reported as `info[build.embeddings_cached] embeddings: cached N, computed M`
- if embedding model load, compute, or dimension validation fails after clean source compilation, exits `1`, still writes `docs.html` and `docs.graph.json`, omits a new `docs.search.json`, and leaves any prior `docs.search.json` untouched
- accepts `--no-embeddings` to skip model loading and search artifact writes; any existing `docs.search.json` is left untouched and an info diagnostic `build.embeddings_skipped` is emitted
- also skips embeddings when config sets `embeddings.provider: none`; config `local` and missing `embeddings` both enable the shipped local provider

HTML rendering defaults to public-only. `--audience` selects an explicit local
audience; the project's `retrieval_policy` supplies the default audience and
retains its allowed-visibility restrictions and Object ID exclusions when a flag
is supplied. Unknown audiences produce `retrieval.audience_unresolved`.
Restricted objects render as kind-and-ID markers (`adoc-restricted`), sensitive fields are withheld,
and existence-excluded objects are omitted. An MCP gateway's trusted policy takes
precedence over project configuration. No audience is read from the environment.

This rendering policy applies to `docs.html`. The canonical Graph Artifact is
unchanged; search/vector exclusion has separate policy boundaries. Neither
artifact should be treated as an audience-filtered HTML export.

`adoc why`:

- reads a compiled graph artifact; it does not compile source
- defaults to config `outputs.graph`, then `dist/docs.graph.json`
- prints the matching Knowledge Object with source and relation metadata
- supports `--format auto|plain|styled|json`

`adoc graph`:

- reads a compiled graph artifact; it does not compile source
- defaults to config `outputs.graph`, then `dist/docs.graph.json`
- traverses all reachable Knowledge Objects by default, with cycle detection
- includes the root node at distance `0` and preserves original edge direction in output
- supports `--relation depends_on|supersedes|related_to` and `--direction outgoing|incoming|both`
- supports `--format auto|plain|styled|json`

`adoc stale`:

- reads a compiled graph artifact; it does not compile source
- lists stale, review-overdue, and expiring Knowledge Objects, re-deriving lifecycle signals as of the query date
- accepts `--within <Nd>` to widen the expiring-soon horizon
- exits `0` whether or not records exist and emits the `adoc.stale.v0` envelope

`adoc contradictions`:

- reads a compiled graph artifact; it does not compile source
- lists unresolved contradictions and contradicted claims; `--all` widens the contradictions listing to resolved ones
- exits `0` whether or not records exist and emits the `adoc.contradictions.v0` envelope

`adoc impacted-by`:

- reads a compiled graph artifact; it does not compile source
- lists verified Knowledge Objects implicated by changed source paths, passed explicitly or derived from `--ref <git-ref>`
- emits the `adoc.impacted.v0` envelope and supports `--format markdown` for PR-comment output

`adoc patch`:

- validates one `adoc.patch.v0` document against the compiled graph artifact's `content_hash` preconditions
- `--check <patch-json>` is read-only and emits the `adoc.patch.check.v0` envelope
- `--apply <patch-json>` (or `@-` to read from stdin) validates, then rewrites the affected source spans and emits the `adoc.patch.apply.v0` envelope

`adoc diff`:

- diffs Knowledge Objects between `<base-ref>` and the working tree, emitting the `adoc.diff.v0` envelope
- supports `--format markdown` for PR-comment output

`adoc review`:

- reviews Knowledge Object changes since `<base-ref>` with source-path impact and required reviewers, emitting the `adoc.review.v0` envelope
- `--patch <patch-json>` embeds an `adoc.patch.check.v0` result in the review
- supports `--format markdown` for PR-comment output

`adoc assess-changes`:

- resolves the requested base and head to commits and uses their unique merge base for the changed set and comparison snapshot
- uses the current worktree when `--head` is omitted and records whether it is clean or dirty
- compiles each snapshot under its own `agentdoc.config.yaml` while applying comparison-base exclusions to the current change
- pins lifecycle evaluation to `--as-of`, defaulting once to the current UTC date
- classifies every changed path as covered, provisional, uncovered, or explicitly excluded and emits body-free implicated objects, knowledge changes, reviewers, and proof obligations
- emits the experimental `adoc.change_assessment.v0` envelope; complete advisory outcomes exit `0`, while partial, invalid, or not-evaluated envelopes exit `2`
- supports heading-free `--format markdown` for embedding in a larger PR comment

`adoc baseline`:

- inventories every tracked path at one immutable Git ref
- uses the same covered, provisional, uncovered, and excluded classifications as pull-request assessment
- reports `readiness.ready: true` only when source is valid and every non-excluded path has authoritative coverage
- emits `adoc.repository_baseline.v0`; complete inventories exit `0` even when they are not ready

An `impacts:` entry may name an exact file or a directory prefix ending in
`/`. Prefixes are component-aware (`src/editor/` does not match
`src/editor-old/`); globs are not supported. Evidence paths remain exact.

Repositories may add optional assessment exclusions. Entries are exact files or component-aware directory prefixes ending in `/`; globs are not supported:

```yaml
assessment:
  exclude_paths:
    - vendor/
    - generated/
```

The block is intentionally absent from `adoc init`. Adding it requires a V9.2.1-capable binary because older strict config parsers reject unknown keys.

`adoc search`:

- reads compiled artifacts; it does not compile source
- defaults to config `outputs.graph`, then `dist/docs.graph.json`
- defaults to config `outputs.search`, then `dist/docs.search.json`
- runs hybrid search by default when the search artifact loads
- degrades to lexical search with one `search.artifact_missing` warning when the search artifact is absent
- accepts `--lexical` for deterministic text search over `docs.graph.json`
- accepts `--semantic` for vector-only search over `docs.search.json`
- pins exact Object ID and raw case-sensitive ID-prefix query matches in every mode
- supports `--kind`, `--status`, `--owner`, and `--source-path` filters
- supports `--related-to`, `--relation`, and `--direction` for opt-in graph candidate filtering without changing unfiltered ranking
- treats an empty lexical query plus filters as a deterministic listing of matching objects
- limits results with `--top`, defaulting to `10`
- supports `--format auto|plain|styled|json`

See [docs/design/v1-retrieval.md](../design/v1-retrieval.md) for retrieval workflow, citation guidance, model-swap behavior, and retrieval-set maintenance.

## Diagnostics

`adoc check` and `adoc build` run the same strict compiler path. Diagnostics include file, line, column, severity, diagnostic code, and fix-oriented message.

When a diagnostic belongs to a Knowledge Object, the CLI also prints `object_id`. When a targeted remediation is available, it prints `help`.

Examples:

- raw HTML emits `error[parse.raw_html]`
- unsafe links emit `error[parse.unsafe_link]`
- broken object references and relation targets emit `error[ref.broken]`
- parseable past `expires_at` values emit warning `lifecycle.expired`; the CLI reports only and does not edit source status or fields
- unreadable directories emit `error[io.unreadable_directory]`
- unsupported single-file source extensions emit `error[io.unsupported_source_extension]`

`adoc build` writes nothing when source compilation has error diagnostics. Embedding failures do not block `docs.html` or `docs.graph.json`: they emit `embed.model_load_failed`, `embed.compute_failed`, or `embed.unexpected_dim`, omit the new search sidecar, preserve any prior `docs.search.json`, and exit `1`.

## MCP tool surface

<!-- adoc:mcp-tools -->
- `adoc_init`
- `adoc_check`
- `adoc_build`
- `adoc_why`
- `adoc_graph`
- `adoc_stale`
- `adoc_contradictions`
- `adoc_impacted_by`
- `adoc_search`
- `adoc_patch_check`
- `adoc_patch_apply`
- `adoc_diff`
- `adoc_review`
- `adoc_project_status`
<!-- /adoc:mcp-tools -->

See [MCP Agent Gateway](../guides/mcp-agent-gateway.md) for configuration and permissions. Advanced managed-runtime command flags are available through `adoc <command> --help`.
