# V0 Design

This document is the implementation contract for the first AgentDoc Rust scaffold. It is deliberately smaller than the PRD and roadmap: it fixes enough structure to start coding without pretending that every later product question is settled.

## Goals

- Implement a local Rust CLI named `adoc`.
- Keep `adoc-cli` thin and put product semantics in `adoc-core`.
- Expose one high-level `compile_workspace()` API from `adoc-core`.
- Parse native `.adoc` files in strict mode.
- Produce useful diagnostics with file, line, column, severity, code, and message.
- Emit `dist/docs.html` and `dist/docs.agent.json`.
- Keep parser, validator, renderer, and artifact modules internal until another consumer needs them.

## Non-Goals

- No config file.
- No `adoc init`.
- No compatibility mode.
- No Markdown migration.
- No includes.
- No nested typed blocks.
- No graph artifact.
- No search or explain command.
- No public parser, validator, or renderer API.

## Workspace Layout

```text
Cargo.toml
crates/
  adoc-cli/
    Cargo.toml
    src/
      main.rs
  adoc-core/
    Cargo.toml
    src/
      lib.rs
      compile.rs
      source.rs
      diagnostic.rs
      ast.rs
      parser.rs
      validate.rs
      render/
        mod.rs
        html.rs
      artifact/
        mod.rs
        agent_json.rs
```

Guidance:

- `adoc-cli` owns argument parsing, terminal formatting, file output, and exit codes.
- `adoc-core` owns source loading, parsing, validation, diagnostics, rendering, and artifact data.
- `adoc-core` may organize internals freely, but `compile_workspace()` is the only public workflow API in V0.

## Public Core API

Initial API sketch:

```rust
pub fn compile_workspace(input: CompileInput) -> CompileResult;

pub struct CompileInput {
    /// One `.adoc` file or a directory scanned recursively for `.adoc` files.
    pub root: std::path::PathBuf,
}

pub struct CompileResult {
    pub diagnostics: Vec<Diagnostic>,
    pub artifacts: Option<BuildArtifacts>,
}

pub struct BuildArtifacts {
    pub html: String,
    pub agent_json: AgentJsonDocument,
}
```

Rules:

- `compile_workspace()` scans `.adoc` files under `root`; if `root` is one file, it compiles that file.
- `compile_workspace()` does not read config because V0 has no config.
- `compile_workspace()` returns diagnostics for all files it can inspect.
- `artifacts` are present only when there are no error diagnostics.
- The CLI serializes `BuildArtifacts` to `dist/docs.html` and `dist/docs.agent.json`.

## CLI Contract

```bash
adoc check <path>
adoc build <path> --out dist
```

`adoc check`:

- Calls `compile_workspace()`.
- Prints diagnostics and a summary.
- Exits `0` when no error diagnostics exist.
- Exits `1` when any error diagnostic exists.

`adoc build`:

- Calls `compile_workspace()`.
- Prints diagnostics and a summary.
- Creates the output directory when it does not exist.
- Fails with a clear diagnostic if the output path already exists as a file.
- Writes `docs.html` and `docs.agent.json` only when no error diagnostics exist.
- Exits `0` after successful writes.
- Exits `1` without writing artifacts when errors exist.

## Source Model

Core source primitives:

```rust
pub struct SourceFile {
    pub path: std::path::PathBuf,
    pub text: String,
    pub line_index: LineIndex,
}

pub struct LineIndex {
    // Maps byte offsets to one-based line and column positions.
}

pub struct SourceSpan {
    pub file: std::path::PathBuf,
    pub start: SourcePosition,
    pub end: SourcePosition,
}

pub struct SourcePosition {
    pub line: u32,
    pub column: u32,
    pub offset: u32,
}
```

Guidance:

- Store byte offsets internally and convert to line/column through `LineIndex`.
- Report one-based lines and columns to users.
- Every parsed block should carry a `SourceSpan`.

## Diagnostics

Diagnostic shape:

```rust
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub severity: Severity,
    pub message: String,
    pub span: Option<SourceSpan>,
    pub object_id: Option<String>,
    pub help: Option<String>,
}

pub enum Severity {
    Error,
    Warning,
    Info,
}

pub enum DiagnosticCode {
    ParseRawHtml,
    ParseUnsafeLink,
    ParseUnclosedFence,
    ParseMalformedPageAnnotation,
    ParseNestedTypedBlock,
    ParseMalformedField,
    ParseMalformedOpenFence,
    SchemaUnknownKind,
    SchemaMissingField,
    SchemaDuplicateField,
    SchemaInvalidStatus,
    ClaimVerifiedMissingEvidence,
    ClaimStatusCasing,
    IdDuplicate,
    IdInvalid,
    RefBroken,
    IoUnreadableFile,
    IoUnsupportedSourceExtension,
}
```

