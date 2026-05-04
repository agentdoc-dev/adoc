# AgentDoc

AgentDoc is a documentation product for maintaining trusted, agent-safe organizational knowledge. This context captures the project language we have resolved while cutting the initial PRD into smaller milestones.

## Language

**AgentDoc**:
A documentation system that turns readable source files into typed, evidence-backed knowledge for humans and agents.
_Avoid_: Markdown replacement, docs CMS

**AgentDoc Source**:
The human-readable `.adoc` file format authors write before compilation.
_Avoid_: Markdown file, source of truth

**Knowledge Object**:
A durable unit of organizational knowledge with a stable identity, lifecycle status, ownership, and optional evidence.
_Avoid_: chunk, paragraph, section

**Agent-Facing Artifact**:
A compiled output that agents can retrieve and cite instead of scraping arbitrary prose.
_Avoid_: RAG dump, text chunks

**Local CLI**:
The initial `adoc` command-line product surface for checking, compiling, and querying AgentDoc inside a Git repository.
_Avoid_: initial web app, SaaS-first product

**Native Authoring**:
The initial workflow where users write AgentDoc Source directly instead of importing existing Markdown.
_Avoid_: Markdown migration, compatibility-first workflow

**Core Object Set**:
The first supported Knowledge Object types: `claim`, `decision`, `warning`, and `glossary`.
_Avoid_: full schema registry, all PRD block types

**Verified Claim**:
A `claim` Knowledge Object that has enough evidence and review metadata for agents to treat it as current within its stated scope.
_Avoid_: high-confidence statement, trusted paragraph

**V0 Evidence Fields**:
The first evidence metadata accepted for a Verified Claim: `source`, `test`, and `reviewed_by`.
_Avoid_: evidence vault, evidence quality score, full evidence model

**CLI Command**:
The executable command users run for the Local CLI: `adoc`.
_Avoid_: `agentdoc`

**Strict Mode**:
The only validation mode in v0; malformed structure, unknown object types, duplicate IDs, broken references, invalid verified claims, and raw HTML are errors.
_Avoid_: compatibility mode, permissive import mode

**V0 CLI Commands**:
The first supported CLI commands: `adoc check` and `adoc build`.
_Avoid_: full command surface, search-first CLI, initializer-first CLI

**V0 Defaults**:
The first CLI behavior is driven by command arguments and built-in defaults, without a project config file.
_Avoid_: config-first workflow, workspace manifest

**V0 Build Artifacts**:
The first compiler outputs: `dist/docs.html` and `dist/docs.agent.json`.
_Avoid_: graph database, search index, RAG export, semantic diff artifact

**V0 Agent JSON**:
A flat compiled list of Knowledge Objects plus diagnostics, with relations preserved as stable object ID strings.
_Avoid_: graph-shaped artifact, embedded graph database, traversal API

**V0 Source Composition**:
The first compiler reads multiple `.adoc` files from a project path directly, without `@include`.
_Avoid_: include graph, remote includes, source-map-preserving composition

**V0 Block Structure**:
The first source grammar supports only top-level typed blocks.
_Avoid_: nested typed blocks, child object parsing

**Page Annotation**:
Optional metadata on a top-level heading, written as `@doc(id)` with a valid **Object ID**, used for page identity and grouping but not as a Knowledge Object.
_Avoid_: page object, source of truth

**V0 Relation Set**:
The first supported relationship fields between Knowledge Objects: `depends_on`, `supersedes`, and `related_to`.
_Avoid_: full graph relation model, graph traversal

**Tracer-Bullet Milestone**:
A small vertical slice that starts with `.adoc` input and ends with runnable CLI behavior, HTML output, agent JSON output, diagnostics, fixtures, and documentation.
_Avoid_: horizontal layer milestone, infrastructure-only phase

**V0 Implementation Stack**:
Rust for the initial `adoc` CLI, parser, validator, compiler, HTML renderer, and agent JSON emitter.
_Avoid_: TypeScript-first compiler, web-first implementation

**V0 Rust Workspace**:
A Cargo workspace with `crates/adoc-cli` for command-line concerns and `crates/adoc-core` for parsing, validation, diagnostics, rendering, and artifact emission.
_Avoid_: single CLI-only crate, over-split compiler crates

