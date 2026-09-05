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

**Change Assessment**:
The deterministic `adoc.change_assessment.v0` result for one requested base,
unique comparison base, head snapshot, and evaluation date. It reports
structural validity, changed-path classification, implicated knowledge, and
review obligations without claiming semantic consistency.
_Avoid_: compliance verdict, semantic review, GitHub receipt

**PR Assessment Receipt**:
The Action-owned `adoc.pr_assessment_receipt.v0` artifact that binds one
validated Change Assessment to exact GitHub revisions, CI identity, verified
toolchain, Action policy, final conclusion, and optional-stage status. It
proves what CI assessed, not what a runtime agent used or acted upon.
_Avoid_: Agent Use Receipt, immutable audit ledger, proof of agent reliance

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
The first compiler outputs were `dist/docs.html` and a flat agent JSON artifact; the flat JSON artifact is now retired in favor of the **Graph Artifact**.
_Avoid_: graph database, search index, RAG export, semantic diff artifact

**Legacy Flat JSON Artifact**:
The retired `docs.agent.json` object-list artifact. It is no longer emitted, loaded, or part of the public `adoc-core` surface.
_Avoid_: new consumers, compatibility shims, treating it as the current read model

**V0 Source Composition**:
The first compiler reads multiple `.adoc` files from a project path directly, without `@include`.
_Avoid_: include graph, remote includes, source-map-preserving composition

**Physical Source Path**:
The canonical host-filesystem path used only to read source bytes. In a project-bound command it must resolve below both the discovered project root and configured documentation root; it is never serialized or hashed.
_Avoid_: artifact coordinate, page identity, absolute path fallback

**Identity Path**:
The documentation-root-relative source path used to derive path-based page IDs. It remains independent from the repository-relative coordinate published in artifacts.
_Avoid_: repository-qualified ID, physical checkout path, graph source coordinate

**Logical Source Path**:
The validated project-root-relative, slash-normalized source coordinate used in diagnostics, Graph Artifact spans, diffs, hashes, and future receipts. It is non-empty UTF-8 and rejects absolute paths, drive prefixes, backslashes, dot components, controls, and edge whitespace. Standalone check/build uses an invocation-root-relative coordinate but establishes no repository identity.
_Avoid_: host path, `<review>` prefix, permissive path normalization

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
A small vertical slice that starts with `.adoc` input and ends with runnable CLI behavior, HTML output, graph JSON output, diagnostics, fixtures, and documentation.
_Avoid_: horizontal layer milestone, infrastructure-only phase

**V10 Planning Milestone**:
A deliberate second sense of "milestone" for the V10 planning layer: a phase-sized grouping of `E*` slices (E0–E9) in `docs/roadmap/v10/MILESTONES.md`, anchored to release stages. The **Tracer-Bullet Milestone** vertical-cut discipline is preserved inside each grouping at the `E*.Tn` tracer-bullet level.
_Avoid_: using this sense outside the V10 planning documents, phase groupings without tracer-bullet decomposition

**V0 Implementation Stack**:
Rust for the initial `adoc` CLI, parser, validator, compiler, HTML renderer, and artifact emitters.
_Avoid_: TypeScript-first compiler, web-first implementation

**V0 Rust Workspace**:
A Cargo workspace with `crates/adoc-cli` for command-line concerns and `crates/adoc-core` for parsing, validation, diagnostics, rendering, and artifact emission.
_Avoid_: single CLI-only crate, over-split compiler crates

**Local Workflow Layer**:
The protocol-free local application adapter in `crates/adoc-local`. It owns AgentDoc project config discovery, default path resolution, local command orchestration, filesystem writes for `init`/`build`, and command outcome shapes shared by CLI and MCP.
_Avoid_: duplicating config/build/check orchestration in each driving adapter, putting terminal presentation in the core

**V0 Parser Architecture**:
A structured hand-written, line-oriented parser with explicit source files, line indexes, spans, blocks, parse functions, and diagnostics.
_Avoid_: parser generator first, ad hoc string hacking

**V0 Core API**:
One high-level `compile_workspace()` entry point in `adoc-core`, backed by internal parser, validator, renderer, and artifact modules.
_Avoid_: public low-level compiler module APIs too early

**Public Core Surface**:
The narrow `adoc-core` API exported for CLI callers and future local integrations: compile/build entry points, graph/retrieval session loaders, query functions, query/result/envelope records, diagnostics, and mode/relation/direction enums. Graph Artifact and Search Artifact DTO structs stay internal; serialized artifact files and retrieval envelopes are the contract.
_Avoid_: public graph DTO construction, public search DTO construction, renderer-shaped read models

**MCP Agent Gateway**:
The local `rmcp` server in `crates/adoc-mcp` that exposes AgentDoc CLI-equivalent tools to agents. It is a driving adapter over `adoc-local` and `adoc-core`, uses a project-root path sandbox, and returns the same stable retrieval, graph traversal, and patch-check envelopes where those contracts already exist. Since V6.4 (ADR-0037) it also applies validated patches through the same sandbox — but only under the explicit `mcp: { patch_apply: enabled }` project opt-in; the always-registered `adoc_patch_apply` tool refuses by default.
_Avoid_: hosted review state, ungated patch application, default-on source writes, graph/search DTO exposure

**Agent Usage Contract**:
The V2.2 stable local contract that tells agents how to inspect project readiness, retrieve and cite knowledge, and validate patch proposals through MCP without guessing tool order or private artifact shapes.
_Avoid_: implicit agent habits, shell wrapper convention, unversioned prompt drift

**Agent Guidance Resource**:
A versioned MCP resource under `adoc://agent/v0/...` that exposes canonical Markdown guidance or JSON Schema documentation for the Agent Usage Contract.
_Avoid_: duplicated prompt strings, docs hidden from MCP discovery, private README scraping

**Agent Workflow Prompt**:
A versioned MCP prompt, with a pinned unversioned v0 alias, that packages a repeatable AgentDoc workflow such as answer-with-citations, propose-patch, inspect-project-status, or billing-pilot dogfood.
_Avoid_: floating latest prompt aliases, ad hoc per-agent instructions

**Project Status Report**:
The `adoc.project.status.v0` envelope returned by `adoc_project_status`. It reports config discovery, resolved paths, refresh diagnostics, artifact load status, readable graph/search schema versions, cheap graph object counts, and readiness booleans for retrieval, semantic search, and patch validation.
_Avoid_: probing random files, assuming artifacts exist, mutating source during inspection

**V0 Design Contract**:
A short implementation design document that fixes the initial Rust module boundaries, core API shape, diagnostic shape, AST sketch, and artifact contracts before scaffolding.
_Avoid_: second PRD, implementation without a contract

**Object ID**:
A stable lowercase dot-separated identifier with at least two kebab-case segments, used to cite and relate Knowledge Objects. Lives in code as the `ObjectId` newtype in `adoc-core`; a page-level Object ID is the `PageId` wrapper.
_Avoid_: UUID-only ID, heading slug, arbitrary string

**Managed Object**:
A Knowledge Object under managed Cloud governance, carrying workspace-qualified canonical identity distinct from its human-readable **Object ID**, its immutable managed version ID, and its Source Binding (ADR-0057 invariant 1; EXECUTION-MAP E1.2). Matching IDs, titles, hashes, or semantic similarity across sources never auto-merge Managed Objects — collisions produce reconciliation candidates only.
_Avoid_: global object namespace, auto-merged identity, cross-workspace object

**Workspace Canonical Identity**:
The workspace-qualified canonical identity of a **Managed Object** (K6 layer 1; `WorkspaceCanonicalIdentity` in `adoc-core`): a Cloud workspace id plus an opaque canonical id, stored separately from the human-readable **Object ID**. Equality includes the workspace, so identity is never linkable across workspaces by an unqualified Object ID.
_Avoid_: Object ID as canonical key, cross-workspace identity linkage, content-derived canonical id

**Managed Version ID**:
The unique immutable identifier every managed candidate/active version receives (K6 layer 3; `ManagedVersionId` in `adoc-core`). A semantic content change mints a new one; a state-only transition or an unchanged re-observation never does (RT-04). Opaque and never reused.
_Avoid_: content hash as version id, mutable version record, version id reuse

**Source Assertion Identity**:
The identity of one immutable Source Assertion extracted from a Source Artifact (K6 layer 4, K7; `SourceAssertionIdentity` in `adoc-core`). Only the identity layer exists today — the Source Record/Assertion store lands in E4.1. Carried per managed version so reconciliation preserves original provenance.
_Avoid_: Source Binding synonym, assertion as Knowledge Object, mutable assertion

**Managed Repository Record**:
One imported repository inside a Cloud workspace (`ManagedRepositoryRecord` in `adoc-core`) — the reserved binding slot, keyed by a workspace-minted slot handle (`RepositorySlotId`) and recording the Graph Artifact's `repository_identity` — `{kind, config_path}` or explicit `null`, required since v5 (ADR-0049). Import routes by slot handle because the graph identity names the producing invocation family, not a workspace-unique repository — every project-bound build emits the constant `{local_project, "agentdoc.config.yaml"}`, degenerate across physical repositories; identity-equality lookup remains only as the single-repo convenience and fails closed when ambiguous. The slot can be reserved before the first artifact arrives (V10.3.2), and its Object IDs stay repository-local — the same ID in another slot is a different **Managed Object**, never silently merged.
_Avoid_: global repository namespace, path-string repository key, repository identity as slot router, cross-repository ID merge

