# G1A cohort v2 closure

- Closed at: `2026-08-26T20:33:00Z`
- Disposition: `invalidated_no_promotion`
- Successor: [`evidence-contract-v3.yaml`](evidence-contract-v3.yaml)
- Preserved selected observation: [AgentDoc run 33010226126, attempt 2](https://github.com/agentdoc-dev/adoc/actions/runs/33010226126), job started `2026-08-26T20:25:42Z`, artifact created `2026-08-26T20:25:57Z`; assessment `sha256:cd1025e7d6afe927b8d7f0e51d06c46fecd2e1eb0b28fbb5953d8f6b1f3c94d7`; receipt `sha256:8ea64c74420c1ae37f8e00f808b90c9233690c0166e3fb7ca52aac0e00ec4d66`.

The first selected v2 observation exposed that eligibility still depended on a
job starting. A cancelled or unscheduled first workflow attempt could therefore
be skipped. The observation and original v2 contract remain unchanged for
audit; they do not contribute to G1A. Version 3 selects the first dispatched
workflow attempt before job scheduling or outcome.
