# Use Rust for the V0 CLI compiler

AgentDoc V0 will be implemented as a Rust CLI for parsing, validation, compilation, HTML rendering, and agent JSON output. The main trade-off is slower early iteration than a TypeScript prototype, but AgentDoc's core is compiler infrastructure and benefits from a fast single binary, strong AST and diagnostic types, and a durable foundation for future CLI and language-server work.
