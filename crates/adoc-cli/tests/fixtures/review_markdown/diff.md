## Diff: 1 created, 1 deleted, 2 changed

<details><summary>⚠️ <code>billing.holds-policy</code> — status changed, owner changed, verified_at changed, evidence removed, relation added, relation removed, impacts added, impacts removed</summary>

**status:** verified → needs_review
**owner:** team-billing → team-payments
**verified_at:** 2026-05-05 → 2026-05-10
- evidence.source_code: holds-spec
- evidence.test: integration
+ depends_on: billing.refunds
- supersedes: billing.legacy-holds
+ impacts: crates/billing/src/holds-v2.rs
- impacts: crates/billing/src/holds.rs

</details>

<details><summary>✅ <code>billing.refunds</code> — body changed</summary>

```diff
- Refunds process within 24 hours.
+ Refunds process within 12 hours.
```

</details>

## Created
- `billing.holds`

## Deleted
- `billing.legacy-credits`
