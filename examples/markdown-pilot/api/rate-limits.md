# Rate Limits

The Acme Payments API enforces per-token rate limits. Limits scale with
the workspace plan; current ceilings are published at
https://docs.acme.test/rate-limits and update automatically with the
plan tier.

## Headers

Every response includes:

- `X-RateLimit-Limit` — requests permitted in the current window
- `X-RateLimit-Remaining` — requests still available
- `X-RateLimit-Reset` — UNIX timestamp at which the window resets

## Deprecated behaviour

The ~~`X-RateLimit-Quota`~~ header was retired in March 2026. Switch any
remaining callers to the three headers above; the legacy header now
always reports zero.
