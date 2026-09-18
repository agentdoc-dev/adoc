# Contributing to AgentDoc

Thanks for improving AgentDoc. The project is a Rust workspace; the current
source preview is `0.4.0`. The released Linux CLI is `0.3.4`.

## Before you start

- Install Rust through Rustup. This repository pins Rust `1.95.0` in
  [`rust-toolchain.toml`](rust-toolchain.toml); Rustup installs `rustfmt` and
  Clippy from that pin.
- Clone your fork, create a focused branch, and build from the checkout:

  ```sh
  cargo build -p adoc-cli --locked
  ```

- Run the CLI from source with `cargo run -p adoc-cli --bin adoc -- <command>`.
  The [installation guide](docs/guides/installation.md) contains the first-project flow.

## Make and check a change

Keep pull requests small, describe the user-visible effect, and include focused
tests when behavior changes. Before opening a pull request, run the applicable
checks:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --locked
cargo clippy -p adoc-core --no-default-features --all-targets --locked -- -D warnings
# Install cargo-deny separately if checking dependency policy locally.
cargo deny check licenses bans sources
python3 scripts/smoke-test.py --bin-dir target/debug
python3 scripts/check-doc-links.py
```

CI runs the Rust, dependency-policy, documentation-link and smoke checks on pull requests.
It also runs the checksum-pinned validation-runtime parity harness. The model-backed
retrieval pilot is opt-in via CI workflow dispatch; it is not part of the default
test pass. To run real-model smoke locally, add `--embeddings` to the smoke command. [`prek`](https://prek.j178.dev/) is
optional local hook tooling; install its hooks with `prek install` and run
`prek run --all-files` if you use it.

Open a pull request from your fork using the provided template. Normal CI runs
for fork pull requests. The optional Claude review is intentionally limited to
human pull requests from this repository, because it uses repository secrets.

## Report a problem or ask for help

For non-sensitive bugs, use the [bug report form](https://github.com/agentdoc-dev/adoc/issues/new?template=bug_report.yml).
For usage questions or other public help, use the
[question form](https://github.com/agentdoc-dev/adoc/issues/new?template=question.yml). Include a minimal
reproduction, expected behavior, and actual behavior where applicable.

Do not disclose vulnerabilities in public issues. Follow
[SECURITY.md](SECURITY.md) for private reporting.

Start with the [current product index](docs/product/README.md) and
[V10 execution map](docs/roadmap/v10/EXECUTION-MAP.md) when proposing product changes.
The [roadmap](docs/roadmap/ROADMAP.md) records planned work; it is not a promise
that an idea will be accepted or scheduled.
