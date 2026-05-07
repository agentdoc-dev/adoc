# AgentDoc

[![CI](https://github.com/alex-bako/adoc/actions/workflows/ci.yml/badge.svg)](https://github.com/alex-bako/adoc/actions/workflows/ci.yml)

AgentDoc is a human-readable documentation system for teams that need documentation to behave like maintained, agent-safe knowledge.

The current implementation is a pre-release Rust CLI named `adoc`. It compiles native AgentDoc Source (`.adoc`) into:

- `docs.html` for humans
- `docs.agent.json` for agents and tooling
- `docs.search.json` for local embedding-backed retrieval
- source-located diagnostics for invalid input

It also provides local, read-only retrieval over `docs.agent.json` with `adoc explain` and lexical-only `adoc search`.

AgentDoc is not AsciiDoc, even though the source extension is `.adoc`.

## Status

AgentDoc is pre-release compiler and retrieval infrastructure. The source-to-artifact loop supports:

- `adoc check <path>`
- `adoc build <path> --out <directory>`
- one file or a directory of `.adoc` files as input
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

V1.3 build embeddings support:

- `adoc build <path> --out <directory>` emits `docs.search.json` by default
- local FastEmbed embeddings using `bge-small-en-v1.5` (`provider: "fastembed"`, `dim: 384`)
- first-run model download through `fastembed-rs`, then local cache reuse on later builds
- per-Object-ID vector reuse when the model header and content hash match the prior `docs.search.json`
- `--no-embeddings` to skip search artifact generation and leave any prior `docs.search.json` untouched

V1.2 local retrieval supports:

- `adoc explain <object-id>` over a compiled `docs.agent.json`
- `adoc search <query>` over the same agent artifact
- text and JSON retrieval output
- search filters for kind, status, owner, and source path

V1.2 search is lexical-only. It reads `docs.agent.json` only; it does not read `docs.search.json`, run semantic mode, or perform hybrid ranking yet.

Config files, includes, custom schemas, migrations, graph exports, semantic diff, CI/PR integrations, agent patching, a web app, and permissioned governance are deferred beyond the current V0 compiler loop. See [docs/ROADMAP.md](docs/ROADMAP.md).

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

Create a small AgentDoc Source file:

````bash
mkdir -p /tmp/adoc-example

cat > /tmp/adoc-example/guide.adoc <<'EOF'
# Getting Started @doc(docs.getting-started)

AgentDoc keeps knowledge readable.

- Write source
- Run check
- Build artifacts

```rust
fn main() {
    println!("hello from AgentDoc");
}
```
EOF
````

Check the source:

```bash
cargo run -p adoc-cli --bin adoc -- check /tmp/adoc-example/guide.adoc
```

Expected output:

```text
0 errors, 0 warnings
```

Build artifacts:

```bash
cargo run -p adoc-cli --bin adoc -- build /tmp/adoc-example/guide.adoc --out /tmp/adoc-example/dist
```

Inspect the generated files:

```bash
ls -la /tmp/adoc-example/dist
cat /tmp/adoc-example/dist/docs.html
cat /tmp/adoc-example/dist/docs.agent.json
cat /tmp/adoc-example/dist/docs.search.json
```

Expected files:

```text
docs.html
docs.agent.json
docs.search.json
```

### Try The Billing Pilot

The realistic V0 pilot under [examples/billing-pilot](examples/billing-pilot) exercises the full core object set: `claim`, `decision`, `warning`, and `glossary`. It contains 20+ Knowledge Objects, 5+ verified claims, object references, relations, and source spans in the agent artifact.

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

### Install Locally

To install the `adoc` binary from this checkout:

```bash
cargo install --path crates/adoc-cli --locked
```

Then run:

```bash
adoc check /tmp/adoc-example/guide.adoc
adoc build /tmp/adoc-example/guide.adoc --out /tmp/adoc-example/dist
```

## CLI Usage

```bash
adoc check <path>
adoc build <path> --out <directory> [--no-embeddings]
adoc explain <object-id> [--artifact <path>] [--format text|json]
adoc search <query> [--artifact <path>] [--kind <value>] [--status <value>] [--owner <value>] [--source-path <value>] [--top <n>] [--format text|json]
```

`<path>` can be:

- a single `.adoc` file
- a directory, scanned recursively for `.adoc` files

`adoc check`:

- compiles the input in strict mode
- prints diagnostics and a summary
- exits `0` when there are no errors
- exits `1` when any error diagnostic exists

`adoc build`:

- runs the same compile path as `check`
- creates the output directory when it does not exist
- fails if the output path exists as a file
- writes `docs.html`, `docs.agent.json`, and `docs.search.json` only when there are no errors
- loads the local FastEmbed `bge-small-en-v1.5` model by default; first run may download model weights into the platform cache
- reads the prior output directory's `docs.search.json` when present and reuses vectors whose model header and content hash still match
- accepts `--no-embeddings` to skip model loading and search artifact writes; any existing `docs.search.json` is left untouched and an info diagnostic `build.embeddings_skipped` is emitted

`adoc explain`:

- reads a compiled agent artifact; it does not compile source
- defaults to `--artifact dist/docs.agent.json`
- prints the matching Knowledge Object with source and relation metadata
- supports `--format text|json`

`adoc search`:

- reads a compiled agent artifact; it does not compile source
- defaults to `--artifact dist/docs.agent.json`
- runs deterministic lexical search over `docs.agent.json`
- pins exact Object ID and raw case-sensitive ID-prefix query matches above BM25 results
- supports `--kind`, `--status`, `--owner`, and `--source-path` filters
- limits results with `--top`, defaulting to `10`
- supports `--format text|json`

V1.2 `adoc search` does not use `docs.search.json`, semantic search, or hybrid ranking.

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

- `adoc search` is lexical-only and reads `docs.agent.json` only
- `adoc init`, custom schemas, includes, config files, semantic search, hybrid ranking, migrations, graph exports, semantic diff, CI/PR integrations, agent patching, web app, and permissions are deferred

## Diagnostics

`adoc check` and `adoc build` run the same strict compiler path. Diagnostics include file, line, column, severity, diagnostic code, and fix-oriented message.

When a diagnostic belongs to a Knowledge Object, the CLI also prints `object_id`. When a targeted remediation is available, it prints `help`.

Examples:

- raw HTML emits `error[parse.raw_html]`
- unsafe links emit `error[parse.unsafe_link]`
- broken object references and relation targets emit `error[ref.broken]`
- unreadable directories emit `error[io.unreadable_directory]`
- unsupported single-file source extensions emit `error[io.unsupported_source_extension]`

`adoc build` writes `docs.html`, `docs.agent.json`, and `docs.search.json` only when there are no error diagnostics. Embedding failures emit `embed.model_load_failed`, `embed.compute_failed`, or `embed.unexpected_dim` and block artifact writes.

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

Hermetic CLI/core tests use the deterministic in-memory embedding provider through the `test-embedding-provider` feature. FastEmbed end-to-end coverage is gated behind `fastembed-it` so default tests do not download model weights:

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
- lexical-only `adoc search <query>` over `docs.agent.json`

Current V1 retrieval focuses on the existing flat agent artifact:

- define the supported `docs.agent.json` read contract
- support `adoc explain <object-id>` for object lookup and citation
- support `adoc search <query>` for deterministic lexical local search
- prove retrieval against the billing pilot
- build `docs.search.json` with local FastEmbed embeddings for the next semantic retrieval slice

Later milestones cover semantic and hybrid search, project ergonomics, migration, review workflows, patch safety, expanded schema, graph exports, composition, and team surfaces.

See [docs/ROADMAP.md](docs/ROADMAP.md) for the full sequence.

## License

This project declares the MIT license in Cargo package metadata.