**V0 Parser Architecture**:
A structured hand-written, line-oriented parser with explicit source files, line indexes, spans, blocks, parse functions, and diagnostics.
_Avoid_: parser generator first, ad hoc string hacking

**V0 Core API**:
One high-level `compile_workspace()` entry point in `adoc-core`, backed by internal parser, validator, renderer, and artifact modules.
_Avoid_: public low-level compiler module APIs too early

**V0 Design Contract**:
A short implementation design document that fixes the initial Rust module boundaries, core API shape, diagnostic shape, AST sketch, and artifact contracts before scaffolding.
_Avoid_: second PRD, implementation without a contract

**Object ID**:
A stable lowercase dot-separated identifier with at least two kebab-case segments, used to cite and relate Knowledge Objects. Lives in code as the `ObjectId` newtype in `adoc-core`; a page-level Object ID is the `PageId` wrapper.
_Avoid_: UUID-only ID, heading slug, arbitrary string

**Diagnostic Code**:
A grouped semantic identifier for a compiler diagnostic, such as `parse.raw_html` or `schema.missing_field`. Lives in code as the `DiagnosticCode` enum in `adoc-core`; emission sites accept the typed value rather than a free-form string.
_Avoid_: numeric-only code, unstable message matching

**Validation Rule**:
One strict-mode check that produces diagnostics from a parsed page (e.g. `RawHtmlForbidden`, `UnsafeLinkForbidden`). Implemented via the `ValidationRule` trait in `adoc-core`, run after parsing as a separate pass per ADR-0007.
_Avoid_: parser-side check, schema linter

**Internal Port**:
A `pub(crate)` trait in `adoc-core` that decouples `compile_workspace`'s orchestration from a specific adapter — today `SourceProvider`, `Renderer`, and `ArtifactWriter`. Internal-only per ADR-0005; promoted to `pub` only when a concrete external consumer needs it. See ADR-0006.
_Avoid_: public plug-in API, dynamic adapter registry

**Build Output Directory**:
The directory passed to `adoc build --out`; the CLI creates it when missing and fails if the path exists as a file.
_Avoid_: manual pre-created output directory

## Relationships

- **AgentDoc Source** contains prose and typed blocks that compile into **Knowledge Objects**.
- A **Knowledge Object** may appear in human-rendered docs and in an **Agent-Facing Artifact**.
- The **Local CLI** compiles **AgentDoc Source** into **Agent-Facing Artifacts** and human-readable outputs.
- The first product milestone is centered on the **Local CLI**, not a collaborative web app.
- **Native Authoring** comes before Markdown migration in the roadmap.
- The **Core Object Set** is the first schema target for the compiler and validator.
- A **Verified Claim** must be supported in the first compiler slice.
- A **Verified Claim** can use **V0 Evidence Fields**; richer evidence types come later.
- The **CLI Command** is `adoc`, while the product name remains **AgentDoc**.
- **Strict Mode** is the only v0 validation mode; compatibility mode arrives with Markdown migration.
- The **V0 CLI Commands** are enough to validate source files and compile the first human and agent outputs.
- **V0 Defaults** avoid config files until modes, schemas, ignores, CI policy, or output presets need configuration.
- **V0 Build Artifacts** prove that the same **AgentDoc Source** can serve humans and agents.
- **V0 Agent JSON** is flat; graph-shaped artifacts are deferred.
- **V0 Source Composition** does not support includes; composition is by scanning files.
- **V0 Block Structure** keeps typed blocks top-level only.
- **Page Annotation** is optional in v0; missing page identity can be derived from the file path.
- **V0 Relation Set** references must resolve to existing Knowledge Object IDs and are preserved in **V0 Agent JSON** as ID arrays.
- Roadmap milestones should be **Tracer-Bullet Milestones**, not horizontal implementation layers.
- **V0 Implementation Stack** treats AgentDoc as compiler infrastructure first; future editor and web surfaces consume compiled artifacts.
- **V0 Rust Workspace** keeps CLI behavior separate from reusable compiler behavior.
- **V0 Parser Architecture** keeps diagnostics and source spans product-specific while leaving room to replace parser internals later.
- **V0 Core API** keeps the public core contract small; lower-level APIs can be exposed when LSP, web preview, semantic diff, or other integrations need them.
- **V0 Design Contract** guides scaffolding without replacing the roadmap or PRD.
- **Object ID** values are validated in v0 and form the citation target for humans and agents.
- **Diagnostic Code** values are semantic in v0; numeric aliases are deferred.
- **Validation Rule** runs after parsing; the parser emits only structural diagnostics, while semantic checks (raw HTML, unsafe link schemes) are validation rules.
- **Workspace Rule** is a validation rule that operates on the whole **WorkspaceAst** aggregate rather than a single page; future cross-page invariants (e.g. duplicate **Object IDs**, broken link targets) land as workspace rules without changing the orchestrator.
- An **Internal Port** stays `pub(crate)` until a concrete external consumer (LSP, web preview, semantic diff) needs it.
- **Build Output Directory** is created by the CLI when missing.