**Reconciliation Candidate**:
The typed record (`adoc.reconciliation_candidate.v0`, registered planned) emitted when an imported **Object ID** collides with a **Managed Object** in another repository of the same workspace. Both objects stay distinct; the candidate names both parties by **Workspace Canonical Identity**, repository identity, latest **Managed Version ID**, and content hash. Deciding a candidate (keep distinct, link, supersede, merge) is a governed E1.3 action; matching hashes, titles, or similarity never produce one.
_Avoid_: auto-merge, duplicate detector, similarity-based unification

**Governance Event**:
An append-only, principal-bound record of one governed transition, carrying the exact policy version and the exact object versions it decides (MILESTONES §E1.3; RT-03/RT-04). The Cloud wire family is `adoc.governance_event.v0` (E4.2); its first concrete payload is the **Reconciliation Decision**. A Governance Event can never be authored by model output alone — a principal is required at the type level.
_Avoid_: model-created authority, mutable audit row, timestamped decision payload

**Reconciliation Decision**:
The typed record (`adoc.reconciliation_decision.v0`, registered planned; `ReconciliationDecision` in `adoc-core`) deciding a **Reconciliation Candidate** pair: a closed verb — keep distinct, link/alias, supersede, or merge/re-home — bound to both parties' **Workspace Canonical Identity** and exact **Managed Version ID**, a principal, and a policy version. The latest-version binding is a record-time admission gate: content that changed after adjudication leaves the recorded decision standing (re-review is post-V1, connector re-observation territory). Recording a decision is the only path that transitions a pair, appends to the workspace's decision log, and never rewrites or drops any managed version, Source Binding, or Source Assertion; replaying the log over the same import history yields byte-identical reconciliation state.
_Avoid_: auto-merge, unprincipaled decision, decision as import side effect, destructive merge

**Managed State Event**:
The append-only record (`ManagedStateEvent` in `adoc-core`, E1.4) of one managed state change over one immutable content version, named by **Workspace Canonical Identity** plus exact **Managed Version ID** and carrying its event family, emitter, and policy version. A state-only transition never alters `content_hash` and never mints a content version (ADR-0057 invariant 2; RT-04). Wall-clock-free: event order (the ordinal) is the only notion of time, so replay is deterministic.
_Avoid_: state row update, mutable status field, timestamped domain record, inferred transition

**State Dimension**:
One of the six separate managed state axes of KNOWLEDGE-MODEL §K4 — governance, verification, effectivity, freshness, integrity, per-connector synchronization — each a closed vocabulary modeled as its own enum in `adoc-core` (D07/D15). Dimensions are never conflated: approval is not verification, and no collapsed status field exists.
_Avoid_: collapsed status field, approval implies verified, cross-dimension enum

**Lifecycle Mapping**:
The versioned `adoc.lifecycle_mapping.v0` contract (`LifecycleMappingContract` in `adoc-core`, E1.5) assigning managed meaning to each kind's released flat `.adoc` status vocabulary — a governance + effectivity target per authored word, with a machine-readable per-dimension loss declaration. Versions resolve exact-match (`schema.unsupported_version` otherwise); a rule change requires a version bump; standalone flat semantics never change (K1). Mapping alone never establishes authority, and no mapping rule can name a verification state.
_Avoid_: status translation table that grants authority, implicit version fallback, mapping verified to verification

**Attestation**:
The typed input (`MappingAttestation`, E1.5) that authorizes a Lifecycle Mapping application to land its mapped target as **Managed State Event**s: a migration attestation, a source-control attestation, or a Cloud Governance Event (K5). Authority comes only from the presence of the typed value — its default absence cannot be forged — and without one the mapped target is advisory while applied governance stays `proposed`.
_Avoid_: authored status as authority, implicit attestation, approval token in source text

**Projection**:
The export direction of the Lifecycle Mapping contract (E1.5): a versioned policy inside the same `adoc.lifecycle_mapping.v0` envelope rendering managed multi-dimension state back to a flat `.adoc` status word — total per kind (ordered rules over an unconditional fallback) and explicit about loss. Every application returns a machine-readable loss report naming each recorded dimension the word cannot carry back through the contract's own import mapping; the word `verified` renders only from recorded `verification: verified`, never from approval (K5/K10).
_Avoid_: lossless export, silent narrowing, approval rendered as verified

**Audit Sink**:
The internal port (`AuditSink` in `adoc-core`, E1.4) where the managed state event store persists one audit record per transition, written before the event commits. A sink failure fails the owning operation with `audit.persistence_failed` — an unaudited committed transition cannot exist, and nothing silently succeeds (V10.4.6). The CI audit coverage guard diffs the state machines' transition sets against the audit emitter registry so every vocabulary state stays consciously wired. Distinct from the planned gate-level `gate.audit_persistence_failed` (E5.3).
_Avoid_: best-effort audit, audit-optional append, tamper-resistant ledger claim

**Retention Floor**:
The policy minimum span of most-recent managed state records that no API may delete (V10.4.1, K9; `RetentionFloor` in `adoc-core`, expressed in recorded-event-order terms — no wall-clock exists in domain records). Append-only is enforced at the store layer: an in-place update attempt is `governance.record_conflict`, a sweep reaching into the protected span is `store.retention_floor_violation`, and corrections are new records referencing the corrected one. Deletion workflows above the floor arrive in E6.6.
_Avoid_: handler-enforced append-only, record rewrite, silent sweep

**Digest Chain**:
The exact canonical bytes of every **Managed State Event** plus a `sha256:`-prefixed digest chaining each record to its predecessor, both stored at write time by the event store and carried on the audit record (E1.4.T5). Later export needs no extra data: the stored `(ordinal, bytes, digest)` triples alone reproduce every event and detect any reordering or alteration of the recorded prefix (detection of accidental corruption, not tamper evidence — nothing is anchored outside the store). All ten RT-04 event families — the six **State Dimensions** plus authorization-affecting source changes, declassification, migration, and deletion/tombstone — flow through the same chained append path.
_Avoid_: bytes re-derived at export, unchained audit rows, wall-clock timestamping

**Replay Posture**:
The recorded K9 answer to whether a derivation can be reproduced — `fully_replayable`, `source_access_required`, `intentionally_non_replayable`, or `no_longer_replayable_after_deletion` (registered closed vocabulary, E0.3.T4; `ReplayPosture` in `adoc-core` since E1.4). A digest-only record is never fully replayable; deleting retained evidence appends a deletion/tombstone event recording the posture it leaves behind, never rewriting governance history.
_Avoid_: implicit replayability, posture recomputed at read time, silent deletion

**Diagnostic Code**:
A grouped semantic identifier for a compiler diagnostic, such as `parse.raw_html` or `schema.missing_field`. Lives in code as the `DiagnosticCode` enum in `adoc-core`; emission sites accept the typed value rather than a free-form string.
_Avoid_: numeric-only code, unstable message matching

**Validation Rule**:
One strict-mode check that produces diagnostics from a parsed page (e.g. `RawHtmlForbidden`, `UnsafeLinkForbidden`). Implemented via the `ValidationRule` trait in `adoc-core`, run after parsing as a separate pass per ADR-0007.
_Avoid_: parser-side check, schema linter

**Internal Port**:
A `pub(crate)` trait in `adoc-core` that decouples application orchestration from a specific adapter — today `SourceProvider`, `ArtifactReader`, `ArtifactWriter`, and `EmbeddingProvider`. Internal-only per ADR-0005; promoted to `pub` only when a concrete external consumer needs it. See ADR-0006.
_Avoid_: public plug-in API, dynamic adapter registry

**Build Output Directory**:
The directory passed to `adoc build --out`; the CLI creates it when missing and fails if the path exists as a file.
_Avoid_: manual pre-created output directory

**V1 Local Retrieval**:
The first post-compiler milestone. Adds `adoc why`, `adoc graph`, and `adoc search` over compiled artifacts, ships per-Knowledge-Object embeddings as a first-class build output, and ranks results via a parameter-free hybrid of BM25 and cosine similarity.
_Avoid_: V1 hosted RAG service, V1 agent server, V1 graph database

**V1 Build Artifacts**:
The V1 compiler outputs: `dist/docs.html`, `dist/docs.graph.json`, and optionally `dist/docs.search.json`. `adoc-core` returns these as ready-to-write strings (`html`, `graph_json`, optional `search_json`); the CLI owns the file-write boundary. The graph artifact is the canonical read model for retrieval.
_Avoid_: SQLite graph artifact, RAG ndjson, separate diagnostics artifact

**Search Artifact**:
The V1 build output, `dist/docs.search.json`, with schema version `adoc.search.v2` since E1.1.T5 (ADR-0058; prose entries since V1.7.2, ADR-0040 — the v2 bump deliberately invalidates v1 embedding caches for the Graph Artifact v6 full re-embed). Carries one `{ id, entry_kind, content_hash, vector }` entry per Knowledge Object and per indexed prose block (`entry_kind: "knowledge_object" | "prose"`), a `model: { id, provider, dim }` header, and a `graph_artifact_hash` for drift detection. Code blocks and prose under a minimum token threshold are not embedded; prose cache reuse is keyed by content hash and model, never by order-derived block id. The serialized JSON shape is public; the Rust DTO used to build or read it is internal to `adoc-core`.
_Avoid_: per-chunk embedding store, vectors embedded in `docs.graph.json`, binary sidecar in V1

