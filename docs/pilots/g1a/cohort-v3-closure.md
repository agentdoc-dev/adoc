# G1A cohort v3 closure

- Closed at: `2026-08-26T20:49:00Z`
- Disposition: `invalidated_no_observation`
- Successor: [`evidence-contract-v4.yaml`](evidence-contract-v4.yaml)
- Eligible observations: none. No pull-request workflow run was created between `eligible_from` (`2026-08-26T20:43:00Z`) and closure.

Pre-observation review found that equal GitHub Actions `created_at` values did
not have a deterministic tie-breaker. The original v3 contract remains
unchanged for audit and contributes no evidence. Version 4 orders attempts by
`(created_at, run_id, run_attempt)` before scheduling or outcome.
