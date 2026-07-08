# Architecture Notes

High-level shape of the payments services. These are working notes, not
a published architecture document.

## Service boundaries

Each service owns one bounded context and one database schema. Inter-
service communication happens over the event bus, not direct database
reads.

## Callouts

Important note about request limits {.callout}

The team's previous tooling rendered the line above as a styled callout
via a Markdown attribute block. V4 Compatibility Mode does not
interpret attribute blocks; the source is preserved so a later
`adoc migrate` run can suggest a native `warning` object.

## Capacity planning

Capacity planning is reviewed quarterly with the SRE team. The current
review cadence and outputs are tracked in the SRE planning board.
