---
title: Acme Payments Documentation Overview
audience: developers
---

# Acme Payments Documentation Overview

This pilot is a hand-curated sample of the Acme Payments team's
existing Markdown docs, exercised end to end by AgentDoc V4
Compatibility Mode.

## What lives here

- `api/` — REST reference for the public payments API: auth,
  webhooks, rate limits, refunds, and error codes.
- `runbooks/` — operational procedures for on-call engineers,
  including incident response, database failover, and rotation
  handoff.
- `tutorials/` — onboarding walkthroughs for first-time integrators
  and release engineers (including this overview).
- `reference/` — supporting reference material: glossary notes and
  architecture sketches.
- `knowledge/` — native AgentDoc `.adoc` knowledge: verified claims
  and decisions about the payments ledger. These are the only files
  that contribute Knowledge Objects to the graph.

## Mode boundary

Markdown files under `api/`, `runbooks/`, `tutorials/`, and
`reference/` are ingested under V4 Compatibility Mode. They appear in
the graph as `page` and `prose_block` nodes only — never as Knowledge
Objects, per ADR-0023.

The `knowledge/` directory holds native `.adoc` sources. Those files
follow Strict Mode and contribute verified claims and decisions that
are citable by agents.

Mixing both file kinds in one tree is intentional: it proves the V4
extension-as-mode-signal contract (ADR-0022) works on a realistic
docs tree.