**Graph Artifact**:
The canonical local read model, `dist/docs.graph.json`, now with schema version `adoc.graph.v6` (ADR-0058). It is derived from validated AgentDoc Source and carries a required `repository_identity` member, page/prose/Knowledge Object nodes, and directed edges. Project-bound builds identify the local project through `agentdoc.config.yaml`; standalone builds emit an explicit `null`. Each Knowledge Object carries a `content_hash` over governed meaning only — placement (**Logical Source Path**, line/column, page, the **Source Binding** member) is serialized on the node but excluded from the hash, so equal semantics hash identically across checkouts, review snapshots, and position-only moves. It is data-only and contains no rendered HTML fields; the serialized JSON shape and JSON Schema are public, while the Rust DTO remains internal to `adoc-core`.
_Avoid_: graph database, SQLite-first graph storage, graph as authoring source of truth, presentation HTML inside graph JSON

**Base Hash**:
The `content_hash` value a patch declares for its target object. It is a `sha256:` hash over canonical JSON for the graph Knowledge Object node's governed meaning (ADR-0058): kind, lifecycle/status, body, authored classifications, fields, relations, and evidence declarations — never identity (the **Object ID** is its own K6 layer), never placement (page, source span), and never the **Source Binding**.
_Avoid_: source-file checksum, search embedding hash, approval token

**Source Binding**:
The exact source placement recorded per Knowledge Object node in the **Graph Artifact** (ADR-0058 §4): connector, source, optional connector-native revision, path-or-coordinate, anchor, and a source-revision digest. Serialized alongside the node and never part of `content_hash`, it exists for provenance, writeback, patch safety, and stale-source detection — `adoc patch --apply` refuses with `patch.source_binding_stale` when the target file's bytes no longer reproduce the recorded digest. For the local filesystem connector (`local_fs`) the source is the containing page, the path is the **Logical Source Path**, the anchor is the **Object ID**, and the digest is the sha256 of the source page bytes.
_Avoid_: hash-bearing placement, source-file checksum inside `content_hash`, connector transport metadata in object hashes

**Visibility**:
The authored per-object visibility classification on any Knowledge Object: the closed set `public | internal | restricted` (ADR-0058 §3). An authored value is typed, serialized as a dedicated node member, and hash-included, so a classification change changes `content_hash`; absence means `public` and is neither serialized nor hashed. An invalid value fails closed with `schema.visibility_invalid` — never a silent default. The optional `field_visibility` map (`field=visibility` entries) carries per-field classifications, carriage only until enforcement lands (E6/V10.6).
_Avoid_: silent public default, open visibility vocabulary, visibility-based access enforcement before E6

**Agent Patch**:
A single-operation JSON proposal with schema version `adoc.patch.v0`, validated by `adoc patch --check`. It expresses patch intent validated against compiled artifacts; since V6.4 (ADR-0036) a validated patch can be **applied** to AgentDoc Source via `adoc patch --apply` or the gated MCP `adoc_patch_apply`, emitting `adoc.patch.apply.v0` — a formatting-preserving span splice on the working tree, never an artifact edit. It does not approve knowledge or create hosted review state.
_Avoid_: source rewrite format, migration script, approval record

**Agent Contract Schema**:
A versioned JSON Schema resource under `docs/agent/v0/schema/` that documents a stable agent wire contract, such as retrieval envelopes, graph traversal envelopes, patch input, patch check output, project status, and MCP command envelopes. These schemas are authored contracts and are tested against representative serialized values.
_Avoid_: generated-only schema, undocumented DTO dump, untested prompt prose

**Artifact Readiness**:
The validated ability of Graph and Search artifacts to support retrieval, semantic search, or patch validation. Readiness is inspected in `adoc-core` using artifact readers, graph index validation, model-header checks, graph-hash drift detection, and diagnostics; `adoc-local` only orchestrates Project Status around those inspectors.
_Avoid_: raw JSON sniffing in adapters, existence-only readiness, assuming stale artifacts are usable

**Patch Validation**:
Artifact-only validation of one **Agent Patch** against a **Graph Artifact**. It checks Object IDs, required reasons, operation-specific fields, target existence, **Base Hash** freshness, relation targets, create placement hints, lifecycle intent, and proof obligations, then emits an `adoc.patch.check.v0` review report.
_Avoid_: applying edits, mutating graph JSON, bypassing source review

**Proof Obligation**:
A review-time requirement emitted when a patch touches knowledge that needs renewed evidence, especially Verified Claims. It records what evidence or follow-up must be resolved before humans or agents treat the proposed change as approved knowledge. Since E1.6 the concept also exists as a typed, stateful, stage-bound record (`adoc.proof_obligation.v0`, `domain/obligation_record.rs`): state `open|satisfied|waived|failed|expired`, `required_at` naming one of the six K8 stages, bound to the exact Managed Object version — both vocabularies registered and closed. Whether an obligation is informational or blocking at a stage/risk/action is classification-policy data, never code. The stateless wire shape embedded in `adoc.review.v0`/`adoc.patch.check.v0` is unchanged; a bridge constructor lifts it into the stage-bound record.
_Avoid_: validation error by default, approval, automated trust upgrade, hard-coded stage or role blocking

**Obligation Waiver**:
A permission-controlled discharge record for one stage-bound **Proof Obligation** (`domain/obligation_record.rs`, part of the `adoc.proof_obligation.v0` contract): bound at the type level to the exact obligation, Managed Object version, principal, and policy version, with a non-blank justification, and time-bounded where appropriate by an explicit event-ordinal expiry — never a wall clock. Waiver application appends to the obligation ledger only and touches no K4 dimension, so a waiver cannot convert `unverified` to `verified`; an expired waiver reopens its obligation as blocking, and a stale waiver never authorizes.
_Avoid_: waiver as verification, silent unverified-to-verified conversion, wall-clock expiry, blanket or unbound waivers

**Validation Runtime**:
The one authoritative execution surface for AgentDoc-domain validation (SEMANTICS §S6; `application/validation_runtime.rs`): a released, checksum-pinned `adoc` binary invoked identically from the local CLI, CI harness, and Cloud driver. Every run returns a **Validation Receipt**. Cloud TypeScript preflights transport concerns only — auth, binding, limits, JSON/version recognition, claimed digest, replay — and never duplicates parsing, semantic hashing, lifecycle/evidence/reference rules, obligations, or citation validation; a Cloud-side reimplementation of domain rules is a defect class, not an optimization.
_Avoid_: Cloud-side domain validation, JSON Schema as domain authority, unpinned runtime invocation

**Validation Receipt**:
The digest-bound envelope the **Validation Runtime** returns (`adoc check --receipt`): `adoc.validation_receipt.v0` normally, or its v1 successor when `--source-invocation` binds Cloud source evidence. Both carry the exact runtime version plus harness-attested binary digest, every consumed input digest by Logical Source Path, context digests, consumed contract versions (graph `adoc.graph.v6`, plus `adoc.semantic_context.v0` exactly when supplied), the closed `pass` / `fail` result, and a sha256 digest of the canonically serialized typed diagnostics; v1 additionally permits the exact Source Invocation digest. Deterministic by contract — stable ordering, no wall-clock timestamps, lifecycle pinned to the explicit `evaluation_date` input — so the same domain input yields byte-identical receipts across local, CI, and Cloud execution. Constructible only by the runtime: unvalidated receipt JSON has no core representation.
_Avoid_: receipt with wall-clock timestamps, self-hashed binary digest, receipt deserialized into the typed envelope, receipt as approval

**Semantic Context**:
The deterministic `adoc.semantic_context.v0` envelope supplied as inert data to a semantic executor: exact subject/source/base/head revisions, assessment and Graph Artifact or managed-revision basis digests, closed citation handles, completeness facts, and its canonical context digest. It records what authorized context was supplied; it does not retrieve context, authorize access, or grant model output authority.
_Avoid_: prompt, RAG dump, open citation URL, context reconstructed by the validator, semantic assessment

**Proposal Record**:
The canonical `adoc.proposal.v0` record (`ProposalRecord` in `adoc-core`, E5.1, ADR-0062) binding a validated set of **Agent Patch** documents to exact base/head revisions, change-request identity, deterministic assessment digest, **Semantic Context** digest, **Semantic Assessment** digest, and the exact `content_hash` of every edited **Knowledge Object**. Its identity is the proposal-set digest over the exact patch bytes ordered by patch digest (ADR-0062 §2, superseding ADR-0053 §8 in part); an edit mints a superseding record naming the prior digest. E5.3 may add per-finding `no_change_required` references to server-minted human-acceptance receipts without changing that patch-only identity; Cloud exact-matches the referenced receipt before using it and stores this evidence separately from proposal-set deduplication. Links are identifiers and digests only, so Git-delivered and API-submitted proposals are byte-identical; `adoc proposal-record` is the shared producer adapter and Cloud consumes it as the `agentdoc.cloud.proposal_command.v0` payload.
_Avoid_: proposal bundle schema, branch-bound proposal, model-approved proposal, candidate activation, model-authored no-change authority

**Gate Result**:
The canonical `adoc.gate_result.v0` conclusion produced by Cloud's E5.3 evaluator from validated typed facts and digests only. It binds the exact Git head, gate-policy version, canonical input-digest set, configured/effective mode, pass/block result, and closed `gate.*` reasons; it carries no model prose or wall-clock data, so equal facts yield equal bytes. AgentDoc owns the contract and Cloud owns evaluation and persistence.
_Avoid_: client-authored gate decision, prose-derived authority, unknown-mode fallback, duplicate evaluator in adapters

