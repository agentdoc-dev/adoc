# G1A cohort v1 closure

- Closed at: `2026-08-26T20:14:00Z`
- Disposition: `invalidated_no_promotion`
- Successor: [`evidence-contract-v2.yaml`](evidence-contract-v2.yaml)
- Preserved eligible observation: [AgentDoc run 33008823898, attempt 2](https://github.com/agentdoc-dev/adoc/actions/runs/33008823898), artifact created `2026-08-26T20:10:21Z`; assessment `sha256:39b94d25754cbefdadf12655bb6b8cf8febd4a6807bf5d2865d414dd09034799`; receipt `sha256:89f05c34b8bbe6ce9d9822d221038618cce173cec2ea449ab302307703d6e4b0`.

The first eligible v1 observation exposed two measurement defects before any
promotion: run selection depended on successful completion, and `frozen_at`
predated the final eligibility edit. The observation and original v1 contract
remain unchanged for audit; they do not contribute to G1A. Version 2 selects
attempts before outcome and records its actual final freeze.
