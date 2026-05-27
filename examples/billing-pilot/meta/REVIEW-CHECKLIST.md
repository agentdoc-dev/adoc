# Billing Pilot Review Checklist

Use this checklist when reviewing generated `docs.html` and `docs.graph.json` artifacts for usefulness.

## HTML

- Confirm each page appears in source order with a visible heading and readable Knowledge Object sections.
- Confirm verified claims show owner, verified_at, and evidence fields.
- Confirm relation groups render as links and jump to the referenced object IDs.
- Confirm warning severity classes are visible in markup for high, medium, and low warnings.
- Confirm glossary references in prose render as object-reference links.

## Graph JSON

- Confirm page, prose block, and Knowledge Object nodes are present.
- Confirm every Knowledge Object node has `id`, `kind`, `body`, `page_id`, and `source_span`.
- Confirm verified claims expose `owner`, `verified_at`, and at least one evidence field in `fields`.
- Confirm `depends_on`, `supersedes`, and `related_to` relation edges preserve source direction.
- Confirm `contains` edges preserve page-to-block ordering.
- Confirm source paths point back to `examples/billing-pilot/*.adoc` files for citation workflows.
- Confirm no diagnostics are emitted for the pilot build.
