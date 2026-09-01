# G1A cohort v9 closure

- Closed at: `2026-09-01T11:20:02Z`
- Disposition: `fail_no_promotion`
- Successor: [`evidence-contract-v10.yaml`](evidence-contract-v10.yaml)

The three selected workflows and the controlled suite completed successfully,
and the retained Action and Cloud bytes have matching digests. The v9 evaluator
nevertheless failed closed because its attempt-population authentication used
GitHub's organization audit-log API, which is unavailable on the
`agentdoc-dev` GitHub Free plan.

The owner declined GitHub Enterprise Cloud solely for this internal pilot.
Version 10 therefore replaces deletion-resistant audit-log enumeration with
authenticated GitHub Actions REST API enumeration. Runs unavailable from that
API during evaluation, including deleted runs, are explicitly excluded, and
v10 makes no deletion-resistant population claim. This is a material rule
change, so the v9 contract and failed readout remain unchanged.
