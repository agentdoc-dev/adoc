# AgentDoc Billing Pilot Dogfood

V2.2 dogfood uses `examples/billing-pilot` to prove the documented agent flow.

## Flow

1. Inspect project readiness with `adoc_project_status`.
2. Refresh with `check` or `build` when artifacts are not ready.
3. Search for billing concepts with `adoc_search`.
4. Fetch exact records with `adoc_why`.
5. Traverse related decisions or claims with `adoc_graph`.
6. Answer with citations using `Object ID`, `kind`, `status`, owner, evidence, and caveats.
7. Propose inline `adoc.patch.v0` JSON only when a source review is needed.
8. Validate the patch with `adoc_patch_check`.
