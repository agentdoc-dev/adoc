# Incident Response

This runbook describes the first thirty minutes of an active payments
incident. Follow the steps in order; do not skip ahead even if the cause
seems obvious.

## Page the on-call

The dashboard renders a customer-visible banner for severity-1 incidents.
The team copied the legacy HTML banner directly into this runbook so the
exact markup is preserved for the comms reviewer:

<div class="alert alert--sev1">
  <strong>Severity 1 incident in progress.</strong>
  Customer-facing services are degraded. Follow the comms checklist
  before posting to status.acme.test.
</div>

## Status page automation

The status page used to update via an inline script. That snippet is
preserved here for the migration tracking ticket — it must not run from
this document and will not be ported into the new runbook system as-is:

<script>
  // Legacy inline status updater. Do not re-enable; this is preserved
  // only so the migration audit can compare wording.
  window.statusBanner = { severity: 'sev1', visible: true };
</script>

## Mitigation

Once paging is acknowledged, follow the mitigation tree in
[on-call-rotation.md](./on-call-rotation.md). The first ten minutes are
diagnosis; the next twenty are containment and customer comms.
