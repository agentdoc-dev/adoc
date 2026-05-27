---
title: Webhooks
audience: developers
---

# Webhooks

Webhooks deliver event notifications to your service as soon as state
changes in Acme Payments. Each event is signed with the secret you
configure for the endpoint.

## Implementation checklist

Use this checklist when wiring a new webhook handler.

- [x] Register the endpoint URL in the dashboard
- [x] Store the per-endpoint signing secret in your secret manager
- [ ] Verify the signature on every incoming request[^signing]
- [ ] Respond with a 2xx within fifteen seconds
- [ ] Replay failed deliveries from the dashboard during incident review

## Retry behaviour

Deliveries that fail with a 5xx response or time out are retried with
exponential backoff for up to twenty-four hours. Persistent failures
disable the endpoint until an operator re-enables it.

[^signing]: The signature header is `X-Acme-Signature` and uses HMAC
    SHA-256 over the raw request body.
