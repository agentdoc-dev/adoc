# E6.2.T5 no-visibility compatibility evidence

The candidate and the independently retained pre-E6 implementation produce
byte-identical HTML, graph v6, and search v2 artifacts for the existing
`crates/adoc-cli/tests/fixtures/v0_6/project` corpus. Lexical search and why also
produce identical stdout, stderr, and exit codes on their independently built
artifacts. No output fields, hashes, whitespace, or source coordinates are removed
or normalized.

## Baseline identity

Baseline commit: `e1b74a3f579933e3f8194c6c998f91a24578e9db`, the direct parent of
the first E6.1 implementation. It is an **unpublished pre-E6 implementation** with
graph v6, search v2, and package version 0.4.0. It is not a published release.
The latest published release checked during this slice, v0.3.4, uses graph v5.
The accepted graph-v5 → graph-v6 migration changes schemas, hashes, and Source
Bindings; this evidence does not claim cross-schema byte identity.

The retained baseline executable has SHA-256
`9b4ad1331514d79f5c772ad3ba2ad73cbcd2c25bf828afe8fa2f6b71b6476d6f`.
Its prior committed provenance is in [E6.1.T3 bindings](e6-1-t3-bindings.json) and
[the performance report](e6-1-t3-performance.md). The slice's
[manifest](../../crates/adoc-cli/tests/fixtures/e6_2_compatibility/baseline.json)
binds that commit/binary, all four source files, the fixed build inputs, and all
three raw artifact digests. Only the baseline executable generated the goldens.
The candidate test has no golden-update switch.

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| docs.html | 2,097 | `68643b9ee3cad9263dd78acd7a96f303972ebccdf6669330f86e34dd588fbb25` |
| docs.graph.json | 6,732 | `cd83663141598914df740a8da98591bd79a7787099bda9b42853c173ef25aece` |
| docs.search.json | 36,974 | `5b025e548ad7956f35c6739600ae751c9a556fe2ba53be3f9746536683ec868e` |

JSON goldens use the existing `*.golden.agent.json` naming convention so the
repository's final-newline hook preserves their exact producer bytes. The
manifest explicitly maps artifact names to golden filenames. HTML ends in a
newline; both JSON artifacts end in `}` without a newline.

## Reproduction

Run the normal regression without any historical executable:

```sh
cargo test -p adoc-cli --test e6_2_compatibility --locked
```

The historical gate requires Python 3 and the exact retained executable. Set
`ADOC_RETRIEVAL_BASELINE_BIN` to its absolute path, then run:

```sh
cargo test -p adoc-cli --test e6_2_compatibility --locked \
  independently_built_pre_e6_artifacts_are_byte_identical -- --ignored --exact
```

The gate verifies binary, commit, source, artifact, and filename bindings before
starting either producer. Both builds run in the same temporary non-Git
workspace, reading the same copied `docs` directory and writing separate empty
output directories. Both use `--as-of 2026-09-07` and
`ADOC_TEST_EMBEDDING_PROVIDER=deterministic` (hash-v1, 384 dimensions). Ambient
ADOC_CONFIG, ADOC_AUDIENCE, and ADOC_RETRIEVAL_POLICY are cleared. No prior search
cache is supplied. Each command reads its own producer's artifacts through the
same query path, preserving path-dependent bytes. The normal test also rebuilds
the candidate once into another empty directory and compares all three outputs.

The historical gate is ignored in ordinary workspace tests because the archived
executable is external. It was explicitly executed for T5 and passed. The older
optional performance gates were not rerun or counted as new passes; this slice
does not introduce a timing matrix.

If the retained executable is lost, archive the exact baseline commit, retain its
Cargo.lock, and use the recorded Rust 1.95.0 release toolchain with:

```sh
cargo test --release -p adoc-cli --test retrieval_pilot --no-run --locked
```

This compiles the CLI with its deterministic test-provider feature. Record the
rebuilt binary's own hash, source archive, toolchain, and build log; a different
build directory/environment can change executable bytes. The existing gate
intentionally refuses a binary with another hash. Rebinding a reconstructed
baseline requires reviewed provenance, not a version-string match or replacing
goldens from the candidate.

## Coverage and limits

The reused corpus has multiple pages, a nested file, prose, cross-page Object
References, a relation, verified-claim evidence, and ordinary metadata. It has no
visibility, field_visibility, managed field provenance, or declassification
opt-in. Separate structural assertions check current schemas and absent new
optional fields; they do not replace the raw comparisons. Existing HTML goldens
cover additional syntax and object kinds.

Actual MCP stdio search and why compare their full unclassified structured
payloads with LocalContext and compare the exact text JSON bytes. The fixture
contains a draft object plus prose and has no date-dependent lifecycle fields.
The stdio harness supplies ambient restricted hints, which do not confer
authority. Both responses remain nonempty and omit classification and managed
metadata. JSON-RPC framing is distinct from the retrieval payload.

Evidence starts with a failing normal test for missing historical goldens.
Isolated tampering then proves a changed HTML byte fails the raw comparison and
a changed source binding fails the historical gate. Both mutations are restored;
the final producer gate is run on the unchanged baseline bytes. No production
defect or production change was needed.

Required checks include the historical producer gate, actual stdio suite,
existing CLI unknown-audience/HTML/billing tests, staged prek (formatting,
Clippy, workspace tests and hygiene), workspace build and documentation.
Execution logs and final source/runtime bindings are retained under the
coordinator's `E6.2.T5-*` evidence prefix. Independent review and final run-state
completion belong to the coordinator. E6.3 embedding privacy and authenticated
MCP audit are outside this slice.
