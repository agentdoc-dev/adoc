# Adopt domain/application/infrastructure module layout in adoc-core

**Status:** Refined by ADR-0046

> This ADR records the V0.2 layout. The current tree adds a private
> `language/` layer for pure parsing, validation, rendering, and graph
> projection; `infrastructure/` now contains I/O and runtime adapters only.
> Production `application/` modules do not import `infrastructure/`, and
> `lib.rs` composes both. See
> [ADR-0046](0046-typed-workspace-and-inward-dependencies.md) and the
> [current architecture map](../architecture.md).

## Historical Decision

AgentDoc V0.1 introduced internal hexagonal ports (ADR-0006) but kept `adoc-core`'s files in a flat `src/` layout. V0.2 lands the first **Knowledge Object** aggregate (`Claim`), and we want the DDD building blocks visible at the file-tree level before any aggregate code is written. This ADR adopts a `domain/`/`application/`/`infrastructure/` layout in `adoc-core` and pins the placement rule for every future aggregate. Aggregates, value objects, and cross-aggregate domain services live in `domain/`; the `compile_with_provider` Application Service and mutating pipeline stages live in `application/`; driven-port traits live in `domain/ports/`; their adapters and the parser, renderer, artifact-writer, and validation-rule registries live in `infrastructure/`. Validation rule traits live in `domain/rules/`; the source-page, resolved-page, and workspace rule registries and their rule structs live in `infrastructure/validate/`. The dependency direction is `domain <- application <- infrastructure`, with `lib.rs` acting as the **composition root**: the public `compile_workspace` entry point lives in `lib.rs` as a three-line wrapper that constructs `FsSourceProvider` and delegates to `application::compile::compile_with_provider`. This keeps the Application Service's only direct construction concern off `domain` (`FsSourceProvider` is never imported from `application/compile.rs`) and concentrates infrastructure wiring in a single, reviewable place.

The DDD building blocks we adopt here are the small useful set: aggregate roots with constructor-enforced invariants (`try_new` returning `Result`), value objects via single-field newtypes, an Application Service as a named layer, targeted Domain Services when behavior spans an aggregate family, and Ports & Adapters per ADR-0006. We deliberately do **not** adopt Repositories (no persistence layer in v0), Domain Events (no subscribers), or Bounded-Context-per-crate (deferred until the model justifies it).

The layout move itself is a single horizontal commit - Slice 0 of v0.2. **This is the only horizontal-slice exception in v0.2;** all subsequent slices are vertical end-to-end. The exception is recorded once in this ADR, not repeated. File paths churn once, reviewers gain an unambiguous home for new aggregates, and a future per-layer crate split is a `git mv` away.

## Addendum: scope of Slice 0 and the application-layer adapter imports

Slice 0 is mechanical file moves with zero behavior change: every test in `cargo test --workspace` remains green, the public surface re-exported from `lib.rs` keeps its current names, and `tests/public_surface.rs` compiles with no edits. The composition root is intentionally narrow - it owns the construction of `FsSourceProvider` and nothing else. `application/compile.rs` still imports `crate::infrastructure::{parser::parse_page, validate::{validate_source_page, validate_resolved_page, validate_workspace}}` because no parser or validator port traits exist in `domain/ports/` today; introducing those abstractions is a structural addition, not a file move, and is deferred until a slice's behavior justifies the trait. `ArtifactWriter` is imported directly from `domain::ports::artifact_writer`, while adapters such as `HtmlRenderer` and `GraphJsonArtifact` are imported separately from `infrastructure/`. The visible payoff today is the layout invariant - every aggregate has a clear home, every adapter has a clear home, and the construction concern is concentrated in `lib.rs`.

This ADR preserves and refines its predecessors. ADR-0003 (two-crate workspace) is unchanged. ADR-0005 (single public API) is preserved: `compile_workspace` remains the only public entry point. ADR-0006 (internal hexagonal ports) is preserved with port traits relocated to `domain/ports/`; visibility (`pub(crate)`) and contracts are unchanged. ADR-0007 (validation as a separate pass) is preserved with rule traits relocated to `domain/rules/` and registries to `infrastructure/validate/`. ADR-0008 (test taxonomy) is preserved; inline `#[cfg(test)]` blocks travel with their host file, and `tests/` continues to cover the public boundary.

## Addendum: first Domain Service boundary

`resolve_knowledge_objects` is an application pipeline stage, not a validation rule. It mutates pages in place, drops invalid pending blocks, and collects declared Object IDs for later reference resolution, so it belongs beside `compile.rs` in `application/`. The per-block Pending -> Typed conversion spans the Knowledge Object aggregate family and is therefore the first justified Domain Service: `domain/services/resolve_pending_block.rs` owns the supported-kind registry and dispatches to `Claim`, `Decision`, `Glossary`, and `Warning` builders. `infrastructure/validate/` contains rule registries and rule implementations only.

## Addendum: inline parsing boundary

`domain/inline.rs` owns the inline value model: `InlineSegment`, `InlineOrigin`, and pure projections such as `plain_text` and `to_source`. Scanning and parsing inline source text lives in `infrastructure/parser/inline.rs`. `ParsedTypedBlock` carries pre-parsed body inlines into Knowledge Object builders, so domain builders consume parsed values instead of parsing raw source text. The domain may project inline values back to plain text or source text, but it does not scan Markdown syntax.

## Addendum: Knowledge Object metadata projection

`domain/knowledge_object/projection.rs` owns the per-aggregate metadata projection. HTML and graph adapters consume that projection and keep format-specific presentation in their own modules: HTML classes, escaping, section ordering, JSON field names, and relation rendering remain adapter concerns. This preserves the aggregate boundary without turning metadata projection into a port.

## Addendum: current four-way internal layout

ADR-0046 supersedes the original placement and dependency claims where this
record put pure language mechanics in `infrastructure/` or allowed
`application/` to import concrete adapters. Current ownership is:

- `domain/`: policy, aggregates, value objects, projections, and ports.
- `language/`: pure parser, validators, source/HTML renderers, and graph
  projection.
- `application/`: typed workflow coordination through ports and language
  functions.
- `infrastructure/`: filesystem, Git, JSON artifact, and embedding adapters.
- `lib.rs`: composition root and Public Core Surface.

The architecture tests enforce this recursively. `ArtifactWriter` no longer
exists; concrete artifact encoding remains behind the composition root until
a second interchangeable implementation proves a port useful.
