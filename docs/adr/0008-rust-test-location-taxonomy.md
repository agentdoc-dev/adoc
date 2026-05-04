# Standardize Rust test locations by behavioral boundary

AgentDoc uses the standard Rust test taxonomy:

- Inline `#[cfg(test)]` modules cover private white-box behavior. Keep parser,
  scanner, source-span, identity-validation, renderer-internal, validation-rule,
  artifact, and compile-pipeline orchestration tests next to the code they
  exercise when those tests need private functions, private adapters, or
  test-only in-memory ports.
- Crate `tests/` directories cover public behavior. Put tests for
  `compile_workspace()`, documented public API imports, CLI commands, fixtures,
  goldens, and snapshots there.

Do not widen item visibility, add public re-exports, or introduce public
test seams solely to move a private test out of an implementation file. A test
that proves private scanner semantics belongs inline even if the behavior is
user-visible through a later validator diagnostic. The public behavior should
still be covered at the public boundary, but the private edge case stays where
the private contract is defined.

For `adoc-core`, `tests/public_surface.rs` remains the public API contract
test. Other `adoc-core/tests/` files should exercise `compile_workspace()` as
the public compile boundary and share fixture workspace helpers through
`tests/support/mod.rs`.

For `adoc-cli`, command behavior, CLI fixtures, goldens, snapshots, and snapshot
approval files stay under `crates/adoc-cli/tests/`. Shared command-test helpers
belong in `tests/support/mod.rs`; CLI tests should not move inline.
