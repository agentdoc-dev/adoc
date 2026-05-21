# ADR-0015: Contract-Tested Agent Contract Schemas

## Status

Accepted.

## Context

ADR-0014 exposed JSON Schema resources for the stable agent wire contracts. Those schemas are intentionally authored documentation, but an authored contract can drift from the actual serialized envelopes when Rust fields are added, skipped by serde, or renamed.

V2.2 needs agents to trust the resource schemas without treating private Rust DTOs as the contract.

## Decision

Keep the schemas in `docs/agent/v0/schema/` as explicit authored contracts. Add dev-only contract tests in `crates/adoc-mcp/tests/` that validate representative serialized envelopes for retrieval, graph traversal, patch input, patch check, project status, and MCP command output against those schema resources.

The tests cover serde-skipped optional fields, patch operations, project artifact diagnostics, and command envelopes. The schemas remain stable v0 resources; this branch hardens them in place rather than introducing a schema-version bump.

## Consequences

Agents get schema resources that match the current serialized surface, and future drift fails in tests.

The schemas are still not generated DTO dumps. Humans keep control of the stable contract language, while tests prove the implementation conforms.