**Semantic Assessment**:
The provider-neutral `adoc.semantic_assessment.v0` envelope validated against one exact **Semantic Context**: typed findings, affected Object ID/hash pairs, closed citations, provider/model identity, proposed dispositions, candidate updates, unresolved questions, and validator-owned materiality. Human submissions additionally bind the reviewing and requesting Principals plus the validator-derived `self_assessment | independent` determination under the fixed `semantic_review` authority; Cloud policy decides whether that truthful fact is eligible. Model and human structured submissions share this boundary; unvalidated JSON has no typed core representation and explanatory prose never becomes gate authority.
_Avoid_: semantic review for the V1 contract, compliance verdict, provider response, model-set gate result

**Semantic Materiality**:
The closed `material | immaterial | undetermined` projection produced by `adoc.materiality.v0` from a validated finding's typed classification plus an exact cited diff-hunk fact (ADR-0059). The projection is deterministic; the executor's semantic classification is not. `no_change_required` is possible only for immaterial findings with no candidates or unresolved questions.
_Avoid_: model confidence, free-text judgment, deterministic semantic meaning, proposal approval

**Executor Qualification**:
The capability-specific `adoc.executor_qualification.v0` record that binds protocol validity, AgentDoc capability evaluation, organization approval for declared scope/risk/deployment, runtime-policy eligibility for one exact operation, and the exact executor configuration. Required-gate authority needs every layer and no requalification trigger. Protocol-valid-only output is advisory. Model executors bind exact executor/model/config and material-input digests; human executors bind an authenticated Principal and permission-policy digest instead of benchmark evidence.
_Avoid_: schema-valid means qualified, provider-wide qualification, benchmark requirement for humans, stale configuration authority

**Semantic Executor**:
The provider-neutral `adoc.semantic_executor_request.v0` boundary shared by Claude Code, Codex, declared generic/customer-hosted or local endpoints, and authenticated human submissions. A request embeds one integrity-validated ready **Semantic Context** plus exact executor, model, configuration, task, and prompt digests; only a validator-accepted **Semantic Assessment** with the declared provider/model identity produces a completed `adoc.semantic_executor_receipt.v0`. Adapters invoke providers and protect credentials, but never validate domain meaning or render unvalidated output.
_Avoid_: provider-specific assessment shape, undeclared endpoint class, caller-authored success receipt, free-text fallback

**External Work Request / Result**:
The paired `adoc.work_request.v0` and `adoc.work_result.v0` contracts for `source_ci` and `customer_worker` processing. The request digest binds the Workspace, repository/source, exact revision/change request, request nonce, requirements, expiry, and authorized workload Principal/subject/audience. The result repeats those trust boundaries and binds runtime identity/version, completion nonce, output digests, and its own digest; validation rejects reuse across a different request, Workspace, repository, revision, or workload identity.
_Avoid_: unsigned job payload, result matched by branch name, reusable upload token, caller-claimed identity, silent processing-mode fallback

**Embedding Provider**:
The internal port that turns a canonical embedding-input string into a vector. Implemented in code as the `EmbeddingProvider` trait under `domain/ports/`, governed by ADR-0006. The default adapter wraps `fastembed-rs` with `bge-small-en-v1.5`; the deterministic adapter is available for repeatable local/offline use.
_Avoid_: hosted-only embedding pipeline, public plug-in registry, per-call API key configuration

**Deterministic Embedding Provider**:
A production-configurable, repeatable hash-based embedding provider selected with `embeddings.provider: deterministic`. It emits `model: { provider: "deterministic", id: "hash-v1", dim: 384 }`, supports offline build/search parity, and must surface warnings because its vectors are non-semantic and lower quality than semantic model providers.
_Avoid_: hidden debug-only provider, calling deterministic vectors semantic quality, model-header mismatch between build and query

**Embedding Composition**:
The canonical input string each embedded record is reduced to before embedding. Knowledge Objects: `{kind}: {body_plain_text}\n[id: {id}] [status: {status}] [owner: {owner}]`. Prose blocks (V1.7.2): `prose: {text}\n[page: {page_id}]`. Part of the `adoc.search.v2` contract; changing either formula requires a schema-version bump.
_Avoid_: per-field separate embeddings in V1, relations folded into embedding input, freeform composition

**Hybrid Retrieval**:
The V1 default search ranking: Reciprocal Rank Fusion over a BM25 lexical index and a brute-force cosine vector index, with exact and prefix Object ID matches pinned above the fused list. Implemented as `HybridRanker` in `domain/retrieval/`.
_Avoid_: tunable score weights in V1, multi-factor PRD §19.3 scoring in V1, ANN libraries in V1

**Graph Retrieval**:
Opt-in retrieval that restricts candidates by graph reachability from a requested Object ID before normal lexical, semantic, or hybrid ranking. It is enabled by `adoc search --related-to`; absent graph flags, default ranking is unchanged.
_Avoid_: default graph score boost, graph proximity ranking, implicit relation expansion

**Graph Traversal**:
Read-only traversal over `docs.graph.json`, exposed by `adoc graph`. The default direction is both, the default relation set is the whole V0 Relation Set, and cycles are finite because visited nodes are not recursively revisited.
_Avoid_: infinite path enumeration, graph mutation, graph visualization as the current contract

**Retrieval Record**:
The stable JSON shape returned by `adoc why --format json` and `adoc search --format json`. Contained inside an `adoc.retrieval.v0` envelope. A projection of a graph Knowledge Object node, including its `content_hash`, plus a small `match` block carrying `mode`, ranks, and (when relevant) `cosine_score`.
_Avoid_: vectors in the retrieval envelope, adapter-local permission filters

**Retrieval Session**:
The immutable value the application layer assembles from a loaded graph artifact, explicit retrieval policy, an optional loaded search artifact, and built indexes. Permission filtering precedes index construction (ADR-0065). CLI commands construct one session per invocation; there is no global retrieval state.
_Avoid_: long-lived retrieval daemon in V1, mutable shared session

**Graph Index**:
The internal deep read module built from a loaded **Graph Artifact**. It validates artifact Object IDs, owns Knowledge Object lookup and iteration, relation traversal, related candidate selection, and related-status lookup for retrieval projection.
_Avoid_: duplicate exact lookup maps in retrieval sessions, ad-hoc graph scans in filters or rankers

**Pilot Retrieval Set**:
The V1.6 evaluation harness: `examples/billing-pilot/retrieval-set.yaml` carries 15-20 manually authored queries with `expected_ids` and `must_appear_in_top` thresholds, complemented by a property-based suite that asserts verbatim-body and Object-ID invariants. Both suites run in CI and gate ranking changes.
_Avoid_: ad-hoc retrieval review, ranking changes without recorded baselines

**Object Change**:
A DDD entity representing one entry in an **Object Diff**. Sealed enum with variants `Created { record }`, `Deleted { record }`, and `Changed { id, base, head }`. Constructible only via `ObjectDiff::compute`. Holds full before/after `KnowledgeObjectRecord`s on `Changed`, so V3.2 field-level projection is pure additive computation over the aggregate.
_Avoid_: line-level diff, text-level diff, prose-block diff, rendered-HTML diff

**Object Diff**:
The V3 aggregate `{ created[], deleted[], changed[] }` produced by `ObjectDiff::compute(&GraphRecord, &GraphRecord)` over two recompiled graph snapshots. Knowledge Object scope only — pages, prose blocks, `contains` edges, and `reference` edges are excluded. Sorted by Object ID; deterministic across runs. Serialized as `adoc.diff.v0`.
_Avoid_: full-graph diff, semantic diff over rendered output, page-level diff

**Field Change**:
A sealed `#[non_exhaustive]` enum projection over a `Changed` Object Change. Variants in V3.2: `Body`, `Status`, `Owner`, `VerifiedAt`, `EvidenceAdded`, `EvidenceRemoved`, `RelationAdded`, `RelationRemoved`. V3.3 adds `ImpactsAdded` and `ImpactsRemoved`. Drives type-based dispatch in V3.4 obligation rules.
_Avoid_: stringly-typed `{ field: String, before, after }`, free-form change descriptions

**Code Impact Path**:
A repo-relative file path declared on a `claim` or `decision` via the `impacts:` field, parsed into `RelPath`. Authored as a list; stored as non-empty, deduplicated, sorted `NonEmpty<RelPath>`. Opt-in — absence means the object has no source-path impact. Matched strictly per path; globs deferred.
_Avoid_: absolute path, `..` segment, free-text source description, evidence-as-impact overload

**Impacted Object**:
A verified claim, accepted decision, or verified API in the complete head graph whose `impacts:` or path-bearing evidence intersects the changed-file set returned by `ChangedFilesProvider` for a given base ref. Surfaced in `adoc.review.v0.impact[]`; carries the deduplicated changed file paths that matched. The legacy envelope does not expose the richer `impacts_path` / `evidence_path` reason kinds retained by the domain traversal.
_Avoid_: heuristic source matching, flagging non-verified objects, flagging without showing why

**Required Reviewer**:
The `owner` of a changed trusted subject or an Impacted Object, aggregated and deduplicated across the diff and complete head graph. Surfaced in `adoc.review.v0.required_reviewers[]` and rendered as a top-of-comment `@team-` mention list in markdown output.
_Avoid_: GitHub/GitLab reviewer mapping (deferred), per-line reviewers, file-pattern owners

