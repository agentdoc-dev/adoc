# G1A cohort v5 closure

- Closed at: `2026-08-27T01:40:00Z`
- Disposition: `stop_ship_no_promotion`
- Successor: [`evidence-contract-v6.yaml`](evidence-contract-v6.yaml)
- Selected observation: [AgentDoc run 33027153198, attempt 1](https://github.com/agentdoc-dev/adoc/actions/runs/33027153198), created `2026-08-27T00:32:03Z`, head `850787dc6ac2fc94136b4371d04a7ab66d98efc9`.
- Binding marker: [authorized exact tuple](https://github.com/agentdoc-dev/adoc/pull/179#issuecomment-5432759076).
- Retained receipt: `sha256:a2b522d53b59e2f17befe3fdd4e8feefcaf80d8f4d49a9477060828cb07463e4`; archive `sha256:c92abc60db5eed8638dc35db42dd67c5857fa207812330324417f5c30bee301a`.

The v5 binding control worked: the authorized exact-tuple marker was posted at
`00:32:07Z`, the binding job completed at `00:32:10Z`, and the assessment job
started at `00:32:14Z`. The pinned Action revision then rejected the
`pull_request:labeled` activity as `action.unsupported_event`. This required
workflow failure is a denominator failure, so the perfect 3/3 digest threshold
became unreachable and the remaining planned observations were not run.

Version 6 uses Action's already-supported `opened` activity and lengthens only
the binding wait so exact-head review and deterministic checks can finish before
the authorized marker schedules assessment. The original v5 contract, run,
binding marker, receipt, and failure remain unchanged for audit and contribute
no evidence to v6.
