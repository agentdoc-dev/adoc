# ADR-0024: Severity Is a First-Class Shared Value Object

## Status

Accepted.

## Context

V5 (the Expanded Knowledge Model) adds `constraint` in its first slice and `contradiction` later, both of which carry a severity. The existing `warning` Knowledge Object already models severity — but as a *private* `WarningSeverity` enum (`Low | Medium | High | Critical`) declared inside `domain/knowledge_object/warning.rs`, validated by a `WarningSeverity::try_new` that accepts the lowercase canonical strings and rejects everything else.

The question is how the new kinds should obtain a severity, given that a typed severity already exists but is owned by a single aggregate. Three options:

1. **Duplicate a severity enum per kind.** `ConstraintSeverity`, `ContradictionSeverity`, etc., each with its own parse and `as_str`. Simple in isolation, but it means "critical" is three different types that cannot be compared, projected, or rendered through one path — and three near-identical parse functions drift over time.

2. **Keep severity a free-form string with per-kind validation.** Each aggregate stores `String` and validates at the boundary. This is the shape the V5 design doc originally assumed warning had. It is strictly worse than the status quo: `warning` is *already* typed, so this would be a regression, and it pushes invariant enforcement out of the type system.

3. **Extract `WarningSeverity` into a shared `Severity` value object.** Lift the existing enum into `domain/value_objects/`, alongside the `RelPath` precedent, and have `warning`, `constraint`, and `contradiction` all reuse it.

Option 3 is the only one consistent with the project's tactical-DDD layout (ADR-0009) and the value-object discipline already established by `RelPath`. Severity must mean the same thing on every kind that carries it; a shared type makes that structural rather than conventional. Critically, because `WarningSeverity` is already a typed enum with exactly the target grammar, the extraction is behavior-preserving — not the "free-form string → typed" breaking migration the V5 design doc first described.

## Decision

`Severity` becomes a shared value object at `domain/value_objects/severity.rs`:

- Variants `Critical | High | Medium | Low`, `#[non_exhaustive]`.
- A fallible constructor (`try_new` / `TryFrom<&str>`) with the unchanged grammar: ASCII-trimmed, lowercase-exact match on `low | medium | high | critical`; empty input and any other spelling are rejected.
- `as_str` / `Display` for the canonical lowercase rendering.
- Inline `#[cfg(test)]` coverage mirroring `rel_path.rs`: valid construction, rejection of empty/unknown/miscased input, and `Display` round-trip.

`warning` drops its private `WarningSeverity` enum and stores `Severity`. This is behavior-preserving: warning's severity grammar, its `SchemaMissingField` (missing) and `SchemaInvalidStatus` (invalid) diagnostics, and every existing warning fixture stay byte-identical. The metadata discriminant in `domain/knowledge_object/projection.rs` becomes a shared `Severity` discriminant used by both `warning` and `constraint`.

`constraint` (V5.1) stores `Severity` as a required field and gets dedicated diagnostic codes `SchemaConstraintMissingSeverity` / `SchemaConstraintInvalidSeverity`. Warning's existing codes are deliberately left untouched so the extraction stays a pure refactor; unifying the two kinds' severity diagnostics is a possible later cleanup, not part of V5.1.

## Consequences

Adding a severity-bearing kind in V5 (and beyond) is a one-line field plus a reused validator, not a new enum. The shared type flows through one projection path into the graph artifact `status`/severity field and one HTML rendering path (the `--{severity}` badge class), so a new kind renders and serializes its severity identically to `warning` for free.

`Severity` carries no numeric ordering or comparison in V5 — variants exist only as a closed set. Ordering (e.g. "critical > high" for filtering or sorting) is deferred until a measured need appears, per YAGNI; adding it later is non-breaking because the enum is `#[non_exhaustive]`.

The extraction touches `warning.rs` and `projection.rs` but changes no observable behavior, so the warning test suite and all V0–V4 fixtures pass unchanged — the refactor is validated by the existing tests rather than requiring new ones beyond the value object's own unit tests.
