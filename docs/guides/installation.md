# Install AgentDoc

## Current source preview (0.4.0)

Use this route for the current README and MCP gateway. Rust 1.95.0 and native
build tools are required: Xcode Command Line Tools on macOS, or a C/C++ compiler,
linker, pkg-config and OpenSSL development headers on Linux. Install Rust with
[Rustup](https://rustup.rs/). Git hooks are not required to use AgentDoc.

```sh
git clone https://github.com/agentdoc-dev/adoc.git
cd adoc
rustup toolchain install --no-self-update
cargo build --release --locked -p adoc-cli -p adoc-mcp
export PATH="$PWD/target/release:$PATH"
adoc --version
```

The last command prints `adoc 0.4.0`. The PATH change lasts for this shell; keep
this checkout or install both executables persistently with Cargo:

```sh
cargo install --path crates/adoc-cli --locked
cargo install --path crates/adoc-mcp --locked
```

Cargo places them in its bin directory, normally `~/.cargo/bin`; add that directory
to your PATH if Rustup did not. Both commands install from this checkout, **not
crates.io**. Build/install CLI and gateway from the same revision.

## Try a fresh project

From the repository root, after building:

```sh
ADOC_EXAMPLE="$PWD/examples/quickstart/refunds.adoc"
mkdir ../agentdoc-demo
cd ../agentdoc-demo
adoc init
cp "$ADOC_EXAMPLE" docs/index.adoc
adoc check
adoc build --no-embeddings
adoc search refund --lexical
adoc why billing.refund-window
```

`check` reports `0 errors, 0 warnings`. The result includes the refund statement,
its draft status, and a source location in `docs/index.adoc`. Open `dist/docs.html`
in a browser to read the generated document (`open dist/docs.html` on macOS,
`xdg-open dist/docs.html` on a Linux desktop).

This first run uses lexical search without downloading a model. To enable local
semantic/hybrid retrieval:

```sh
adoc build
adoc search "How long do I have to request a refund?"
```

The first model-backed build downloads FastEmbed's bge-small-en-v1.5 model. Later
runs reuse its cache. Inference runs locally. If a proxy, offline machine, or model
load blocks that step, use `build --no-embeddings` and `search --lexical`; semantic
search requires the model and a successfully generated search artifact. The
no-embedding build leaves any older search artifact untouched.

`init` refuses existing config/source files. Use a new directory instead of
removing existing work. For invalid source, fix the source-located diagnostics and
rerun `check`; failed validation does not make the source valid by skipping build.

## Platforms and downloadable versions

| Platform | Current path | Packaging |
|---|---|---|
| macOS Apple Silicon | Source build; locally exercised CLI/MCP | Next release workflow builds an arm64 archive |
| macOS Intel | Source build; not locally qualified by this launch audit | Next release workflow builds an x86_64 archive and runs its smoke check |
| Linux x86_64 / arm64 | Source build, or published 0.3.4 CLI with its own docs | Release workflow runs natively for each architecture |
| Windows | No supported launch installation path yet | No release artifact; compatibility unverified |

The matrix describes build/release routes, not a claim that a future archive has
already been published. [Version compatibility](releases.md) separates the 0.4.0
source preview (graph v6/search v2) from published 0.3.4 (graph v5/search v1).
Current 0.3.4 archives contain the CLI only; do not expect an MCP binary inside.

Download only from [GitHub releases](https://github.com/agentdoc-dev/adoc/releases).
Choose the OS/architecture explicitly and download its archive plus `.sha256`.
For example, in a directory containing the two matching files:

```sh
shasum -a 256 -c adoc-v0.3.4-x86_64-unknown-linux-gnu.tar.gz.sha256
tar -xzf adoc-v0.3.4-x86_64-unknown-linux-gnu.tar.gz
./adoc --version
```

These are **Linux** binaries. Use the
[0.3.4 README](https://github.com/agentdoc-dev/adoc/blob/v0.3.4/README.md) for them.
Future CLI/MCP archives from the current workflow retain this filename convention
and include `adoc`, `adoc-mcp`, and `LICENSE`. Where an attestation is available:

```sh
gh attestation verify <archive.tar.gz> --repo agentdoc-dev/adoc
```

Checksums detect a changed download; attestations provide separate provenance.
Do not bypass a failed check. macOS distribution signing/notarization is not
promised; use the source-build path if local platform policy disallows an archive.

Next: [connect an MCP agent](mcp-agent-gateway.md), or
[contribute to AgentDoc](../../CONTRIBUTING.md).
