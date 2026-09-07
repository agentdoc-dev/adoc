# Managed field provenance

`adoc.managed_field_provenance.v0` is an unreleased opt-in immutable contract.
It binds one exact managed version/content digest to materially contributing
native assertion rows, scoped to the actual workspace. It does not change
`adoc.source_assertion.v0`, Source Binding or ACL snapshot bytes.

The closed [schema](schema/adoc.managed_field_provenance.v0.schema.json) requires
canonical lowercase UUIDs, a lowercase SHA256 content digest, 1–100 fields and
1–100 distinct assertion UUIDs per field. Selectors are `/body` or `/fields/`
followed by one nonempty canonical JSON-pointer token: only `~0` and `~1`
escapes, no unescaped slash, and at most 256 Unicode characters including the
prefix. The domain validator additionally rejects duplicate selectors whose
contributor lists differ. Native storage verifies selector existence, string
value, exact version/digest, assertion joins and current authority.

`build_managed_field_provenance` and `validate_managed_field_provenance` produce
an immutable value; fields and assertion IDs are sorted lexically for
`to_canonical_json`. Consumers accept valid unsorted manifests and normalize
only their in-memory value. Cloud retains exact supplied bytes and digest;
replay requires identical bytes after fresh authorization. Do not substitute
the normalized bytes when checking a retained native record digest.

`strictest_contributing_visibility` uses the existing public < internal <
restricted ordering. Authored object/field classifications are floors, with
absence meaning public. The maximum of every known contributor and both floors
wins. Empty contributor evidence or any missing contributor classification
returns unresolved `None`; invalid values return a typed error even when other
evidence is missing. Classification alone never grants reader access.

Policy administrators explicitly classify each historical snapshot through
Cloud's current native authorization surface. Until field projection lands in
E6.2.T2, provenance-bound versions are denied by old whole-object read paths.
T1 changes no CLI output, managed-runtime projection, or local audit behavior.
