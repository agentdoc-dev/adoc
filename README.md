# AgentDoc

[![CI](https://github.com/alex-bako/adoc/actions/workflows/ci.yml/badge.svg)](https://github.com/alex-bako/adoc/actions/workflows/ci.yml)

AgentDoc is a human-readable documentation system for teams that need documentation to behave like maintained, agent-safe knowledge.

The current implementation is an early Rust CLI named `adoc`. It compiles native AgentDoc Source (`.adoc`) into:

- `docs.html` for humans
- `docs.agent.json` for agents and tooling
- source-located diagnostics for invalid input

AgentDoc is not AsciiDoc, even though the source extension is `.adoc`.

## Status

AgentDoc is pre-release compiler infrastructure. The V0.1 implementation supports a prose page slice:

- `adoc check <path>`
- `adoc build <path> --out <directory>`
- one file or a directory of `.adoc` files as input
- page headings with optional `@doc(id)` page identity
- path-derived page identity when no annotation exists
- headings, paragraphs, unordered lists, ordered lists, and fenced code blocks
- strict diagnostics for raw HTML and unclosed fenced code blocks
- HTML and agent JSON artifact emission when no error diagnostics exist

Typed Knowledge Objects such as `claim`, `decision`, `warning`, and `glossary` are planned for later V0 slices. See [docs/ROADMAP.md](docs/ROADMAP.md).

## Quick Start

### Prerequisites

- Rust `1.95.0`
- Cargo, rustfmt, and Clippy

The repository pins the toolchain in [rust-toolchain.toml](rust-toolchain.toml), so Rustup will select the correct version automatically.

```bash
rustup toolchain install --no-self-update
```

### Run From Source

Create a small AgentDoc Source file:

````bash
mkdir -p /tmp/adoc-example

cat > /tmp/adoc-example/guide.adoc <<'EOF'
# Getting Started @doc(getting-started)

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
```

Expected files:

```text
docs.html
docs.agent.json
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
adoc build <path> --out <directory>
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
- writes `docs.html` and `docs.agent.json` only when there are no errors

## AgentDoc Source

The V0.1 source grammar is intentionally small.

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

Page annotations are optional. If the first heading does not include `@doc(id)`, the compiler derives the page identity from the file path.

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

- inline code, emphasis, and links are not rendered as rich inline HTML yet
- typed Knowledge Objects are not implemented yet
- custom schemas, includes, config files, search, and migrations are deferred

## Smoke Tests

Run the happy path:

```bash
rm -rf /tmp/adoc-smoke
mkdir -p /tmp/adoc-smoke

cat > /tmp/adoc-smoke/guide.adoc <<'EOF'
# Getting Started @doc(getting-started)

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
- agent JSON includes `schema_version`, `pages`, `"objects": []`, and `"diagnostics": []`

Run strict-mode failure checks:

```bash
cat > /tmp/adoc-smoke/raw-html.adoc <<'EOF'
# Unsafe @doc(unsafe)

<div>raw html</div>
EOF

cargo run -p adoc-cli --bin adoc -- check /tmp/adoc-smoke/raw-html.adoc
```

Expected: non-zero exit with `error[parse.raw_html]`.

````bash
cat > /tmp/adoc-smoke/unclosed-fence.adoc <<'EOF'
# Broken @doc(broken)

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

Run the same checks as CI:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --locked
```

Useful focused commands:

```bash
cargo test -p adoc-cli
cargo test -p adoc-core
cargo run -p adoc-cli --bin adoc -- check <path>
cargo run -p adoc-cli --bin adoc -- build <path> --out dist
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
- [docs/ROADMAP.md](docs/ROADMAP.md): V0 tracer-bullet roadmap
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
```

Parser, validation, renderer, and artifact internals stay private until another real consumer needs lower-level APIs.

## Roadmap

The next V0 milestones add:

- richer page identity and source diagnostics
- common prose rendering for inline code, emphasis, and links
- first `claim` Knowledge Object
- verified claim evidence fields
- `decision`, `warning`, and `glossary`
- object references and relations
- multi-file project behavior

See [docs/ROADMAP.md](docs/ROADMAP.md) for the full sequence.

## License

This project declares the MIT license in Cargo package metadata.
