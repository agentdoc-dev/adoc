# G1A cohort v6 closure

- Closed at: `2026-08-27T02:52:00Z`
- Disposition: `stop_ship_no_promotion`
- Successor: [`evidence-contract-v7.yaml`](evidence-contract-v7.yaml)
- AgentDoc observation: [run 33032752044, attempt 1](https://github.com/agentdoc-dev/adoc/actions/runs/33032752044), retained pass at head `f7f9bb423a54c2fb0de49d3cc17f7c842d97a9c2`.
- Action observation: [run 33033734552, attempt 1](https://github.com/agentdoc-dev/action/actions/runs/33033734552), binding failure at head `368cec1996a7f222a95cae62cbc873935984ada9`.

The AgentDoc tuple completed after its authorized exact comment marker. The
selected Action tuple then exhausted all 180 polls even though two exact
markers were API-visible with `MEMBER` association; its assessment job was
skipped. That required missing assessment is a denominator failure, so the
perfect 3/3 digest threshold became unreachable and Cloud was not run.

Version 7 keeps the opened-event selection and exact tuple unchanged in
meaning, but moves the authorized marker to a distinct pull-request body line.
The binding job reads that marker from the same pull endpoint that already
validated the immutable head successfully. The v6 contract, observations,
markers, receipts, and failure remain unchanged and contribute no v7 evidence.
