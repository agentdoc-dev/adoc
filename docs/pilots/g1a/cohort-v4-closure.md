# G1A cohort v4 closure

- Closed at: `2026-08-26T23:50:00Z`
- Disposition: `failed_threshold`
- Successor: [`evidence-contract-v5.yaml`](evidence-contract-v5.yaml)
- Readout: [immutable Cloud revision](https://github.com/agentdoc-dev/cloud/blob/642485d4cc0158614f126510df0f138f69a9ab59/docs/pilots/g1a/evidence/v4/readout.json)

The v4 digest-match rate was 2/3 against a frozen threshold of 1. The selected
Cloud attempt completed before its exact tuple was bound, so it remains an
invalid denominator failure and is never replaced. Downstream Cloud governance
stays blocked by the v4 result.

Version 5 changes the Cloud ingestion pin to the reviewed receipt-compatibility
revision and requires a dedicated binding job to observe an exact, authorized
comment marker before the assessment job can be scheduled. The original v4
contract and readout remain unchanged for audit and contribute no evidence to
v5.
