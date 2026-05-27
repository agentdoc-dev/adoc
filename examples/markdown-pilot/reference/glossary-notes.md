# Glossary Notes

Working notes the team uses to keep the public glossary consistent.
These notes are not the public glossary itself; they capture rationale
for terminology choices.

## Capacity reference

When sizing the settlement worker pool, the team uses the steady-state
throughput approximation. The original Markdown source kept the formula
in display math so it would render in the team's notebook tool — the
V4 docs preserve the source verbatim and the migration tool can later
suggest an equivalent native rendering:

$$
T = \lambda \cdot \mu
$$

In the formula above, lambda is the arrival rate of payment intents
and mu is the service rate of the settlement worker.

## Naming choices

- "Captured" beats "charged" — captured matches the network-side state.
- "Refund" beats "reversal" — reversals are an ACH-specific operation.
- "Settlement" beats "payout" — payout is the wire-transfer leg, which
  happens later.
