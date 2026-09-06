# E6.1.T3 frozen-source performance evidence

Status: **required cumulative budget PASS; coarse timing gate PASS; preregistered
51-pair incremental experiment PASS in all 32 cells.** The two earlier 15-pair
runs remain recorded as failures: initial 30/32, controlled rerun 31/32. This
report and its evidence are frozen. No production optimization or threshold
waiver was used. Older pre-final production snapshots and their historical passes
are superseded; all three runs on the final production executable are retained.

| Comparison | Cells | Observed maximum | Unchanged limit | Result |
| --- | ---: | ---: | ---: | --- |
| Cumulative versus pre-T1, no policy | 16 | 1.0678058285 paired ratio | 1.10 each cell | PASS |
| Incremental versus T1/T2 baseline, `run=initial` | 32 | 1.1242024368 paired ratio | 1.10 each cell | 30 PASS / 2 FAIL |
| Incremental, `run=controlled_rerun` | 32 | 1.1072522976 paired ratio | 1.10 each cell | 31 PASS / 1 FAIL |
| Incremental, `run=precision_51` | 32 | 1.0814359749 paired ratio | 1.10 each cell | 32 PASS |
| Hidden-present/absent coarse timing | 30 | 0.154 ms absolute paired median | 1 ms each cell | PASS |
| Interleaved absent/absent controls | 30 | 1.426 ms p90 absolute gap | 3 ms each cell | PASS |

No threshold or timing requirement is waived. No production source changes are
made in response to these measurements. Both failed incremental runs remain
visible; no samples or runs have been discarded to obtain a passing result.

## Frozen source and executables

Candidate base: `1b452e60eeba0010a28eb5d9f6a97d1a65b2195a` (reviewed Adoc208), plus the
bound tracked patch and two listed untracked test sources. Incremental baseline:
`0b69f1ed9bb27a370a2cadfe119c81b63d6036db`; cumulative baseline:
`e1b74a3f579933e3f8194c6c998f91a24578e9db` (pre-T1). The incremental baseline
already contains T1/T2 retrieval behavior and cannot establish the cumulative
budget by itself.

Portable provenance: [complete bindings JSON](e6-1-t3-bindings.json). The current
PR source, stated base and digests bind the measured revision; the raw patch and
498-entry listing need not be committed.

Local-only evidence directory: `/tmp/adoc-e6-1-t3-perf.ivJR4B/`.
All `final-*.log`, `precision-incremental.log`, `final-startup-control.json`, `final-bindings.json`,
`final-source.patch` and `final-source-sha256.txt` references below are local-only.
The listing binds 498 Cargo/config/source/test/example files, including the
untracked tests; the patch alone is not a complete source binding. Report/JSONL
updates after timing do not change the measured production source or executable.

Reproduce the complete listing digest from the repository root at the frozen
revision. Enumerate tracked and nonignored untracked files under `crates/`,
`examples/` and the three root config files, deduplicate and sort repository-relative
UTF-8 path bytes. Each listing line is lowercase SHA-256, two spaces, path, newline;
hash the complete concatenated listing. This command reproduces **498** entries
and the current precision-run digest `5bc34d…` without writing the listing.
The original 15-pair listing digest `091706…` remains a historical binding:

```sh
python3 - <<'PYTHON'
import hashlib, subprocess
from pathlib import Path
paths = sorted(set(subprocess.check_output([
    "git", "ls-files", "--cached", "--others", "--exclude-standard", "-z", "--",
    "crates", "examples", "Cargo.lock", "Cargo.toml", "rust-toolchain.toml",
]).split(b"\0")) - {b""})
listing = b"".join(hashlib.sha256(Path(p.decode()).read_bytes()).hexdigest().encode()
                   + b"  " + p + b"\n" for p in paths)
print(len(paths), hashlib.sha256(listing).hexdigest())
PYTHON
```

| Binding | SHA-256 |
| --- | --- |
| Candidate executable | `e98bedabf6d1be57cc85ae87857f8c8d412562a23c3ead0b1b4194373d8c2f56` |
| Incremental baseline executable | `ea5f25fd63a56a878f6accca63567ce85e46ca24e7b5307629428b28ec390451` |
| Pre-T1 baseline executable | `9b4ad1331514d79f5c772ad3ba2ad73cbcd2c25bf828afe8fa2f6b71b6476d6f` |
| Original 15-pair source patch over candidate base | `26167d73d7b776fa6c2abccadf38a69c70c2215856379f733b8e07c0957df7e5` |
| Original 15-pair source listing (498 entries) | `0917068390caa50bc724f0dfb43952132bcc62df8c00f5fff347de91e830e00b` |
| Cargo.lock | `dc18161fc86bf582b4634c7c0c201e222de48d60b340e4abf2b76397be970bca` |
| Original 15-pair release gate retrieval_pilot.rs | `ee1c89fc45f86efae3e2c99522846e211f4c4c1108a52a7f4f034ee2e1cc7374` |
| Precision source patch over candidate base | `dc5bd48b3ab4b558687b4be9e2ea8c8efb21240d39ffbacf872ac76a7efa7cab` |
| Precision source listing (498 entries) | `5bc34d5c3e2f954b40cec42cc128395e2c761878a151fa523498c90c95e837d3` |
| Precision 51-pair release gate retrieval_pilot.rs | `36d389dcff0285cc9689f70a8aa7d9dad68a3e2f269ca39ec12aacefb7b1ec21` |
| Parity CLI source (untracked in snapshot base) | `753431fd7eb5b427d4e10227ddaca6522fd1d4003925eaad0a71adb490b31048` |
| Local graph signals source (untracked in snapshot base) | `6feb1105f564789bd1e92f57aa7891e27e8c624900df25d80ad27081ed72e0ff` |