**Review Report**:
The V3 aggregate `{ diff, impact[], required_reviewers[], proof_obligations[], patch_check? }`. Serialized as `adoc.review.v0`. New fields added across V3.4–V3.7 are JSON-optional with empty defaults; schema version stays `v0` for the whole milestone. V3.7 adds `patch_check`, the embedded `adoc.patch.check.v0` envelope produced when `adoc review --patch` (or the MCP `adoc_review` tool with a `patch` parameter) is invoked; its own `proof_obligations` are also unioned into the top-level `proof_obligations[]` (deduped by `(object_id, reason)`) so tolerant readers see the complete obligation set without descending into `patch_check`. The patch is never applied — V3 explicitly rejects hypothetical post-patch diffs.
_Avoid_: bumping schema per slice, including rendered HTML, mutating source, applying the patch, computing a speculative post-patch diff

**Snapshot Workspace**:
A RAII handle wrapping a filesystem path. Two variants: workdir (no-op cleanup on drop) or a temporary linked git worktree (drop runs `git worktree remove`). Returned by `SnapshotWorkspaceProvider::checkout`. Immutable review snapshots are materialized from resolved commit SHAs and verify their checked-out `HEAD` before reads. Each snapshot independently parses its own `agentdoc.config.yaml` and compiles only its configured documentation root; configuration is never borrowed across sides.
_Avoid_: leaking worktrees on panic, sharing tmp paths across processes, mutating the checked-out workdir, applying one snapshot's config to the other

**Snapshot Selector**:
The sealed enum input to a `SnapshotWorkspaceProvider`: `Workdir` or `GitRef(GitRef)`. `GitRef` is an opaque `String` passed verbatim to `git rev-parse`; supports branches, tags, SHAs, and revspecs without reinventing the parser.
_Avoid_: typed enum over branch/tag/sha, validating refs in the constructor

**Compatibility Mode**:
The second validation mode, introduced in V4, that applies to **Markdown Source** only. Raw HTML and unsafe link/image schemes that are errors under **Strict Mode** become `Severity::Warning` diagnostics under Compatibility Mode. Mode is selected purely by file extension — `.md` files are parsed under Compatibility Mode, `.adoc` files stay under **Strict Mode**. See ADR-0022.
_Avoid_: `--compat` flag on `.adoc`, project-wide compat toggle, third validation mode, runtime mode switching

**Source Mode**:
A property of a `SourceFile` set at the `SourceProvider` boundary from the file extension (ADR-0022): `Strict` for `.adoc`, `Compat` for `.md`. The classifier runs once during source construction; downstream pipeline stages read `source.mode()` instead of re-deriving from the path. Lives as `SourceMode` in `crates/adoc-core/src/domain/source.rs` alongside the `SOURCE_EXTENSIONS` discovery list, so adding a third extension is a single edit.
_Avoid_: re-deriving mode from the path in application stages, threading mode through tuples alongside the source, classifying mode in adapter code

**Mode Pipeline**:
The per-mode bundle of validation entry points (`parse`, `validate_source_page`, `validate_resolved_page`) returned by `pipeline_for(SourceMode)` in `crates/adoc-core/src/infrastructure/validate/mode_pipeline.rs`. The orchestrator iterates pages and calls into the pipeline instead of `match mode { Strict => …, Compat => … }`; mode dispatch is data, not code. Compat's `validate_resolved_page` is `ResolvedPagePolicy::Empty` rather than an `if mode == Strict` branch — the "Compat skips resolved-page rules" invariant lives in the table row. Extends ADR-0007's "rule registries are data" to mode selection.
_Avoid_: per-mode match arms in the orchestrator, parser/validator imports outside `mode_pipeline.rs`, skipping a phase by `if` rather than by `ResolvedPagePolicy` shape

**Markdown Source**:
The `.md` files AgentDoc ingests in V4 **Compatibility Mode**. Parsed by the **Markdown Parser** into a Page AST populated only with prose blocks. Never produces **Knowledge Objects**, relations, references, or typed metadata — durable structure still requires `.adoc` typed blocks. See ADR-0023.
_Avoid_: auto-typed claims from Markdown, inferred glossary terms from definition lists, Markdown as authoring source of truth

**Markdown Parser**:
The V4 parser for **Markdown Source**, wrapping `pulldown-cmark` with CommonMark and GFM feature flags. Lives at `crates/adoc-core/src/infrastructure/parser/markdown.rs` per ADR-0009's `domain <- application <- infrastructure` layout. Produces the same `Page` AST the `.adoc` parser produces, populated only with `ProseBlock` children. Spans are byte-offsets from `pulldown-cmark`, mapped to `LineIndex` for diagnostics. See ADR-0021.
_Avoid_: hand-written CommonMark parser, comrak/markdown-rs alternates, port-based abstraction over a pure-computation parser

**V4 Markdown Subset**:
The Markdown feature set V4 supports end-to-end: CommonMark core (headings, paragraphs, ordered/unordered lists, blockquotes, fenced and indented code blocks, links, emphasis, inline code, horizontal rules), GFM extensions (tables, task lists, strikethrough, autolinks, footnotes), and image embeds. Image and link URLs share the same scheme allowlist; `javascript:`, `data:`, and `vbscript:` are dropped. Unknown extensions (MDX, Pandoc directives, custom attribute blocks) emit `compat.unknown_extension` and render as escaped code.
_Avoid_: Markdown extensions out of scope in V4 (math, definition lists, embedded MDX components), partial GFM support, ad-hoc extension whitelisting

**Quarantined HTML**:
Raw HTML found inside **Markdown Source**, rendered as escaped text inside `<pre class="quarantined-html">…</pre>` (or `<code class="quarantined-html">…</code>` for inline). The CSS class string is authored once as `QUARANTINED_HTML_CLASS` in `crates/adoc-core/src/infrastructure/render/html.rs`. Visible to the reader as code, never interpreted as markup. The **Graph Artifact** stores the original source text on the wrapping `prose_block` node; quarantine is a renderer-side transform driven by the `compat.raw_html_quarantined` diagnostic.
_Avoid_: passing raw HTML through to the rendered output, dropping raw HTML silently, allowlisting iframe/script/style elements, hard-coding the class string outside `QUARANTINED_HTML_CLASS`

**Compat Validation Rule**:
A validation rule run after **Markdown Parser** parsing, against pages whose source is **Markdown Source**. Lives under `crates/adoc-core/src/infrastructure/validate/compat/` per ADR-0007 and ADR-0009. Emits `Severity::Warning` only — never `Severity::Error`. Examples: `RawHtmlQuarantine`, `UnsafeLinkDropped`, `UnsafeImageSrcDropped`, `UnknownExtension`. Runs in a parallel pipeline to the strict registry; the orchestrator in `compile_with_provider` dispatches by `source.mode()`.
_Avoid_: shared validator pipeline with a mode flag, raising compat rules to `Error` severity, applying compat rules to `.adoc` pages

**Markdown Pilot**:
The V4.4 evaluation fixture: `examples/markdown-pilot/` carries 15–20 hand-curated `.md` files modeled on real product docs, exercising the **V4 Markdown Subset**, **Quarantined HTML**, unsafe link/image dropping, and the empty-retrieval diagnostic. Paired end-to-end test in `crates/adoc-cli/tests/markdown_pilot.rs`. Mirrors the Billing Pilot pattern used to gate V0–V3.
_Avoid_: ad-hoc Markdown fixtures scattered across crates, large random Markdown corpora without curated diagnostic expectations

**Expanded Object Set**:
The V5 superset of the **Core Object Set**, comprising `claim`, `decision`, `warning`, `glossary` (V0) plus `constraint`, `procedure`, `example`, `policy`, `agent_instruction`, `contradiction`, `source` (V5). Every member is a **Knowledge Object** with stable identity, lifecycle, and (where the type requires it) evidence. Lives in code as variants of the `BlockKind` enum in `domain/knowledge_object/mod.rs`; each kind has its own aggregate file under `domain/knowledge_object/<kind>.rs`.
_Avoid_: ad-hoc string `kind` field, custom-schema-driven kinds before V6, kinds without a complete authoring/validation/rendering/graph story

**Severity**:
A shared value object with variants `Critical | High | Medium | Low`, used by `constraint`, `warning`, and `contradiction`. Lives at `domain/value_objects/severity.rs`; `#[non_exhaustive]`, `TryFrom<&str>`, total once constructed. Extracted from `warning`'s existing `WarningSeverity` enum into a shared value object (V5.1); the parse grammar is unchanged and the extraction is behavior-preserving.
_Avoid_: free-form severity string, per-kind severity duplication, severity comparison as numeric ordering before measured demand

**LifecycleStatus**:
A shared value object with variants `Draft | Verified | Deprecated`, used by the kinds whose closed status set is exactly `draft | verified | deprecated`: `example`, `procedure`, and `api`. Lives at `domain/value_objects/lifecycle_status.rs`; case-sensitive lowercase parse with `Missing`/`Invalid` errors that each aggregate maps into its own error variant, so per-kind diagnostic codes, help text, and requiredness are unchanged. A behavior-preserving extraction following the **Severity** template (ADR-0024); distinct from the derived, clock-dependent **Lifecycle Signal**.
_Avoid_: per-kind status-grammar duplication for identical sets, a generic status table over non-identical sets (claim/decision/observation/policy keep their own enums), confusing it with the derived Lifecycle Signal

