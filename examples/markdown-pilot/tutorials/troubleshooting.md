# Troubleshooting

Common integration failures grouped by symptom. If your symptom is not
listed, page the developer-relations on-call.

## Dashboard embeds

The legacy team docs embedded a live dashboard widget directly in this
page. The embed used an MDX-style component, which V4 Compatibility Mode
does not interpret — the source is preserved so the migration audit can
see what the original docs intended:

<DashboardWidget tenantId="acme" view="payments-health" />

## Inline operations notes

When pairing this troubleshooting page with the operations runbook, the
team historically included a Pandoc callout to flag the most common
mitigation step. The callout block is preserved here in its original
form so the migration tool can suggest the right native equivalent:

:::warning
Verify the webhook signing secret before re-enabling the endpoint. A
mismatched secret will silently drop deliveries.
:::

## When to escalate

If the symptom involves customer-visible charges or refunds, follow the
incident-response runbook before posting in the developer support
channel.
