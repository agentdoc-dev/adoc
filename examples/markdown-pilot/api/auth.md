---
title: Authentication
audience: developers
last_reviewed: 2026-05-20
---

# Authentication

The Acme Payments API accepts OAuth 2.0 bearer tokens. Every request must
include an `Authorization: Bearer <token>` header. Tokens are scoped per
integration and expire after sixty minutes.

## Scopes

Request only the scopes you need. Over-broad scopes are rejected during
review.

| Scope               | Purpose                                  | Refresh window |
| :------------------ | :--------------------------------------- | -------------: |
| `payments.read`     | Read payment intents and charge records  |        90 days |
| `payments.write`    | Create and cancel payment intents        |        30 days |
| `refunds.write`     | Issue refunds against captured payments  |        14 days |
| `webhooks.manage`   | Configure webhook endpoints and secrets  |         7 days |

## Token rotation

Rotate production tokens at least once per quarter. Use the staging
environment to verify the new token works before retiring the old one,
and revoke the old token through the dashboard.
