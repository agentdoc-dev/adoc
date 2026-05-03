# AgentDoc

AgentDoc is a human-readable documentation system for teams that need documentation to behave like maintained, agent-safe knowledge.

Traditional docs are mostly formatted text. AgentDoc treats durable documentation as typed, versioned, evidence-backed knowledge objects that humans can read and AI agents can safely retrieve, cite, validate, and update through reviewable workflows.

This repository is currently in the product-definition stage. The source of truth for the initial scope is [docs/PRD.md](docs/PRD.md).

## Product Thesis

Modern documentation needs to answer questions that Markdown cannot represent on its own:

- Is this statement current?
- Who owns it?
- What evidence supports it?
- Where does it apply?
- Is it verified, draft, stale, deprecated, or contradicted?
- Can an AI agent safely use it?
- What code, tests, tickets, commits, or reviewers support it?

AgentDoc's goal is to let humans keep writing readable notes while turning durable knowledge into a validated graph that agents can use safely instead of guessing from prose.

## Core Ideas

- **Readable source format:** prose by default, structure when knowledge becomes durable.
- **Typed knowledge objects:** claims, decisions, constraints, procedures, examples, warnings, policies, glossary terms, agent instructions, contradictions, and related objects.
- **Stable object IDs:** references target durable object IDs instead of fragile headings or line numbers.
- **Evidence-backed knowledge:** verified objects link to source code, tests, commits, tickets, reviewers, data, or external sources.
- **Lifecycle-aware docs:** objects can be draft, proposed, accepted, verified, stale, deprecated, superseded, contradicted, revoked, or archived.
- **Agent-safe retrieval:** agents retrieve status-aware, scope-aware, permission-aware objects with citations.
- **Semantic review:** changes can be reviewed as knowledge-object changes instead of only text diffs.
- **Governance and trust:** ownership, approval, permissions, audit trails, and trust boundaries are first-class product concerns.

## Example Source

AgentDoc source is intended to stay readable while carrying machine-parseable structure:

```adoc
# Billing Credits @doc(billing.credits)

Users spend credits when generating content.

::claim billing.credits.decrement-after-success
status: verified
owner: backend-platform
source: apps/backend/src/features/credits/consume.use-case.ts
test: apps/backend/src/features/credits/consume.test.ts
verified_at: 2026-05-02
expires_at: 2026-08-02
--
Credits are decremented only after generation completes successfully.
::
```

Compiled output should support human docs, agent JSON, search indexes, graph data, CI diagnostics, and future compliance or review views.

## Planned CLI

The PRD defines `agentdoc` as the primary developer interface. Planned commands include:

```bash
agentdoc init
agentdoc check
agentdoc build
agentdoc search
agentdoc explain
agentdoc impacted-by
agentdoc patch
agentdoc migrate
```

The MVP CLI should validate source files, compile human and agent-facing outputs, detect broken references, reject raw HTML in strict mode, report stale objects by expiration date, and expose basic search.

## MVP Scope

The first usable version is expected to focus on:

- AgentDoc source syntax
- typed blocks with stable IDs
- core schema validation
- lifecycle, owner, and evidence fields
- references by object ID
- strict mode and compatibility mode
- HTML rendering
- JSON output for agent retrieval
- basic search
- stale-by-expiration diagnostics
- Markdown migration reports
- read-only agent retrieval
- documentation and examples

The MVP explicitly does not include the full SaaS web app, enterprise RBAC, full compliance suite, schema marketplace, autonomous agent approval, or complex AI contradiction reasoning.

## Intended Architecture

AgentDoc is designed as a pipeline:

```text
Authoring sources
  -> parser and compiler
  -> schema validator
  -> knowledge object store
  -> knowledge graph
  -> lifecycle and evidence engines
  -> permission and trust engine
  -> agent API
  -> renderers and lenses
```

The same source should be usable through multiple lenses:

- docs website
- agent view
- search index
- knowledge graph
- IDE view
- semantic diff
- CI diagnostics
- compliance report

## Agent Safety Model

AgentDoc separates content from instructions. Agents may read prose, but they should only follow explicit, typed, trusted `agent` instruction objects with scoped permissions.

Agent-facing responses should prefer verified and accepted objects, include citations, surface stale or contradictory knowledge, and refuse to answer definitively when unresolved contradictions make the answer unsafe.

## Development Status

Current repository contents:

- `docs/PRD.md` - full draft product requirements document
- `README.me` - initial project overview

No implementation is present yet. The next practical step is to choose the prototype implementation stack and build the Phase 0 parser, schema validator, compiler, basic diagnostics, JSON output, HTML output, and simple CLI.

## North Star

AgentDoc succeeds when teams stop asking only "Where is the doc?" and can instead ask:

```text
What do we currently believe?
What proves it?
Who owns it?
Where does it apply?
Can an agent safely use it?
```
