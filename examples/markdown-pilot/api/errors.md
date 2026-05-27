# Error Codes

Acme Payments returns structured error responses for every non-2xx
status. Each response carries a stable `error.code` field that callers
can branch on programmatically.

## Common codes

- `auth.token_expired` — the bearer token is past its `exp`; refresh
  and retry the request.
- `auth.scope_missing` — the token lacks a scope the endpoint requires.
- `payments.amount_invalid` — the amount is below the minimum allowed
  for the currency or is non-positive.
- `refunds.exceeds_captured` — the refund amount is larger than the
  remaining refundable balance on the payment.
- `webhooks.signature_invalid` — the signature header did not validate
  against the configured endpoint secret.

## Retired codes

The ~~`legacy.unknown`~~ catch-all is no longer returned. Callers that
fell back to it should switch to the specific codes above; the
retired code is now returned as `system.unspecified` only when a new
internal error has no public code yet.
