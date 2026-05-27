# Deploying to Production

Promoting an Acme Payments integration to production has three gates:
credential rotation, webhook validation, and ledger reconciliation.

## Credential rotation

Issue a separate token for production. Do not reuse the sandbox token.

## Webhook validation

Send a synthetic event to your production endpoint after rotation; the
dashboard tools show the delivery and signature outcome inline.

## Anti-pattern: embedded base64 diagrams

Earlier versions of this tutorial inlined the deployment diagram as a
base64 data URL so the page would render offline. The data URL has been
preserved below as an **anti-pattern** — the rendered docs drop the
unsafe `src` so the embed cannot smuggle markup back into the page.
Replace it with the linked PNG when refreshing the tutorial.

![deployment overview (anti-pattern)](data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIC8+)

## Ledger reconciliation

Run the reconciliation report at the end of the first production day.
Any unmatched entries should be triaged with the finance team before
the second day's traffic begins.
