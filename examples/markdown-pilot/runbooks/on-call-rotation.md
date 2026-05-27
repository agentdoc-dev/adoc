# On-Call Rotation

The payments on-call rotation runs in weekly shifts. Primary covers
P1 and P2 alerts; secondary covers P3 and escalation backup.

## Current rotation

| Week starting | Primary       | Secondary      |
| :------------ | :------------ | :------------- |
| 2026-05-25    | priya.s       | dimitri.l      |
| 2026-06-01    | tomasz.k      | alex.b         |
| 2026-06-08    | sara.m        | ji-min.l       |
| 2026-06-15    | dimitri.l     | priya.s        |

## Handoff checklist

At Monday handoff, the outgoing primary walks the incoming primary
through any open incidents, pending tickets, and changes scheduled for
the week.

## Things to avoid

A previous edition of this runbook included a one-click "force failover"
link that invoked an in-page script. The link is preserved below as an
**anti-pattern** — the rendered docs strip the unsafe scheme so it
cannot fire. Do not re-introduce this style of link.

Click here to [trigger a force failover](javascript:forceFailover()) —
this link must never be live in rendered docs.
