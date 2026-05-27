# Refunds

The refund endpoints reverse a captured payment in part or in full. A
refund draws the funds back from your settled balance and credits the
original payment method. Issuing a refund requires the `refunds.write`
scope.

## Refund lifecycle

A refund moves through `pending`, `succeeded`, or `failed`. Most refunds
settle within five business days, though card networks may delay
settlement during holiday periods.

For the canonical contract that describes how refund operators must
record audit trails before issuing credit, see the verified
`billing.refunds.issue-credit` claim in
[knowledge/billing-claims.adoc](../knowledge/billing-claims.adoc).

## Partial refunds

You can refund any amount up to the original captured total. Partial
refunds against the same payment are tracked individually so finance
can reconcile each line item.