The bindings JSON retains the original values and adds `precision_experiment`
for the new runner, listing and patch. Comparing all 498 listing entries changes
only `crates/adoc-cli/tests/retrieval_pilot.rs`: 15 to 51 measured pairs and the
explicit pair-count metadata. Production source and the `e98bed…` executable
are unchanged. The cumulative and coarse results therefore still bind the
current production executable.

Toolchain: Rust 1.95.0 (`59807616e`, 2026-04-14), default release profile,
locked dependencies; Darwin 25.5.0, ARM64. Build evidence:
`final-build.log`,
`final-binary-build.log`.
The final release pilot quality run passed six tests with the performance gate
ignored: `final-quality.log`.

## Method and explicit compatibility change

Both revisions read the same untouched freshly built pilot artifact paths/bytes:
billing has 49 nodes, Markdown 156, all unclassified. The existing build helper
uses deterministic embeddings. Corpora, queries, ranking, policy cases and
thresholds are unchanged. Eight operations run: lexical/semantic/hybrid search,
why, graph (both directions), stale, contradictions and impacted-by. Incremental
measurement includes no policy and an explicit public policy with no exclusions;
pre-T1 measurement includes only no-policy cases.

Per cell, three warmup pairs precede 15 measured pairs in the initial,
controlled-rerun and cumulative runs, or 51 in the preregistered precision
experiment. Order alternates AB/BA; each side averages five fresh CLI invocations.
The gate uses the median of that run's 15 or 51 candidate/baseline ratios, not
the quotient of separately computed medians.
Startup, config/artifact I/O, filtering, query, serialization and process exit are
included; build and output comparisons are outside timing. This is end-to-end
CLI evidence, not isolated predicate cost.

Legacy diagnostics are **not** byte-compatible. For Markdown only, the gate
requires the baseline's complete diagnostics array to equal the carried graph
array, including messages, spans, help and order, with exactly eight warnings:
four `compat.unknown_extension`, two `compat.raw_html_quarantined`, one
`compat.unsafe_link_dropped`, one `compat.unsafe_image_src_dropped`. The current
array must be empty. Existing pretty serialization constructs the exact
indented top-level field; exactly one occurrence is replaced. Every other stdout
byte, exit status and stderr byte must match. Empty-baseline operations retain
raw equality. Each revision's timed repetitions match its own validated
preflight output byte-for-byte. JSONL `compatibility_delta` records `none` or
`markdown_carried_compat_warnings_removed`; no generic diagnostic filtering is
allowed. These timings include the intended removal of diagnostic handling.

## Retained failures and preregistered precision experiment

[Incremental JSONL](e6-1-t3-performance.jsonl) retains all 32 initial cells as
`run=initial`, followed by all 32 controlled-rerun cells as
`run=controlled_rerun`, then all 32 precision cells as `run=precision_51`.
All 64 earlier rows were preserved byte-for-byte when appending the precision
experiment. Every row retains all 15 or 51 base/head batch samples and ratios;
precision rows also emit `measured_pairs=51` and `warmup_pairs=3`.
[Cumulative JSONL](e6-1-t3-total-performance.jsonl) retains all 16 cumulative
cells, also labelled `run=initial`. Source logs:
`final-incremental.log`, `final-incremental-controlled-rerun.log`,
`precision-incremental.log`, `final-cumulative.log`.

| Run | Pilot | Policy | Operation | Base median ms | Candidate median ms | Paired ratio | Result |
| --- | --- | --- | --- | ---: | ---: | ---: | --- |
| initial | billing-pilot | none | stale | 7.736167 | 8.654017 | 1.1242024368 | FAIL |
| initial | billing-pilot | permissive | impacted-by src/billing.rs | 6.213292 | 6.870233 | 1.1020770566 | FAIL |
| controlled_rerun | markdown-pilot | permissive | contradictions --all | 7.673900 | 8.292075 | 1.1072522976 | FAIL |

The stale cell exceeds 1.10 in 9/15 pairs; impacted-by in 8/15. These failures
cannot be dismissed as one outlier. A 49.23 ms versus 12.37 ms pair occurred in a
passing lexical cell. The same candidate's billing stale median was 8.654 ms in
the incremental run and 6.373 ms in the cumulative run, indicating substantial
run variability. `final-startup-control.json`
measured baseline/saved-candidate paired ratio 1.00017 and identical-binary
saved/target ratio 1.02958; these do not isolate config or retrieval cost.