**Trust**:
A value object on the **Agent Instruction Object** giving its authority level: an ordered enum `informal < team < authoritative < regulated < system` (PRD §17.2). Lives at `domain/value_objects/trust.rs`; `#[non_exhaustive]`, case-sensitive lowercase parse, `Missing`/`Invalid` errors mapping to `schema.agent_instruction_missing_trust` / `schema.agent_instruction_invalid_trust`. The ordering exists so a **Proof Obligation** can detect a trust *upgrade* (`after > before`); `trust` rides the graph node `status` slot via the metadata discriminant, exactly as `constraint` rides it with **Severity**.
_Avoid_: free-form trust string, treating `internal` as a trust level (corrected per ADR-0025), numeric trust comparison outside upgrade detection

**Constraint Object**:
A **Knowledge Object** representing a rule that must remain true (PRD §13.3). Required fields: `id`, `severity` (**Severity**), `body`. Constraints may declare `impacts:` per V3.3. Verified constraints require an `enforced_by` evidence reference. Lives at `domain/knowledge_object/constraint.rs`.
_Avoid_: claim that happens to read like a rule, constraint without severity, blanket constraints without a clear violated-when condition

**Procedure Object**:
A **Knowledge Object** representing an ordered sequence of steps (PRD §13.4). Required fields: `id`, `status`, `body`. Optional: `role_required`, `permissions_required`, `estimated_time`, `environment`, `rollback`, `risks`. `status` is a closed enum `draft | verified | deprecated` (ADR-0029); a `verified` procedure requires `owner`, `verified_at`, and at least one evidence field — `source`, `human_review`, or `reviewed_by` (the verified-claim rule with `human_review` accepted in place of `test`). The body must begin with an ordered list, else `schema.procedure_body_must_start_with_ordered_list`; the renderer emits it as `<ol>` numbered steps while the **Graph Artifact** stores body as canonical prose text (ADR-0029). Lives at `domain/knowledge_object/procedure.rs`.
_Avoid_: prose passed off as procedure, body that does not start with an ordered list, free-form procedure status, procedures with hidden role requirements

**Example Object**:
A **Knowledge Object** carrying a code, API, workflow, or usage example (PRD §13.5). Required fields: `id`, `lang` (or `format`), `body`. Optional `status` is a closed enum `draft | verified | deprecated` (absent ⇒ unverified); typos reuse `schema.invalid_status`. Verified examples additionally require both `checks` and `sandbox` declarations; here "verified" means *executable-declared* (no **Verification** evidence is involved), and V5 does NOT execute the checks. Lives at `domain/knowledge_object/example.rs`.
_Avoid_: example without `lang`, verified example without `checks` + `sandbox`, running `checks` from `adoc check` (deferred runtime concern)

**Policy Object**:
A **Knowledge Object** representing an authoritative organizational rule (PRD §13.12). Required fields: `id`, `status`, `owner`, `approved_by` (`NonEmpty<ApprovedBy>`, authored as scalar `approved_by: name` or list `approved_by: [a, b]`), `effective_at` (`YYYY-MM-DD`), `body`. Optional: `review_interval` (`[0-9]+d`). Supported statuses: `proposed | active | archived | revoked`. Policy does NOT support `verified` status — policy authority comes from approvers, not verification (ADR-0031). Required-field checks are aggregate-owned (`schema.policy_missing_*`); an `active` policy additionally must have `effective_at <= today`, enforced by the clock-dependent `PolicyActiveApproval` rule under `infrastructure/validate/` (`schema.policy_future_effective_at`). The renderer emits an approval block listing approvers and effective date; the graph node carries a dedicated `approved_by` slot. Lives at `domain/knowledge_object/policy.rs`.
_Avoid_: policy without approver, future `effective_at` on an active policy, `verified` status on policy, prose treated as policy

**Agent Instruction Object**:
A **Knowledge Object** declaring an explicit instruction targeted at AI agents (PRD §13.13). Required fields: `id`, `scope` (glob string), `trust` (**Trust**), `allowed_actions`, `forbidden_actions`, `body`. `allowed_actions` and `forbidden_actions` are **Disjoint Action Sets**. The fence word and graph kind are `agent_instruction` (the `::agent` / `trust: internal` shorthand in the V5-DESIGN acceptance example is corrected per ADR-0025). Required-field and disjointness validation are aggregate-owned (`schema.agent_instruction_*`); there is no clock-dependent rule, so no `ValidationRule` is added. A **Proof Obligation** fires a security review on a `trust` upgrade or a `forbidden_actions` removal. **Per ADR-0025, agent_instruction objects are NOT runtime ACLs.** They are authored, rendered, and retrievable knowledge; the MCP gateway does not consult them when deciding whether to run a tool, and the renderer emits a mandatory "NOT runtime ACL" banner linking `adoc://agent/v0/agent-instruction-guide`. Lives at `domain/knowledge_object/agent_instruction.rs`.
_Avoid_: treating agent_instruction as runtime permission grant, agent_instruction inferred from prose, omitting the "NOT enforced at runtime" renderer banner

**Contradiction Object**:
A **Knowledge Object** declaring an explicit conflict between two or more existing Knowledge Objects (PRD §13.14, §7.6). Required fields: `id`, `severity` (**Severity**), `status`, `claims` (`NonEmpty<ObjectId>` with arity ≥ 2), `body`. **Per ADR-0026, V5 contradictions are manually authored.** Automated pairwise scanning is deferred to V6+. A `claim` listed in an active contradiction may carry `status: contradicted` set manually by the author; V5 does NOT auto-propagate. Lives at `domain/knowledge_object/contradiction.rs`.
_Avoid_: automated contradiction detection in V5, contradiction with one claim, auto-transition of referenced claim status

**Source Object**:
A reusable evidence **Knowledge Object** (PRD §13.15). Required fields: `id`, `kind` (**Evidence Kind**), exactly one of `path: RelPath` or `url: Url`, `body`. The `body` is the prose explanation of what this source contains. Per ADR-0027, **Source Objects** coexist with inline V0 evidence fields on `claim` and `decision`; references to source objects are an opt-in upgrade, never a forced migration. Lives at `domain/knowledge_object/source.rs`.
_Avoid_: source object with both `path:` and `url:`, source object with neither, deprecating inline evidence in V5

**Evidence Anchor**:
The opt-in `hash` field on a path-target **Source Object**: a `sha256:` + 64-lowercase-hex hash of the cited file's full bytes taken at verification time (ADR-0048, V8.5.1). `adoc check` re-hashes the file through the `EvidenceFileReader` port and warns `evidence.hash_drift` when the cited content has changed — a deterministic changed-not-necessarily-wrong signal; the semantic judgment stays human. Absent `hash` means no reads and no diagnostics. Complements the path axis: `impacts:`/evidence paths answer "is a cited path in this diff", the anchor answers "did the cited bytes move since verification".
_Avoid_: line-span anchor, symbol resolution, automatic hash refresh, drift as a validation error or CI gate, automated contradiction detection, confusing it with **Base Hash** (graph-node hash for patches) or `patch.source_drift` (artifact-vs-source freshness)

**Evidence Kind**:
A value object enumerating the PRD §15.1 evidence types: `source_code`, `test`, `commit`, `pull_request`, `issue`, `design_doc`, `human_review`, `external_url`, `api_schema`, `runtime_metric`, `incident`, `support_ticket`, `audit_record`, `policy_reference`, `dataset`, `experiment`. Lives at `domain/value_objects/evidence_kind.rs`; `#[non_exhaustive]`, case-sensitive `TryFrom<&str>`, unknown kinds emit `schema.evidence_unknown_kind`.
_Avoid_: free-form evidence kind strings, deriving evidence kind from field name alone, accepting alias spellings

**V5 Evidence Model**:
The expanded evidence vocabulary on `claim` and `decision` introduced in V5.8. The V0 fields `source`, `test`, `reviewed_by` continue to accept string values for backwards compatibility. New evidence forms accept either an inline literal (matching the V0 string shape) or an `evidence_ref: <object-id>` Object ID reference to a **Source Object**. The minimum-evidence-by-kind table from PRD §15.4 is encoded in the verified-status validators per object kind.
_Avoid_: deprecating V0 inline evidence in V5, evidence quality scoring in V5 (deferred to V5.10+), evidence freshness checks in V5 (deferred to V5.10+)

**Disjoint Action Sets**:
The V5.5 invariant that an **Agent Instruction Object**'s `allowed_actions` and `forbidden_actions` sets share no common member. Enforced in `domain/value_objects/action_set.rs` via a value-object factory `DisjointActionSets::try_new(allowed, forbidden) -> Result<Self, OverlapError>` that is the only public path to a valid disjoint pair. Violations emit `schema.agent_instruction_actions_not_disjoint` and name each overlapping action.
_Avoid_: per-rule disjoint check that runs after aggregate construction, partial set validation, opaque overlap diagnostic without naming the overlapping actions

**Graph Artifact V3**:
The V5 graph artifact, `dist/docs.graph.json`, with schema version `adoc.graph.v3`. Additive bump from V2 — every V0–V4 node and edge shape is preserved byte-identical; new fields appear only on the seven new V5 kinds (`constraint`, `procedure`, `example`, `policy`, `agent_instruction`, `contradiction`, `source`). Older `adoc.graph.v2` artifacts are rejected by the existing reader with `SchemaUnsupportedVersion` and require a rebuild. The bump invalidates the **Search Artifact** for every project on first V5 build because the `graph_artifact_hash` changes — full re-embed expected.
_Avoid_: silently dropping unknown kinds at the v2 boundary, claiming the v3 bump is non-additive, bumping `adoc.search.v0` along with the graph (it stays at v0)