Diagnostic guidance:

- Use stable diagnostic codes from the start.
- Use grouped semantic diagnostic codes, not numeric codes, in V0.
- Prefer fix-oriented messages.
- Include `object_id` when a diagnostic belongs to a Knowledge Object.
- V0 includes diagnostics for raw HTML, unsafe links, unreadable files, unsupported single-file source extensions, malformed fences, malformed page annotations, malformed typed blocks, duplicate IDs, missing fields, invalid verified claims, unknown object types, invalid Object IDs, and broken references.

Initial diagnostic code examples:

- `parse.raw_html`
- `parse.unsafe_link`
- `parse.unclosed_fence`
- `parse.malformed_page_annotation`
- `parse.nested_typed_block`
- `schema.unknown_kind`
- `schema.missing_field`
- `id.invalid`
- `id.duplicate`
- `ref.broken`
- `claim.verified_missing_evidence`
- `io.unreadable_file`
- `io.unsupported_source_extension`
- `io.output_not_directory`

## AST Sketch

V0 starts with page and block ASTs:

```rust
pub struct WorkspaceAst {
    pub pages: Vec<PageAst>,
}

pub struct PageAst {
    pub id: String,
    pub title: Option<String>,
    pub source_path: std::path::PathBuf,
    pub blocks: Vec<BlockAst>,
}

pub enum BlockAst {
    Heading(HeadingAst),
    Paragraph(ParagraphAst),
    List(ListAst),
    CodeBlock(CodeBlockAst),
    TypedBlock(TypedBlockAst),
}
```

Typed blocks are added in V0.2:

```rust
pub struct TypedBlockAst {
    pub kind: String,
    pub id: String,
    pub fields: Vec<FieldAst>,
    pub body: Body,
    pub span: SourceSpan,
}

pub struct Body {
    pub inlines: Vec<InlineSegment>,
}

pub struct FieldAst {
    pub key: String,
    pub value: FieldValueAst,
    pub span: SourceSpan,
}

pub enum FieldValueAst {
    Scalar(String),
    List(Vec<String>),
}
```

Guidance:

- V0 typed block bodies are inline-aware `Body` values. The parser preserves
  each body line's source span, Knowledge Object construction parses body text
  with `parse_inlines`, and validation resolves `[[object.id]]` references
  before rendering.
- Nested typed blocks are invalid in V0.
- Agent JSON projects body inlines back to source text (`[[object.id]]`,
  emphasis, strong, links, and code markers) so agents can cite Object IDs
  without a graph artifact.

## Parser Strategy

Use a structured hand-written line-oriented parser.

Internal parse functions should stay small:

- `parse_workspace`
- `parse_file`
- `parse_heading`
- `parse_paragraph`
- `parse_list`
- `parse_code_block`
- `parse_page_annotation`
- `parse_typed_block`
- `parse_fields`
- `parse_object_reference`

Guidance:

- Prefer explicit state machines for fenced code and typed blocks.
- Do not use ad hoc string replacements for syntax recognition.
- Do not choose a parser generator in V0.
- Keep parser internals replaceable behind AST and diagnostic types.

## Validation Model

Validation runs after parsing and before rendering.

V0 validation phases:

1. Source validation: raw HTML, malformed blocks, malformed annotations.
2. Object validation: object type, required fields, duplicate IDs.
3. Verified claim validation: `owner`, `verified_at`, and at least one V0 evidence field.
4. Reference validation: `[[object.id]]` and relation targets resolve.

The first supported object kinds are:

- `claim`
- `decision`
- `warning`
- `glossary`

The first supported relation fields are:

- `depends_on`
- `supersedes`
- `related_to`

Relation values are Object IDs in any supported Knowledge Object field region.
They can be scalar, comma-separated, or bracket-array values:

```adoc
::decision billing.new-policy
status: accepted
decided_by: architecture
supersedes: billing.old-policy
related_to: billing.credits, billing.ledger
depends_on: [billing.ledger, billing.policy]
--
Use the new billing policy.
::
```

