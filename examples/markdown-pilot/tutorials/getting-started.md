---
title: Getting Started with Acme Payments
audience: developers
estimated_time: 20m
---

# Getting Started with Acme Payments

This tutorial walks through the minimum viable integration: creating a
sandbox workspace, issuing your first test charge, and verifying a
webhook.

## Step 1 — Create a workspace

Sign in at https://dashboard.acme.test and create a sandbox workspace.
Sandbox workspaces use the test card numbers documented at
https://docs.acme.test/test-cards.

## Step 2 — Capture a payment

Run the SDK quickstart from the dashboard. The sample shows the same
flow your production integration will use.

![dashboard quickstart](https://docs.acme.test/img/quickstart.png)

## Step 3 — Verify a webhook

Configure a webhook endpoint pointing at your local development tunnel.
The dashboard delivery log shows whether the request reached your
service and whether the signature validated.

When the webhook arrives, confirm the response status is 2xx and the
signature header matches the payload. See
[../api/webhooks.md](../api/webhooks.md) for the full handler contract.
