# Core Concepts

A short orientation to the vocabulary used across the Acme Payments
docs. The full glossary lives in
[../reference/glossary-notes.md](../reference/glossary-notes.md).

## Workspace

A workspace is the billing and access boundary for an integration.
Workspaces own tokens, webhook endpoints, and payout settings.

## Payment intent

A payment intent represents a single attempt to collect money from a
customer. It moves through `requires_action`, `processing`, and
`succeeded` or `failed` terminal states.

## Settlement

Settlement is the point at which captured funds become eligible for
payout to your bank account. Most card networks settle within two
business days.
