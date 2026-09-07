# ADR-0067: Preserve gateway audit history across delivery and login recovery

Status: Accepted for E6.3.T2 under the continuing E6 scope and the user's explicit same-person recovery decision on 2026-09-07. Extends ADR-0066; preserves accepted PRD, domain, event v1 and managed v0.

## Decision

Retain metadata-only exact gateway events in a private append-only durable spool inside the canonical served root. Append and sync before transmission or a pending release; acknowledge only a matching Cloud receipt. Restart and retry preserve exact event bytes, IDs, original caller/session and monotonic sequence. Corruption is a typed error on the next retrieval call, never truncation, deletion or a quiet reset. Use the installed runtime's file locking and existing containment/write patterns; do not introduce a queue service.

Default gateway delivery permits spooled-pending output only when current human/repository read and egress authority has been established, recording/acknowledgement is unavailable, and a fresh current-session check still succeeds before release. Explicit synchronous policy withholds output unless recorded. No authority cache, unbound/offline privilege, or egress permission is inferred from a saved binding. Pending and refused outcomes remain visible and machine-readable; ordinary nonsensitive wire payloads stay unchanged.

The same person may reauthenticate to upload unchanged old-session pending events. Cloud separately checks the current uploader session/permissions/egress and retains original event identity. A small immutable upload fact records successful reauthentication without rewriting historical event bytes. Different principals, invalid current sessions, revoked read permission and disabled egress refuse. Historical drain never releases old withheld content; new reads bind the current login and receive their own event.

Use a separate historical-session transmission preflight endpoint returning the existing original binding plus fresh transmission metadata. Normal current-session setup and exact event envelopes/receipts remain unchanged. Database changes append a new migration.

## Validation and bounds

Prove real MCP restart and sink recovery, fivefold redelivery without duplicate events, failure at append/ack boundaries, root confinement and cross-process locking, sticky corruption, explicit synchronous/default pending outcomes, refreshed same-person login and denied other-person/current-permission/egress cases. Preserve exact ordinary response bytes and sensitive subject attribution. No persistent content response, token storage, external queue, spool garbage collection/capacity alerting, merge or deployment is included.

The six unreleased public MCP Rust retrieval wrappers return the same `CallToolResult` as actual async handlers so status and warnings cannot be discarded; all 43 in-repository direct callers are updated. This corrects the Rust return type while preserving ordinary MCP wire bytes and closed content envelopes.