## Example dialogue

> **Dev:** "Should the first release include a web dashboard for browsing every object?"
> **Domain expert:** "No - the first release should prove that the **Local CLI** can compile **AgentDoc Source** into useful **Knowledge Objects** and **Agent-Facing Artifacts**."

## Flagged ambiguities

- "Initial product" could mean either a collaborative web app or a local developer tool - resolved: the first product surface is the **Local CLI** inside a Git repository.
- "First authoring workflow" could mean native AgentDoc files or Markdown import - resolved: start with **Native Authoring** using `.adoc` files; Markdown migration comes later.
- ".adoc" is commonly associated with AsciiDoc, but in this project it is the chosen extension for **AgentDoc Source**.
- "MVP object types" could mean every block type listed in the PRD - resolved: the first slice supports only the **Core Object Set**.
- "Verified lifecycle support" could mean all object types get full lifecycle enforcement immediately - resolved: v0 supports **Verified Claim** first.
- "Evidence" could mean the full PRD evidence model - resolved: v0 evidence is limited to `source`, `test`, and `reviewed_by`; commits, PRs, issues, external URLs, metrics, audit records, and scoring are deferred.
- "AgentDoc CLI" could imply the executable is `agentdoc` - resolved: the **CLI Command** is `adoc`.
- "Validation modes" could mean strict and compatibility both ship in the first slice - resolved: v0 supports **Strict Mode** only.
- "Initial CLI" could include every command named in the PRD - resolved: v0 supports only the **V0 CLI Commands**.
- "Project setup" could imply `adoc init` in v0 - resolved: users create `.adoc` files manually until initializer behavior is worth standardizing.
- "Project configuration" could imply an `agentdoc` or `adoc` config file in v0 - resolved: no config file in v0.
- "Build output" could include every artifact named in the PRD - resolved: v0 emits only the **V0 Build Artifacts**.
- "Agent JSON" could imply a graph-shaped export - resolved: **V0 Agent JSON** is a flat object list with diagnostics.
- "Source composition" could imply `@include` support from the PRD - resolved: v0 has no includes and scans `.adoc` files directly.
- "Typed block syntax" could include nested blocks from the PRD - resolved: **V0 Block Structure** allows only top-level typed blocks.
- "Page annotation" could imply pages are first-class knowledge objects - resolved: **Page Annotation** is metadata only in v0.
- "Relations" could mean the full PRD graph model - resolved: v0 supports only the **V0 Relation Set**.
- "Milestone" could mean a horizontal subsystem like parser, renderer, or storage - resolved: project milestones should be **Tracer-Bullet Milestones**.
- "Implementation stack" could mean TypeScript for faster early iteration - resolved: v0 starts with **V0 Implementation Stack** in Rust.
- "Rust project layout" could mean a single CLI crate - resolved: v0 starts with the **V0 Rust Workspace**.
- "Parser architecture" could mean choosing a generator or combinator library first - resolved: v0 starts with **V0 Parser Architecture**.
- "`adoc-core` API" could mean exposing parser, validator, and renderer APIs immediately - resolved: v0 exposes **V0 Core API** first and keeps lower-level modules internal.
- "Design pass" could mean writing another large product document - resolved: create a compact **V0 Design Contract** before implementation.
- "Object ID grammar" could mean arbitrary unique strings - resolved: use strict **Object ID** grammar.
- "Diagnostic code format" could mean compact numeric codes like `ADOC001` - resolved: use semantic **Diagnostic Code** values in v0.
- "Build output behavior" could require users to create `dist` manually - resolved: create the **Build Output Directory** when missing.