The one controlled full 32-cell rerun completed with exit 101: **31/32 passed**,
with maximum paired ratio **1.107252297636159** for Markdown, permissive policy,
`contradictions --all`. The candidate hashes at both executable paths were
confirmed unchanged by the parent. The two original failing cells passed in
this run; that does not erase the original failures or make the second run pass.

The failures moved between shared graph-query operations, and the 15-pair
estimates had broad uncertainty. Before taking further samples, the parent
preregistered one complete 32-cell experiment with 51 measured pairs to reduce
that uncertainty. It retained three warmups, five invocations per side, AB/BA
ordering, frozen production/baseline executables, corpora, output checks and the
1.10 per-cell threshold. The decision required stopping publication if this
experiment failed; it did not permit further sampling until a pass appeared.

The experiment completed with exit 0: **32/32 passed**, maximum paired median
ratio **1.081435974866282**, for billing, permissive policy, `graph` in both
directions. Every cell has exactly 51 samples per side and 51 paired ratios.
This is the outcome of the fixed higher-precision plan, not selection of the
best of repeated 15-pair runs. Both earlier failures remain failures, without
sample removal, outlier trimming or waiver. No confidence interval substitutes
for the unchanged observed-median gate. No further rerun or production change
is needed for this completed experiment; no isolated-cost or universal latency
claim follows.

## Coarse hidden-presence timing

The same frozen candidate passed all 30 cells in
`final-coarse-timing.log`.
The fixture has two public and three hidden nodes, under default-public and
explicit-public policies. Three warmup pairs precede 31 alternating
hidden/absent pairs, interleaved with absent/absent controls. Fixture writes and
output assertions are outside timing; complete stdout/stderr/status parity is
checked. Limits remain absolute paired median gap ≤1 ms and control p90 ≤3 ms.

Values below are the logged millisecond precision. D = default-public;
E = explicit-public; gap is the signed present-minus-absent paired median.

| Operation | Format | D gap | D control p90 | E gap | E control p90 |
| --- | --- | ---: | ---: | ---: | ---: |
| `search credits --lexical` | plain | 0.069 | 0.183 | -0.024 | 0.965 |
| `search billing.internal --lexical` | json | 0.086 | 1.231 | 0.025 | 0.845 |
| `search credits` | styled | 0.044 | 0.797 | 0.063 | 0.578 |
| `search credits --semantic` | json | 0.082 | 1.172 | 0.088 | 0.282 |
| `search credits --lexical --related-to billing.root` | json | 0.137 | 0.764 | -0.012 | 0.484 |
| `why billing.root` | json | 0.014 | 0.952 | 0.039 | 0.549 |
| `why billing.root` | plain | 0.036 | 0.957 | 0.045 | 0.195 |
| `why billing.root` | styled | 0.046 | 0.425 | 0.034 | 0.593 |
| `why billing.internal` | styled | 0.077 | 0.552 | 0.085 | 1.426 |
| `graph billing.root --direction incoming` | plain | 0.154 | 0.308 | 0.044 | 0.205 |
| `graph billing.root --direction outgoing` | json | 0.029 | 0.652 | 0.034 | 0.593 |
| `graph billing.root --direction both` | styled | 0.152 | 0.770 | 0.118 | 0.572 |
| `stale` | json | 0.128 | 0.930 | 0.117 | 0.661 |
| `contradictions --all` | plain | 0.106 | 0.484 | 0.072 | 0.791 |
| `impacted-by src/billing.rs` | markdown | 0.105 | 0.640 | 0.033 | 0.538 |

This is bounded coarse evidence, not constant-time behavior or coverage of cold
starts, unbounded hidden corpora, fine statistical attacks, or successful
semantic/model loading; the semantic timing case is a refusal. No RT-08 waiver
is asserted.

## Reproduction

Build isolated baseline and candidate snapshots with the same release toolchain,
features and lockfile. Preserve the executables and verify the bindings before
and after measurement; do not rebuild during timing. From the candidate snapshot:

```sh
ADOC_RETRIEVAL_BASELINE_BIN=/absolute/path/to/saved/base-adoc \
  cargo test --release -p adoc-cli --test retrieval_pilot --locked \
  permission_retrieval_paired_release_gate -- --ignored --exact --nocapture --test-threads=1
```

The current runner performs 51 measured pairs. To reproduce the retained
15-pair runs exactly, restore only the documented 54-to-18 loop bound,
51-to-15 comment and remove the added pair-count metadata, then verify its original
`ee1c89…` digest; do not reinterpret the 16-cell cumulative evidence as 51-pair
measurements. Cumulative comparison uses the saved pre-T1 executable and
`ADOC_RETRIEVAL_NO_POLICY_ONLY=1`. For the separate coarse gate:

```sh
ADOC_PARITY_TIMING_BINARY=/absolute/path/to/saved/final-adoc \
  cargo test --release -p adoc-cli --test permission_parity_cli --locked \
  coarse_hidden_presence_timing_preserves_complete_observable_parity -- --ignored --exact --nocapture --test-threads=1
```

The reusable tests cannot infer binary provenance: the caller binds baseline
revisions, source and executable digests in the report/snapshot; no
host-specific binary hashes are embedded in the tests.
