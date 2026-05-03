# Use a two-crate Rust workspace

AgentDoc V0 will start as a Cargo workspace with `crates/adoc-cli` for command parsing, terminal output, file walking, and exit codes, and `crates/adoc-core` for parsing, validation, diagnostics, rendering, and artifact emission. This keeps the compiler reusable for future language-server, test, web, and agent integrations without prematurely splitting parser, schema, renderer, and storage into separate crates.
