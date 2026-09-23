# Migration export conformance fixture

`migration-export-native.json` retains unmodified JSON values emitted by the Cloud
T5 actual-runtime and authorized-human integration run on 2026-09-08. It contains
one complete successful export group and the syntax-invalid and invalid-UTF8
qualification source-evidence groups. The duplicate successful export group was
omitted. The fixture contains generated test identities, not credentials.

These are native export JSON values, re-indented for this fixture. They do not
claim original retained wire-byte encoding. The tests validate published shape;
Cloud integration separately proves authorization, causal joins and roundtrip
behavior. No fixture record grants activation authority.

## Migration lifecycle fixture

`migration-lifecycle-native.json` is a byte-identical copy of the E7.2.T1 real
PostgREST conformance capture from 2026-09-08. It retains six native result
objects (including original base64 receipt bytes), their decoded receipt JSON,
five actual command inputs and seven exported native facts. The real pinned
runtime and signed-in human initialization case reached `catching_up`; it did
not perform cutover. Runtime SHA256:
`8c9093250e761f7d772a95c64ce2b4825388a5b6a3ccf3207e5c6c8d0308eda5`.

All six receipt byte digests, LF endings and decoded JSON correspondence were
verified when copying this fixture. The registered shape tests retain the byte
strings unchanged and check the captured ordering and references. The Cloud
suite independently tests native authorization, concurrency and causal
admission. No handmade output or reconstructed original prepare receipt is
included; `preparation_validation` is native metadata projection only.

## Migration cutover fixture

`migration-cutover-native.json` contains records from the actual signed-in local
Supabase/PostgREST T3 integration on 2026-09-13, using the freshly built paired
Adoc CLI for preparation and qualification. The capture was normalized into
28 typed values in `records` and two original policy export descriptors in
`retained_policy_parts`; record values and receipt bytes were preserved.
Separate native transactions committed readiness and final cutover, replayed both
receipts, and exported five new native record kinds. Existing policy export parts
retain exact bytes, actual policy scope, `policy.read` permission and opaque
`retained_bytes` representation; cutover digest links reuse them. Its SHA256 is
`9784636ff7ca7408e5460e747dc33ef5e2ecd2a956430ea4e85b59eea66b4d63`.

The test creates a real local authenticated user/session and consumes actual
pinned-runtime outputs, but deliberately supplies synthetic GitHub admission and
source observations to exercise the trusted native receiver. It proves native
behavior, real HTTP authorization and wire shape; it is not real GitHub deployment
admission, controller credential-isolation proof or provider/native race acceptance.
Those remain separate required T3 checks. No production identity, credential or
repository content is retained here.

## Repository inspection examples

`repository-inspection-examples.json` retains real `adoc repository inspect`
stdout values (re-indented) for configured, no-config (generated profile),
invalid-config, empty and malformed fixture commits with fixed commit dates.
Test identities only. Receipts are read-only evidence and grant no import,
registration or promotion authority.
