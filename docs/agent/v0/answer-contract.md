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

Do not cite content originating from `.md` files. Markdown source is ingested under V4 Compatibility Mode (`adoc://agent/v0/compat-guide`) and contributes prose-block nodes only — it never produces Knowledge Objects and carries none of the citation fields (`status`, `owner`, evidence) the answer contract requires.
