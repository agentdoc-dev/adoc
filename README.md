# AgentDoc

[![CI](https://github.com/agentdoc-dev/adoc/actions/workflows/ci.yml/badge.svg)](https://github.com/agentdoc-dev/adoc/actions/workflows/ci.yml)

**Documentation your coding agent can validate, search, and cite.**

Write policies, decisions, and constraints alongside your code. AgentDoc checks their structure and references, builds readable HTML, and gives agents a local knowledge graph with source citations through a Rust CLI and MCP gateway.

For example, a refund policy becomes a named, retrievable claim:

```adoc
# Refund policy @doc(billing.refunds)

::claim billing.refund-window
status: draft
owner: billing
--
Customers can request a refund within 30 days of purchase.
::
```

Run `adoc why billing.refund-window` to retrieve it:

```text
Object: billing.refund-window
Kind: claim
Status: draft
Owner: billing

Statement:
Customers can request a refund within 30 days of purchase.

Source: docs/index.adoc:3:1
```

The citation points back to editable source. Validation checks the document's structure and declared evidence; it does not establish that the statement is true.

## Try it locally

This branch contains the **1.0.0-alpha.1** prerelease. The previous release was the older **0.3.4 Linux CLI**, with different artifact formats. Build from source for the workflow below.

Install [Rustup](https://rustup.rs/) and your platform's native build tools, then:

```sh
git clone https://github.com/agentdoc-dev/adoc.git
cd adoc
rustup toolchain install --no-self-update
cargo build --release --locked -p adoc-cli -p adoc-mcp
export PATH="$PWD/target/release:$PATH"
```

The repository pins Rust 1.95.0. See [installation](docs/guides/installation.md) for platform requirements, persistent installation, downloads, and checksums.

Create a project using the example above:

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

`check` reports `0 errors, 0 warnings`. Search returns the policy heading and claim; `why` returns the cited claim shown above. Open `dist/docs.html` in a browser. The build also writes `dist/docs.graph.json` for tooling.

This first build needs no model download. For local semantic and hybrid search, run `adoc build`, then `adoc search "How long do I have to request a refund?"`. The first model-backed build downloads FastEmbed's `bge-small-en-v1.5`; subsequent inference runs locally. Use `--no-embeddings` and `--lexical` when working offline.

`init` refuses to overwrite existing starter files. Start in an empty project directory, fix any source-located errors, and rebuild after changing documents: retrieval reads compiled artifacts.

## Give your coding agent access

The `adoc-mcp` binary exposes local retrieval over stdio. In **Claude Code**, register the executable built above using its absolute path:

```sh
claude mcp add --transport stdio --scope local agentdoc -- /absolute/path/to/adoc/target/release/adoc-mcp
```

Start Claude Code in `agentdoc-demo`, check the connection with `/mcp`, and ask:

> Use AgentDoc to check this project's status, then explain `billing.refund-window` and cite its source. Pass this project's absolute directory as `project_root` in both calls.

The [MCP guide](docs/guides/mcp-agent-gateway.md) covers the tool list, other client configuration, and permissions. The gateway defaults to public-only retrieval; patch application is disabled unless the operator enables it. It opens no network listener.

## What you can do

- **Validate knowledge:** check typed objects, IDs, references, required fields, and strict markup.
- **Retrieve with context:** search prose and objects, look up an object with `why`, or follow relations with `graph`.
- **Maintain documentation:** find stale and contradictory records, inspect change impact, and validate proposed patches.
- **Read the same source:** generate HTML for people and graph/search artifacts for tools.

AgentDoc Source uses `.adoc`, but **it is not AsciiDoc**. It combines prose with 15 built-in object kinds and explicit relations. Markdown ingestion and migration are also supported; Markdown prose can be searched and cited without first converting it into typed objects.

For a larger example, see the [billing pilot](examples/billing-pilot). The [CLI reference](docs/reference/cli.md) covers command flags and diagnostics; the [Source reference](docs/reference/source.md) covers syntax and object kinds. [CI integration](docs/guides/ci-integration.md) describes the separate released Action/CLI assessment workflow.

## Maturity and limits

AgentDoc is pre-release software. The source preview uses graph v6 and search v2; rebuild artifacts when upgrading from 0.3.4 and recreate patches based on older hashes. See [release compatibility](docs/guides/releases.md).

There is no hosted service or web application in this local workflow. Custom schemas, automatic semantic contradiction detection, and managed multi-repository governance are not shipped. Platform verification and release availability are listed in the [installation guide](docs/guides/installation.md).

HTML can be filtered by audience. The canonical graph is **not** an audience-filtered export: protect generated artifacts as you would their source. Model download failures can leave an older search artifact in place; use lexical search until a successful rebuild.

## Contribute

Bug reports, examples, documentation improvements, and focused code changes are welcome. Start with [CONTRIBUTING.md](CONTRIBUTING.md) for setup, checks, and help. Report vulnerabilities privately through [SECURITY.md](SECURITY.md).

The [product index](docs/product/README.md) and [roadmap](docs/roadmap/ROADMAP-V10.md) describe the project's direction. AgentDoc is [MIT licensed](LICENSE), including revisions predating the license file.
