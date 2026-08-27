# G1A cohort v7 closure

- Closed at: `2026-08-27T05:44:00Z`
- Disposition: `stop_ship_no_promotion`
- Successor: [`evidence-contract-v8.yaml`](evidence-contract-v8.yaml)
- Controlled observation: [Cloud run 33024353206, attempt 2](https://github.com/agentdoc-dev/cloud/actions/runs/33024353206/attempts/2), retained pass at head `642485d4cc0158614f126510df0f138f69a9ab59`.

The v7 Cloud revision predated the storage and route support needed to attest
that exact runtime revision on accepted ingestion rows. A deployment of the
frozen revision therefore could not produce evidence satisfying the evaluator,
so no real-run population can promote under v7.

Version 8 keeps the thresholds and authorized exact-tuple binding, pins the
attestation-capable Cloud revision, authenticates controlled outcomes from a
retained CI result, and enumerates runs against retained GitHub audit events.
The v7 contract and controlled observation remain unchanged and contribute no
v8 evidence.
