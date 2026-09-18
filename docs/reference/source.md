# AgentDoc Source reference

AgentDoc Source is a small, strict language using `.adoc` files. It is not AsciiDoc.

````adoc
# Page Title @doc(product.area)

Paragraph text is plain prose.

- Unordered item
- Another unordered item

1. Ordered item
2. Another ordered item

```text
Fenced code is preserved and escaped in HTML.
```
````

Typed Knowledge Objects use top-level fenced blocks:

````adoc
::claim billing.ledger
status: verified
owner: team-billing
verified_at: 2026-05-06
source: ledger reconciliation report
--
The ledger records every credit and refund balance movement.
::

::decision billing.refund-policy
status: accepted
decided_by: architecture
depends_on: [billing.ledger, billing.credit-balance]
--
Use policy-based refund approval with ledger-backed audit entries.
::

::warning billing.invoice.manual-adjustment
severity: high
related_to: billing.ledger
--
Manual invoice adjustments must cite [[billing.ledger]] before approval.
::

::glossary billing.credit-balance
--
The customer-visible balance available for future invoices.
::
````

Supported object kinds:

<!-- adoc:kinds -->
- `claim`
- `decision`
- `glossary`
- `warning`
- `constraint`
- `policy`
- `procedure`
- `example`
- `agent_instruction`
- `contradiction`
- `source`
- `api`
- `observation`
- `question`
- `task`
<!-- /adoc:kinds -->

Supported relation fields:

- `depends_on`
- `supersedes`
- `related_to`

Relation values can be a single Object ID, a comma-separated list, or a bracket array. The compiler deduplicates repeated targets while preserving first occurrence order. A trailing empty segment from a final comma is ignored; leading or interior empty segments emit `id.invalid`. Valid targets that do not resolve to a declared Knowledge Object emit `ref.broken`; malformed targets emit `id.invalid`.

Object references use `[[object.id]]` in prose, headings, list items, and typed object bodies. References are rendered as HTML links and preserved as citeable source text in graph JSON object bodies.

Page annotations are optional. IDs must be lowercase dot-separated kebab-case values with at least two segments, such as `product.area`. If the first heading does not include `@doc(id)`, the compiler derives the page identity from the file path and applies the same ID grammar.

Raw HTML is rejected in strict mode:

```adoc
<div>not allowed</div>
```

Unclosed fenced code blocks are rejected:

````adoc
```rust
fn main() {}
````

Current limitations:

- custom schemas, includes, automatic semantic contradiction/alignment, hosted embedding adapters, web UI, managed multi-repository storage, and permissioned governance are not shipped
- current configuration remains repository-local; managed central knowledge, connectors, and on-prem operation are gated successor programs

See the [CLI reference](cli.md) for diagnostics and build behavior. The [quickstart example](../../examples/quickstart/refunds.adoc) is exercised by the [first-use smoke test](../../scripts/smoke-test.py).