**V5 Expanded Pilot**:
The V5.9 evaluation fixture (implemented): `examples/expanded-pilot/` carries 11 hand-curated `.adoc` files across auth, billing, and security domains, exercising every new V5 kind, the **Severity** value object, the **V5 Evidence Model**, **Disjoint Action Sets**, and a **Contradiction Object** referencing two pre-existing `claim` objects. 18 Knowledge Objects; a stable `0 errors, 2 warnings` budget (two `lifecycle.expired`). Paired end-to-end test in `crates/adoc-cli/tests/expanded_pilot.rs`; maintenance contract in `docs/guides/expanded-pilot.md`. Mirrors the Billing Pilot (V1.6) and **Markdown Pilot** (V4.4) pattern.
_Avoid_: ad-hoc V5 fixtures scattered across crates, V5 fixtures without the contradiction case, drift between the pilot and the `docs/guides/expanded-pilot.md` maintenance contract

**Lifecycle Signal**:
A derived, clock-dependent fact about a **Knowledge Object**'s trustworthiness right now: stale (past `expires_at`), review-overdue (active policy past `effective_at + review_interval`), expiring-soon (verified, expiry within a requested horizon), or contradicted. Derived from authored fields, never authored itself, and re-derived **at read time** against the query date by the V6.1+ signal commands (`derive_effective_status_from_fields` is the single shared rule; ADR-0038). Signals are data for agents and humans to act on — not validation errors, not gates.
_Avoid_: health score, validation error, trusting the artifact's build-time `effective_status` at read time, treating stale findings as build failures

**Stale Query**:
The V6.1 read command `adoc stale` / MCP tool `adoc_stale` (implemented): a graph-artifact reader emitting `adoc.stale.v0` — `evaluated_at` plus records categorized `stale | review_overdue | expiring_soon`, sorted most-overdue first then Object ID. The `stale` category lists any object with a past expiry (the `lifecycle.expired` breadth); the record's `effective_status` re-derives `stale` only for verified objects and otherwise echoes the authored status. Exit 0 with or without records; logic in `application/signals.rs`. See `docs/design/V6-DESIGN.md` §V6.1.
_Avoid_: recompiling source to answer staleness, exit codes that gate on findings, `--fail-on` thresholds before measured demand

