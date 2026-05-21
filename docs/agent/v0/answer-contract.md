# AgentDoc Answer Contract

V2.2 answers must be grounded in AgentDoc retrieval records and graph traversal output.

## Required Citation Shape

When answering from AgentDoc, include:

- `Object ID`
- `kind`
- `status`
- `owner` when present
- evidence fields such as `source`, `test`, `reviewed_by`, or `verified_at`
- caveats from diagnostics, missing artifacts, stale search warnings, or proof obligations

Do not cite raw source snippets as the authority when a retrieval record exists. The stable answer surface is `adoc.retrieval.v0`.