The compiler trims ASCII whitespace around each segment, ignores a trailing
comma, deduplicates repeated targets while preserving first occurrence order,
and emits `id.invalid` for malformed or empty interior segments. Relation
targets that use valid Object ID grammar but do not resolve emit `ref.broken`.
Relation fields are removed from ordinary object metadata before rendering and
agent JSON emission.

The first evidence fields for verified claims are:

- `source`
- `test`
- `reviewed_by`

Verified claim contract:

- Only exact lowercase `status: verified` creates a Verified Claim.
- ASCII case variants such as `Verified` emit `claim.status_casing` and are treated as plain claims.
- Verified claims require non-empty `owner`, non-empty `verified_at`, and at least one non-empty evidence field.
- Missing `owner` or `verified_at` emits `schema.missing_field` with the claim `object_id`.
- Missing evidence emits `claim.verified_missing_evidence` with the claim `object_id`.
- `verified_at` date format, owner format, multi-value evidence, and evidence scoring are out of scope for V0.

## Object IDs

V0 object IDs use lowercase dot-separated kebab segments with at least two segments.

Examples:

- `billing.credits`
- `billing.credits.decrement-after-success`
- `auth.refresh-token.rotation`

Rules:

- Segment characters: lowercase ASCII letters, digits, and internal hyphens.
- Dot separates segments.
- At least two segments are required.
- Segments cannot be empty.
- Segments cannot start or end with a hyphen.
- Uppercase letters, underscores, slashes, spaces, colons, and UUID-only IDs are invalid.

Suggested validation regex:

```text
^[a-z0-9]+(?:-[a-z0-9]+)*(?:\.[a-z0-9]+(?:-[a-z0-9]+)*)+$
```

## HTML Artifact

`docs.html` is a single static HTML document.

Rules:

- Escape rendered user content.
- Use semantic HTML where practical.
- Include page headings and prose.
- Render Knowledge Objects as identifiable sections/cards once typed blocks exist.
- Render inline-aware Knowledge Object bodies through the shared inline renderer.
- Show kind, ID, status, owner, verification, evidence, and relations when present.
- Do not emit JavaScript in V0.

## Agent JSON Artifact

Initial shape:

```json
{
  "schema_version": "adoc.agent.v0",
  "pages": [],
  "objects": [],
  "diagnostics": []
}
```

Page record sketch:

```json
{
  "id": "billing.credits",
  "title": "Billing Credits",
  "source_path": "docs/billing.adoc"
}
```

Object record sketch:

```json
{
  "id": "billing.credits.decrement-after-success",
  "kind": "claim",
  "status": "verified",
  "body": "Credits are decremented only after generation completes successfully.",
  "page_id": "billing.credits",
  "source_span": {
    "path": "docs/billing.adoc",
    "line": 10,
    "column": 1
  },
  "fields": {
    "owner": "backend-platform",
    "verified_at": "2026-05-02",
    "source": "apps/backend/src/features/credits/consume.use-case.ts"
  },
  "relations": {
    "depends_on": [],
    "supersedes": [],
    "related_to": []
  }
}
```

Rules:

- Agent JSON remains flat in V0.
- Object `status` is the object's kind-primary normalized discriminant. For the
  current object set, this means claim status, decision status, and warning
  severity. V0 keeps these values in one slot rather than adding per-kind
  top-level discriminant fields.
- Addendum: object kinds without a kind-primary normalized discriminant omit
  `status` from Agent JSON rather than emitting `null` or an empty string.
  For glossary, an author-supplied `status` field remains ordinary optional
  metadata under `fields.status`; it is not promoted to top-level `status`.
- Relations are ID arrays, not embedded objects.
- Relation arrays preserve source order after duplicate target removal.
- Object `body` is a faithful source-text projection of the inline-aware body,
  preserving `[[object.id]]` markers rather than embedding a graph model.
- Diagnostics are included using the same codes and messages as CLI diagnostics.
- Schema versioning starts immediately, even if the shape is still pre-1.0.

## Tests and Fixtures

Fixture layout:

```text
fixtures/
  v0_1/
    prose_page.adoc
    raw_html.adoc
  v0_2/
    one_claim.adoc
    duplicate_id.adoc
```

Test guidance:

- Unit-test parser functions around spans and diagnostics.
- Snapshot-test rendered HTML where stable enough.
- Golden-test agent JSON.
- Include at least one positive and one negative fixture per tracer-bullet slice.

## Open Questions Before Scaffolding

None. The next step is scaffolding the Rust workspace.
