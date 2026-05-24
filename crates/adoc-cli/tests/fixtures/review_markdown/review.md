**Required reviewers:** @team-billing

## Diff: 1 created, 1 deleted, 2 changed

<details><summary>❌ <code>billing.holds-policy</code> — status changed, owner changed, verified_at changed, evidence removed, evidence added, relation added, relation removed, impacts added, impacts removed</summary>

**status:** verified → needs_review
**owner:** team-billing → team-payments
**verified_at:** 2026-05-05 → 2026-05-10
- evidence.test: integration
+ evidence.reviewed_by: team-payments
+ depends_on: billing.refunds
- supersedes: billing.legacy-holds
+ impacts: crates/billing/src/holds-v2.rs
- impacts: crates/billing/src/holds.rs

</details>

<details><summary>❌ <code>billing.refunds</code> — body changed</summary>

```diff
- Refunds process within 24 hours.
+ Refunds process within 12 hours.
```

</details>

## Created
- `billing.holds`

## Deleted
- `billing.legacy-credits`

## Impact
- `billing.refunds` → `crates/billing/src/refund.rs`

## Proof obligations
- [ ] `billing.holds-policy`: stale verified claim
- [ ] `billing.refunds`: re-verify body (evidence: source, test, reviewed_by)
- [ ] `billing.refunds`: review impacted claim (evidence: source)
