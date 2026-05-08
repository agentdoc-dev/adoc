# AgentDoc

[![CI](https://github.com/alex-bako/adoc/actions/workflows/ci.yml/badge.svg)](https://github.com/alex-bako/adoc/actions/workflows/ci.yml)

AgentDoc is a human-readable documentation system for teams that need documentation to behave like maintained, agent-safe knowledge.

The current implementation is a pre-release Rust CLI named `adoc`. It compiles native AgentDoc Source (`.adoc`) into:

- `docs.html` for humans
- `docs.agent.json` for agents and tooling
- `docs.search.json` for local embedding-backed retrieval
- source-located diagnostics for invalid input

It also provides local, read-only retrieval over compiled artifacts with `adoc explain` and hybrid `adoc search`.

AgentDoc is not AsciiDoc, even though the source extension is `.adoc`.

## Status

AgentDoc is pre-release compiler and retrieval infrastructure. The source-to-artifact loop supports:

- `adoc init`
- `adoc check [path]`
- `adoc build [path] [--out <directory>]`
- one file or a directory of `.adoc` files as input
- config-backed defaults from `agentdoc.config.yaml` for `docs_path`, outputs, and embedding mode
- page headings with optional `@doc(id)` page identity
- path-derived page identity when no annotation exists
- headings, paragraphs, unordered lists, ordered lists, and fenced code blocks
- rich inline rendering for inline code, emphasis, strong text, and links
- typed Knowledge Objects: `claim`, `decision`, `warning`, and `glossary`
- verified claims with `owner`, `verified_at`, and V0 evidence fields
- object references written as `[[object.id]]`
- relation fields `depends_on`, `supersedes`, and `related_to`
- strict diagnostics for raw HTML, unsafe links, unclosed fenced code blocks, malformed typed blocks, malformed page annotations, invalid or duplicate Object IDs, invalid verified claims, broken references, and unsupported single-file source extensions
- diagnostic metadata with source location, severity, code, message, and `object_id`/`help` when available
- HTML, agent JSON, and search artifact emission when no error diagnostics exist
- warning-only `lifecycle.expired` diagnostics for Knowledge Objects with parseable past `expires_at` dates; source files are not mutated

V1.5 local workflow supports:

- `adoc init` creates `agentdoc.config.yaml` and `docs/index.adoc`
- omitted `check` and `build` paths use config `docs_path`
- omitted `build --out` uses config outputs
- `embeddings.provider: local|none`; missing `embeddings` defaults to `local`
- `local` uses FastEmbed `bge-small-en-v1.5` (`provider: "fastembed"`, `dim: 384`)
- first-run model download through `fastembed-rs`, then local cache reuse on later builds
- per-Object-ID vector reuse when the model header and content hash match the prior `docs.search.json`
- `--no-embeddings` to skip search artifact generation and leave any prior `docs.search.json` untouched
- hosted embedding adapters remain deferred; the shipped default provider is local

V1 local retrieval supports:

- `adoc explain <object-id>` over a compiled `docs.agent.json`
- `adoc search <query>` over `docs.agent.json` and, when present, `docs.search.json`
- text and JSON retrieval output
- hybrid search by default, fusing lexical BM25 and vector cosine ranks with Reciprocal Rank Fusion
- `--lexical` and `--semantic` escape hatches
- exact Object ID and ID-prefix pins in all search modes
- search filters for kind, status, owner, and source path

Includes, custom schemas, migrations, graph exports, semantic diff, CI/PR integrations, agent patching, a web app, hosted embedding adapters, and permissioned governance are deferred beyond the current local CLI workflow. See [docs/ROADMAP.md](docs/ROADMAP.md).

## Quick Start

### Prerequisites

- Rust `1.95.0`
- Cargo, rustfmt, and Clippy
- prek for local Git hooks

The repository pins the toolchain in [rust-toolchain.toml](rust-toolchain.toml), so Rustup will select the correct version automatically.

```bash
rustup toolchain install --no-self-update
```

### Run From Source

Build the CLI, then initialize a local AgentDoc project:

````bash
cargo build -p adoc-cli
ADOC_BIN="$(pwd)/target/debug/adoc"

mkdir -p /tmp/adoc-example
cd /tmp/adoc-example

"$ADOC_BIN" init
````

`adoc init` writes:

```text
agentdoc.config.yaml
docs/index.adoc
```

Check the source using the config default `docs_path`:

```bash
"$ADOC_BIN" check
```

Expected output:

```text
0 errors, 0 warnings
```

Build artifacts using the config output defaults:

```bash
"$ADOC_BIN" build
```

Inspect the generated files:

```bash
ls -la dist
cat dist/docs.html
cat dist/docs.agent.json
cat dist/docs.search.json
```

Expected files:

```text
docs.html
docs.agent.json
docs.search.json
```

Explicit paths still work and override config defaults where provided:

```bash
"$ADOC_BIN" check docs/index.adoc
"$ADOC_BIN" build docs/index.adoc --out /tmp/adoc-example/explicit-dist
```

### Try The Billing Pilot

The realistic V0 pilot under [examples/billing-pilot](examples/billing-pilot) exercises the full core object set: `claim`, `decision`, `warning`, and `glossary`. It contains 30+ Knowledge Objects, 8+ verified claims, object references, relations, source spans, and a golden retrieval set.

```bash
rm -rf /tmp/adoc-billing-pilot
cargo run -p adoc-cli --bin adoc -- check examples/billing-pilot
cargo run -p adoc-cli --bin adoc -- build examples/billing-pilot --out /tmp/adoc-billing-pilot
ls -la /tmp/adoc-billing-pilot
```

Expected files:

```text
docs.html
docs.agent.json
docs.search.json
```

The pilot also has [agentdoc.config.yaml](examples/billing-pilot/agentdoc.config.yaml), so config-backed local commands work from the example directory:

```bash
cd examples/billing-pilot
cargo run -p adoc-cli --manifest-path ../../Cargo.toml --bin adoc -- check
cargo run -p adoc-cli --manifest-path ../../Cargo.toml --bin adoc -- build
```

### Install Locally

To install the `adoc` binary from this checkout:

```bash
cargo install --path crates/adoc-cli --locked
```

Then run:

```bash
mkdir -p /tmp/adoc-example
cd /tmp/adoc-example
adoc init
adoc check
adoc build
```

## CLI Usage

```bash
adoc init
adoc check [path]
adoc build [path] [--out <directory>] [--no-embeddings]
adoc explain <object-id> [--artifact <path>] [--format auto|plain|styled|json]
adoc search <query> [--artifact <path>] [--search-artifact <path>] [--lexical | --semantic] [--kind <value>] [--status <value>] [--owner <value>] [--source-path <value>] [--top <n>] [--format auto|plain|styled|json]
```

`<path>` can be:

- a single `.adoc` file
- a directory, scanned recursively for `.adoc` files

`adoc init`:

- creates `agentdoc.config.yaml` and `docs/index.adoc` in the current directory
- refuses to overwrite either target if it already exists
- configures strict mode, `docs_path: docs`, `outputs.dir: dist`, exact default artifact paths, and `embeddings.provider: local`

`adoc check`:

- uses explicit `[path]` when passed
- otherwise discovers the nearest `agentdoc.config.yaml` from the current directory upward and uses `docs_path`
- compiles the input in strict mode
- prints diagnostics and a summary
- exits `0` when there are no errors
- exits `1` when any error diagnostic exists

`adoc build`:

- uses explicit `[path]` and `--out` when passed
- otherwise discovers config defaults; without `--out`, config must provide `outputs.dir` or exact `outputs.html`, `outputs.agent_json`, and `outputs.search`
- with `--out <directory>`, writes `<directory>/docs.html`, `<directory>/docs.agent.json`, and, when embeddings are enabled, `<directory>/docs.search.json`
- with config outputs, paths are resolved relative to the config file; `outputs.dir` fills omitted artifact paths as `docs.html`, `docs.agent.json`, and `docs.search.json`; exact artifact paths override the `outputs.dir` defaults
- runs the same compile path as `check`
- creates the output directory when it does not exist
- fails if the output path exists as a file
- writes `docs.html` and `docs.agent.json` when source compilation is clean
- loads the local FastEmbed `bge-small-en-v1.5` model by default through the default-on `embeddings` feature; first run may download model weights into the platform cache
- reads the prior output directory's `docs.search.json` when present and reuses vectors whose model header and content hash still match, reported as `info[build.embeddings_cached] embeddings: cached N, computed M`
- if embedding model load, compute, or dimension validation fails after clean source compilation, exits `1`, still writes `docs.html` and `docs.agent.json`, omits a new `docs.search.json`, and leaves any prior `docs.search.json` untouched
- accepts `--no-embeddings` to skip model loading and search artifact writes; any existing `docs.search.json` is left untouched and an info diagnostic `build.embeddings_skipped` is emitted
- also skips embeddings when config sets `embeddings.provider: none`; config `local` and missing `embeddings` both enable the shipped local provider

`adoc explain`:

- reads a compiled agent artifact; it does not compile source
- defaults to config `outputs.agent_json`, then `dist/docs.agent.json`
- prints the matching Knowledge Object with source and relation metadata
- supports `--format auto|plain|styled|json`

`adoc search`:

- reads compiled artifacts; it does not compile source
- defaults to config `outputs.agent_json`, then `dist/docs.agent.json`
- defaults to config `outputs.search`, then `dist/docs.search.json`
- runs hybrid search by default when the search artifact loads
- degrades to lexical search with one `search.artifact_missing` warning when the search artifact is absent
- accepts `--lexical` for deterministic text search over `docs.agent.json`
- accepts `--semantic` for vector-only search over `docs.search.json`
- pins exact Object ID and raw case-sensitive ID-prefix query matches in every mode
- supports `--kind`, `--status`, `--owner`, and `--source-path` filters
- treats an empty lexical query plus filters as a deterministic listing of matching objects
- limits results with `--top`, defaulting to `10`
- supports `--format auto|plain|styled|json`

See [docs/v1-retrieval.md](docs/v1-retrieval.md) for retrieval workflow, citation guidance, model-swap behavior, and retrieval-set maintenance.

## AgentDoc Source

The V0 source grammar is intentionally small.

````adoc
# Page Title @doc(product.area)

Paragraph text is plain prose.

- Unordered item
- Another unordered item

1. Ordered item
2. Another ordered item

```text
Fenced code is preserved and escaped in HTML.
```
````

Typed Knowledge Objects use top-level fenced blocks:

````adoc
::claim billing.ledger
status: verified
owner: team-billing
verified_at: 2026-05-06
source: ledger reconciliation report
--
The ledger records every credit and refund balance movement.
::

::decision billing.refund-policy
status: accepted
decided_by: architecture
depends_on: [billing.ledger, billing.credit-balance]
--
Use policy-based refund approval with ledger-backed audit entries.
::

::warning billing.invoice.manual-adjustment
severity: high
related_to: billing.ledger
--
Manual invoice adjustments must cite [[billing.ledger]] before approval.
::

::glossary billing.credit-balance
--
The customer-visible balance available for future invoices.
::
````

Supported object kinds:

- `claim`
- `decision`
- `warning`
- `glossary`

Supported relation fields:

- `depends_on`
- `supersedes`
- `related_to`

Relation values can be a single Object ID, a comma-separated list, or a bracket array. The compiler deduplicates repeated targets while preserving first occurrence order. A trailing empty segment from a final comma is ignored; leading or interior empty segments emit `id.invalid`. Valid targets that do not resolve to a declared Knowledge Object emit `ref.broken`; malformed targets emit `id.invalid`.

Object references use `[[object.id]]` in prose, headings, list items, and typed object bodies. References are rendered as HTML links and preserved as citeable source text in agent JSON object bodies.

Page annotations are optional. IDs must be lowercase dot-separated kebab-case values with at least two segments, such as `product.area`. If the first heading does not include `@doc(id)`, the compiler derives the page identity from the file path and applies the same ID grammar.

Raw HTML is rejected in strict mode:

```adoc
<div>not allowed</div>
```

Unclosed fenced code blocks are rejected:

````adoc
```rust
fn main() {}
````

Current limitations:

- custom schemas, includes, migrations, graph exports, semantic diff, CI/PR integrations, agent patching, hosted embedding adapters, web app, and permissions are deferred
- config is intentionally minimal: strict mode only, one `docs_path`, output paths, and `embeddings.provider: local|none`

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

`adoc build` writes nothing when source compilation has error diagnostics. Embedding failures do not block `docs.html` or `docs.agent.json`: they emit `embed.model_load_failed`, `embed.compute_failed`, or `embed.unexpected_dim`, omit the new search sidecar, preserve any prior `docs.search.json`, and exit `1`.

## Smoke Tests

Run the happy path:

```bash
rm -rf /tmp/adoc-smoke
mkdir -p /tmp/adoc-smoke

cat > /tmp/adoc-smoke/guide.adoc <<'EOF'
# Getting Started @doc(docs.getting-started)

AgentDoc keeps knowledge readable.

- Write source
- Run check
- Build artifacts
EOF

cargo run -p adoc-cli --bin adoc -- check /tmp/adoc-smoke/guide.adoc
cargo run -p adoc-cli --bin adoc -- build /tmp/adoc-smoke/guide.adoc --out /tmp/adoc-smoke/dist

ls -la /tmp/adoc-smoke/dist
cat /tmp/adoc-smoke/dist/docs.html
cat /tmp/adoc-smoke/dist/docs.agent.json
```

Expected:

- `check` exits `0`
- `build` exits `0`
- `docs.html` exists
- `docs.agent.json` exists
- `docs.search.json` exists
- agent JSON includes `schema_version`, `pages`, `"objects": []`, and `"diagnostics": []`

Run strict-mode failure checks:

```bash
cat > /tmp/adoc-smoke/raw-html.adoc <<'EOF'
# Unsafe @doc(docs.unsafe)

<div>raw html</div>
EOF

cargo run -p adoc-cli --bin adoc -- check /tmp/adoc-smoke/raw-html.adoc
```

Expected: non-zero exit with `error[parse.raw_html]`.

````bash
cat > /tmp/adoc-smoke/unclosed-fence.adoc <<'EOF'
# Broken @doc(docs.broken)

```rust
fn main() {}
EOF

cargo run -p adoc-cli --bin adoc -- check /tmp/adoc-smoke/unclosed-fence.adoc
````

Expected: non-zero exit with `error[parse.unclosed_fence]`.

```bash
echo "not a directory" > /tmp/adoc-smoke/out-file
cargo run -p adoc-cli --bin adoc -- build /tmp/adoc-smoke/guide.adoc --out /tmp/adoc-smoke/out-file
```

Expected: non-zero exit with `error[io.output_not_directory]`.

## Development

This is a Cargo workspace:

```text
crates/
  adoc-cli/   # command-line adapter, file output, exit codes
  adoc-core/  # compile API, parser, diagnostics, renderers, artifacts
```

The architectural contract is documented in [docs/V0-DESIGN.md](docs/V0-DESIGN.md).

### Quality Gates

Single test command:

```bash
cargo test --workspace --locked
```

Run the same full check set as CI:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --locked
```

Install the pre-commit hook:

```bash
prek install
```

Run the hook suite manually:

```bash
prek run --all-files
```

Useful focused commands:

```bash
cargo test -p adoc-cli
cargo test -p adoc-core
cargo run -p adoc-cli --bin adoc -- check <path>
cargo run -p adoc-cli --bin adoc -- build <path> --out dist
```

The `embeddings` feature is default-on and enables the FastEmbed dependency. Build without it with `cargo test -p adoc-core --no-default-features` or equivalent no-default build commands when embedding support is intentionally excluded.

Hermetic CLI/core tests use the deterministic in-memory embedding provider through the `test-embedding-provider` feature only when `ADOC_TEST_EMBEDDING_PROVIDER=in-memory` is set. With that feature enabled, unset `ADOC_TEST_EMBEDDING_PROVIDER` and `ADOC_TEST_EMBEDDING_PROVIDER=fastembed` both use FastEmbed. FastEmbed end-to-end coverage is gated behind `fastembed-it`:

```bash
cargo test -p adoc-core --features fastembed-it --no-run --locked
```

Format code before committing:

```bash
cargo fmt --all
```

## Continuous Integration

CI runs on pushes and pull requests to `main` using [.github/workflows/ci.yml](.github/workflows/ci.yml).

The workflow checks:

- formatting
- Clippy with warnings denied
- workspace tests
- workspace build
- documentation build with rustdoc warnings denied

Dependabot is configured in [.github/dependabot.yml](.github/dependabot.yml) for Cargo and GitHub Actions updates.

## Project Documents

- [CONTEXT.md](CONTEXT.md): project language and domain decisions
- [docs/PRD.md](docs/PRD.md): product requirements
- [docs/ROADMAP.md](docs/ROADMAP.md): product roadmap from completed V0 through planned retrieval, review, patching, schema, graph, and team surfaces
- [docs/V0-DESIGN.md](docs/V0-DESIGN.md): Rust implementation contract
- [docs/adr/](docs/adr): architecture decision records

## Architecture

AgentDoc V0 is intentionally shaped as a compiler pipeline:

```text
AgentDoc Source
  -> adoc-core compile_workspace()
  -> parser and diagnostics
  -> HTML renderer
  -> agent JSON artifact
  -> adoc-cli exit codes and file output
```

The public Rust API is deliberately small:

```rust
pub fn compile_workspace(input: CompileInput) -> CompileResult;
pub fn build_workspace(input: BuildInput) -> CompileResult;
```

Parser, validation, renderer, and artifact internals stay private until another real consumer needs lower-level APIs.

## Roadmap

V0 is complete for the local source-to-artifact compiler loop. Implemented milestones include:

- richer page identity and source diagnostics
- common prose rendering for inline code, emphasis, and links
- first `claim` Knowledge Object
- verified claim evidence fields
- `decision`, `warning`, and `glossary`
- object references and relations
- multi-file project behavior
- standardized diagnostics and production-usable fixtures
- a realistic billing pilot
- artifact-backed `adoc explain <object-id>`
- hybrid `adoc search <query>` over `docs.agent.json` and `docs.search.json`
- `adoc init` and minimal `agentdoc.config.yaml`

Current local retrieval focuses on the existing flat agent artifact:

- define the supported `docs.agent.json` read contract
- support `adoc explain <object-id>` for object lookup and citation
- support `adoc search <query>` for deterministic lexical and local embedding-backed search
- prove retrieval against the billing pilot
- build `docs.search.json` with local FastEmbed embeddings

Later milestones cover migration, review workflows, patch safety, expanded schema, graph exports, composition, hosted embedding adapters, and team surfaces.

See [docs/ROADMAP.md](docs/ROADMAP.md) for the full sequence.

## License

This project declares the MIT license in Cargo package metadata.