**Contradictions Query**:
The V6.2 read command `adoc contradictions` / MCP tool `adoc_contradictions` (implemented): a graph-artifact reader emitting `adoc.contradictions.v0` — every `unresolved` contradiction (`--all` adds resolved/dismissed) joined with every contradicted claim and its implicating contradiction ids, so consumers never join the lists. Implication is recomputed at read time via the `unresolved_contradiction_claim_index` shared with the build-time projection. **Clock-free by design**: no `evaluated_at`, byte-identical output for the same artifact on any day; the envelope reports the contradiction axis only (the expiry axis is the **Stale Query**'s job). Sorted severity-descending then Object ID. See `docs/design/V6-DESIGN.md` §V6.2.
_Avoid_: trusting the artifact's persisted `effective_status`, threading a clock into the contradictions path, `--all` changing `contradicted_claims`, consumers re-joining contradictions to claims

**Impacted Query**:
The V6.3 read command `adoc impacted-by` / MCP tool `adoc_impacted_by` (implemented): a graph-artifact reader emitting `adoc.impacted.v0` — the inverse of review impact: given changed source paths (explicit list XOR `--ref <git-ref>` against the working tree), every verified claim and accepted decision whose declared `impacts:` or evidence paths exactly match, with per-path `reasons` (`impacts_path` / `evidence_path`, optionally `via_source_object`) and one impact-review proof obligation each. `impacted_objects` is a pure sibling of `compute_impact` sharing `impact_entry_for` (ADR-0038); clock-free; exit 1 for bad input, 2 for environment failure, 0 otherwise. See `docs/design/V6-DESIGN.md` §V6.3.
_Avoid_: reusing `compute_impact`'s diff projection, glob `impacts:` matching, listing non-verified subjects, recompiling source to answer impact

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
- **V0 Build Artifacts** proved that the same **AgentDoc Source** can serve humans and agents.
- **Legacy Flat JSON Artifact** is retired; graph structure is now the canonical **Graph Artifact**.
- **V0 Source Composition** does not support includes; composition is by scanning files.
- **V0 Block Structure** keeps typed blocks top-level only.
- **Page Annotation** is optional in v0; missing page identity can be derived from the file path.
- **V0 Relation Set** references must resolve to existing Knowledge Object IDs and are preserved in the **Graph Artifact** as directed relation edges.
- Roadmap milestones should be **Tracer-Bullet Milestones**, not horizontal implementation layers.
- **V0 Implementation Stack** treats AgentDoc as compiler infrastructure first; future editor and web surfaces consume compiled artifacts.
- **V0 Rust Workspace** keeps CLI behavior separate from reusable compiler behavior.
- **V0 Parser Architecture** keeps diagnostics and source spans product-specific while leaving room to replace parser internals later.
- **V0 Core API** keeps the public core contract small; lower-level APIs can be exposed when LSP, web preview, semantic diff, or other integrations need them.
- **Public Core Surface** exposes serialized artifacts and retrieval envelopes, not graph/search artifact DTOs.
- The **Agent Usage Contract** is MCP-discoverable through **Agent Guidance Resources**, **Agent Workflow Prompts**, and the **Project Status Report**.
- A **Project Status Report** may run check/build refreshes only when explicitly requested; static **Agent Guidance Resources** never mutate files.
- **V0 Design Contract** guides scaffolding without replacing the roadmap or PRD.
- **Object ID** values are validated in v0 and form the citation target for humans and agents.
- **Diagnostic Code** values are semantic in v0; numeric aliases are deferred.
- **Validation Rule** runs after parsing; the parser emits only structural diagnostics, while semantic checks (raw HTML, unsafe link schemes) are validation rules.
- **Workspace Rule** is a validation rule that operates on the whole **WorkspaceAst** aggregate rather than a single page; future cross-page invariants (e.g. duplicate **Object IDs**, broken link targets) land as workspace rules without changing the orchestrator.
- An **Internal Port** stays `pub(crate)` until a concrete external consumer (LSP, web preview, semantic diff) needs it.
- **Build Output Directory** is created by the CLI when missing.
- **V1 Local Retrieval** reads compiled artifacts only; it never re-runs `compile_workspace()`.
- **V1 Build Artifacts** use the **Graph Artifact** as the canonical read artifact and add an optional **Search Artifact**; callers receive ready-to-write strings from `adoc-core`.
- A **Search Artifact** is keyed by **Object ID** and is invalidated by model mismatch, schema-version mismatch, or per-object content-hash drift.
- A **Graph Artifact** is keyed by **Object ID**, is derived from validated **AgentDoc Source**, is invalidated by schema-version mismatch, carries per-object **Base Hash** material, and does not carry presentation HTML.
- An **Agent Patch** is checked against the current **Graph Artifact** by **Patch Validation**; fresh **Base Hash** values prevent stale review intent from being accepted silently.
- A **Proof Obligation** can coexist with a valid **Agent Patch**; it means the change is structurally acceptable for review, not that verified knowledge is approved.
- An **Embedding Provider** is an **Internal Port** under ADR-0006; it stays `pub(crate)` until a concrete external consumer needs it.
- **Embedding Composition** is the reduction from a Knowledge Object aggregate to a single canonical input string; relations stay filter targets, not semantic signal.
- **Hybrid Retrieval** combines lexical and vector ranks via RRF; lifecycle, freshness, and authority remain filter targets in V1, not score modifiers.
- **Graph Retrieval** filters candidate sets explicitly; it does not change unfiltered **Hybrid Retrieval** ranking.
- **Graph Traversal** preserves original edge direction even when traversing incoming or both directions.
- A **Retrieval Record** is a projection of a graph Knowledge Object node plus a small `match` block; it never carries vectors.
- A **Retrieval Session** is constructed per CLI invocation, delegates graph reads to the **Graph Index**, and is dropped at command exit.
- The **Pilot Retrieval Set** gates every later ranking, embedding-composition, or model change.
- **Compatibility Mode** is the second validation mode, applying only to **Markdown Source**; **Strict Mode** continues to apply to all **AgentDoc Source**.
- **Markdown Source** produces `kind: "page"` and `kind: "prose_block"` graph nodes only — never **Knowledge Objects**, relations, references, or typed metadata.
- **Quarantined HTML** is escaped at the renderer boundary; the **Graph Artifact** stores original source text only, never interpreted HTML.
- **Compat Validation Rule** instances emit `Severity::Warning` only; the strict-mode equivalents (`parse.raw_html`, `parse.unsafe_link`) stay reserved for **AgentDoc Source**.
- The **Markdown Parser** uses `pulldown-cmark` per ADR-0021; **V0 Parser Architecture** stays hand-written for `.adoc`.
- The **V4 Markdown Subset** is fixed at V4; new Markdown features land only with new ADRs and new diagnostic codes.
- The **Markdown Pilot** gates V4 acceptance the way the **Pilot Retrieval Set** gates V1 ranking changes.
- A **Markdown Source** page never participates in `adoc.diff.v0`, `adoc.review.v0`, **Patch Validation**, or as a citation in retrieval — those surfaces remain Knowledge-Object-scoped.
- `adoc search` over a project containing only **Markdown Source** emits `retrieval.no_knowledge_objects_consider_migration` and points users at the `adoc migrate` workflow (shipped in V8.1).
- The **Expanded Object Set** extends the **Core Object Set** with `constraint`, `procedure`, `example`, `policy`, `agent_instruction`, `contradiction`, `source` — all of which are **Knowledge Objects** and inherit the existing **Object ID**, lifecycle, **Patch Validation**, and **Review Report** contracts.
- A **Severity** value object is shared by `constraint`, `warning`, and `contradiction`; no per-kind severity grammar.
- A **Constraint Object** may declare `impacts:` per V3.3; **Source-Path Impact** analysis treats verified constraints symmetrically with verified claims when computing **Required Reviewer** sets.
- A **Procedure Object** body preserves ordered-list structure through to HTML; the **Graph Artifact** stores body as canonical prose text — the renderer is responsible for visual ordering.
- A verified **Example Object** declares `checks` and `sandbox` but V5 does NOT execute them; runtime sandbox execution is a later milestone.
- A **Policy Object** does not support `verified` status; authority comes from `approved_by` plus a non-future `effective_at`.
- An **Agent Instruction Object** is read-only declarative knowledge per ADR-0025; the **MCP Agent Gateway** does not consult `allowed_actions` or `forbidden_actions` at runtime.
- **Disjoint Action Sets** are enforced at value-object construction time in `domain/value_objects/action_set.rs`; the validator only catches schema-level overlaps that originate at parse time.
- A **Contradiction Object** is manually authored in V5 per ADR-0026; automated pairwise scanning of verified claims is V6+.
- A **Source Object** coexists with V0 inline evidence per ADR-0027; references via `evidence_ref:` are an opt-in V5.8 upgrade, never a forced migration.
- The **V5 Evidence Model** encodes PRD §15.4 minimum-evidence-by-kind rules in per-kind verified-status validators; the V0 evidence fields continue to parse byte-identical.
- The **Graph Artifact V3** bump (`adoc.graph.v2` → `adoc.graph.v3`) is the only schema-version change in V5; `adoc.search.v0`, `adoc.retrieval.v0`, `adoc.patch.v0`, `adoc.patch.check.v0`, `adoc.diff.v0`, `adoc.review.v0`, `adoc.project.status.v0` all stay at their current versions.
- The **V5 Expanded Pilot** gates V5 acceptance the way the **Pilot Retrieval Set** gates V1 ranking changes and the **Markdown Pilot** gates V4 ingestion.
- A **Proof Obligation** triggers re-verify on a verified `constraint` `Severity` change, re-approve on an `active` `policy` `effective_at` or `approved_by` change, and security review on an `agent_instruction` `Trust` upgrade or `forbidden_actions` removal.

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
- "Agent JSON" could imply a current read model - resolved: the **Legacy Flat JSON Artifact** is retired; the current read model is the **Graph Artifact**.
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
- "V1 retrieval staging" could mean shipping lexical search first and embeddings later - resolved: V1 ships **Hybrid Retrieval** with embeddings as a first-class build output, gated behind the **Embedding Provider** port.
- "Embedding compute" could mean a hosted embeddings API in V1 - resolved: the V1 default **Embedding Provider** is local (`fastembed-rs` + `bge-small-en-v1.5`); a hosted adapter is deferred behind the same port.
- "Vector storage shape" could mean SQLite, an embedded ANN library, or a binary sidecar in V1 - resolved: V1 uses a sidecar JSON (**Search Artifact**) with `adoc.search.v0` as schema version.
- "Graph storage shape" could mean SQLite or a graph database - resolved: the local product uses a sidecar JSON (**Graph Artifact**), currently `adoc.graph.v6`; SQLite waits until JSON becomes limiting.
- "Embedding granularity" could mean per-paragraph or per-chunk embeddings - resolved: V1 is one embedding per **Knowledge Object**; chunked retrieval is deferred.
- "Search ranking" could mean a multi-factor weighted score from the PRD - resolved: V1 uses **Hybrid Retrieval** via parameter-free RRF; lifecycle, freshness, and authority remain filters, not score modifiers.
- "Agent surface" could mean an MCP/JSON-RPC retrieval server in V1 - resolved: V1 ships only CLI commands plus a stable `--format json` envelope (`adoc.retrieval.v0`); a server is deferred.
- "Graph ranking" could mean boosting search results by relation distance - resolved: **Graph Retrieval** is explicit candidate filtering only; unfiltered search ranking is unchanged.
- "MCP guidance" could mean only prose in repository docs - resolved: V2.2 exposes **Agent Guidance Resources** and **Agent Workflow Prompts** directly through the **MCP Agent Gateway**.
- "Project readiness" could mean agents should infer state from artifact files - resolved: agents use the **Project Status Report** before retrieval, semantic search, or patch validation.
- "Compatibility mode could relax `.adoc` validation when a project enables it" - resolved: **Compatibility Mode** applies only to `.md` files; `.adoc` files stay under **Strict Mode** regardless of project configuration (ADR-0022).
- "Markdown ingestion could auto-create suggested Knowledge Objects from prose" - resolved: **Markdown Source** is prose-only; auto-typing violates the evidence-first principle (ADR-0023). `adoc migrate` (V8.1) keeps the rule: suggested typed blocks are report records, never applied to output.
- "Markdown parser could be hand-written to match V0 Parser Architecture" - resolved: V4 uses `pulldown-cmark` (ADR-0021); CommonMark spec is too large to hand-roll for ingestion-only use.
- "Compatibility mode could be selected by CLI flag, config block, or file extension" - resolved: file extension only; no flag, no config (ADR-0022).
- "Prose blocks could become retrievable in V4 to make Markdown-only projects searchable" - resolved: V4 keeps the Knowledge-Object-only retrieval invariant; prose retrieval is its own milestone (**V1.7 Prose Retrieval**), applied symmetrically to `.md` and `.adoc` prose.
- "Raw HTML in Markdown could be passed through to the rendered output" - resolved: raw HTML becomes **Quarantined HTML**, escaped as visible text inside `<pre class="adoc-quarantined-html">`; the renderer is the security boundary, never the parser.
- "V5 could add all seven new object types in one slice" - resolved: V5.1 lands `constraint` + `Severity` (foundation); V5.2–V5.7 each add one kind; V5.8 expands the inline evidence model; V5.9 is the **V5 Expanded Pilot** that proves all of V5 end-to-end. One kind per slice mirrors V0.4's per-object discipline.
- "Severity could be duplicated per-kind rather than shared" - resolved: **Severity** is a shared value object (ADR-0024); `warning`'s existing private `WarningSeverity` enum is extracted into it and reused by `constraint` (and later `contradiction`). The extraction is behavior-preserving — warning's severity grammar and diagnostics are unchanged — and a shared type means severity means the same thing on every kind that carries it.
- "`agent_instruction` could double as runtime ACL for the MCP Agent Gateway" - resolved: **Agent Instruction Objects** are authored, rendered, retrievable knowledge ONLY per ADR-0025. The MCP gateway never consults `allowed_actions` or `forbidden_actions` at runtime; the renderer banner and the `adoc://agent/v0/agent-instruction-guide` resource make this explicit to humans and agents. Runtime permission enforcement is a future permission-engine milestone.
- "V5 could include automated contradiction detection via pairwise verified-claim scanning" - resolved: per ADR-0026, V5 ships only manually-authored **Contradiction Objects**. Automated detection is V6+; pairwise scanning requires the V6 graph store to be stable.
- "Verified-status rules could expand object-by-object across V5" - resolved: per-kind verified-status checks land in the slice that introduces the kind (V5.1 constraint, V5.2 procedure, V5.3 example, V5.4 policy active-status, V5.6 contradiction); V5.8 promotes the inline evidence model to **Evidence Kind** typing for `claim` and `decision`. Evidence-quality scoring (PRD §15.3) and freshness-driven status transitions are deferred to V5.10+ where they can be designed against measured pain.
- "`source` objects could deprecate inline evidence in V5.7" - resolved: per ADR-0027, **Source Objects** coexist with inline V0 evidence. V5.7 only introduces the new object type; V5.8 adds `evidence_ref:` to reference one. Inline evidence stays the canonical short form; references upgrade to a graph edge only when an author opts in.
- "V5 could ship without a graph artifact schema bump" - resolved: the v2→v3 bump is unavoidable because new `kind` values appear in the graph payload (ADR-0028). The bump is **additive only** — every V0–V4 node and edge shape is byte-identical; new fields appear only on the seven new V5 kinds. The **Search Artifact** stays at `adoc.search.v0` because the **Embedding Composition** formula is unchanged.
- "Verified examples could be executed by `adoc check` to validate the `checks` declaration" - resolved: V5 ships declaration-only **Example Objects**. Verified status requires both `checks` and `sandbox` present, but neither is executed; sandbox runtime is a later milestone.
- "Policy could support `verified` status by analogy with `claim`" - resolved: V5 policy supports `proposed | active | archived | revoked` only. Policy authority comes from `approved_by` plus `effective_at`, not from verification. Revisit only if the V5 Expanded Pilot reveals real demand.
- "The graph node `status` slot could keep carrying Severity (warning/constraint) and Trust (agent_instruction) indefinitely" - resolved: per ADR-0035, dedicated derived `severity`/`trust` fields are dual-emitted on graph nodes and **Retrieval Records** (additive within `adoc.graph.v3`, excluded from `content_hash`); the `status` slot is unchanged for wire stability, and a v4 cleanup makes `status` lifecycle-only. Other accepted as-built deviations from the V5 contract (pilot layout, flat validator location, `pub(crate)` `BlockKind`) are recorded in V5-DESIGN.md §Implementation Deviations.
