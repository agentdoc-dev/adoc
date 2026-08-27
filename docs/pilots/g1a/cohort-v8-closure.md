# G1A cohort v8 closure

- Closed at: `2026-08-27T07:10:00Z`
- Disposition: `stop_ship_no_promotion`
- Successor: [`evidence-contract-v9.yaml`](evidence-contract-v9.yaml)

The v8 eligibility window began before its contract had an immutable Sigstore
seal in Rekor. No real-run PR was opened for v8, and no v8 observation is used
for promotion.

Version 9 preserves the v8 revisions, thresholds, workflow authentication, and
evaluation rules, but begins eligibility only after its own immutable seal can
be issued and verified. The v8 contract remains unchanged.
