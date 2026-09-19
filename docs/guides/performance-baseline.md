# Local performance baseline

Measured on 2026-09-19 with release binaries on macOS 27.0 ARM64, 8 logical
CPUs, Rust 1.95.0 and Python 3.11.6. This was a working development machine;
other applications and review processes were active. Power and CPU frequency
were not controlled. Results are observations, not a service-level guarantee.

Times below are seconds. Each repeated scenario has five samples; the first
computed embedding build has one observation per corpus. The
[raw samples](evidence/performance-2026-09-19.json) include minimum/maximum,
per-command correctness, source and binary digests, and available peak RSS.

| Scenario | 100 objects | 1,000 objects | 10,000 objects |
|---|---:|---:|---:|
| Check | 0.0162 | 0.0270 | 0.1355 |
| Build without embeddings | 0.0161 | 0.0292 | 0.2055 |
| First computed embeddings (one sample) | 0.7401 | 6.4294 | 61.6562 |
| Build with cached embeddings | 0.1907 | 0.2766 | 0.9163 |
| Lexical search | 0.0153 | 0.0267 | 0.1460 |
| Semantic search | 0.1948 | 0.2355 | 0.5249 |
| Hybrid search | 0.1994 | 0.2475 | 0.5459 |
| MCP startup | 0.0125 | 0.0119 | 0.0130 |
| MCP project status | 0.0440 | 0.0980 | 0.6361 |
| MCP evidence lookup | 0.0016 | 0.0113 | 0.1135 |

All retrieval measurements found `billing.refund-window` with a source path and
positive line number. Model: FastEmbed `bge-small-en-v1.5`, 384 dimensions; exact
model cache hashes are retained in the raw data. Provisioning an empty owned
model cache took 5.919 seconds, including initialization and one embedding.
That setup is separate from the corpus timings; it is not download-only time.
The largest observed CLI peak RSS was 1,336,721,408 bytes. Persistent MCP memory
was not measured.

The corpus generator is deterministic and bounded at 10,000 objects per corpus.
Model files are reused after setup; each corpus computes its own embeddings
before cached builds. CLI requests use fresh processes; OS filesystem caches
are not flushed. MCP startup is measured five times, then one persistent
connection serves one warm-up and five measured calls of each tool. No p99,
cache-cold, cross-machine or regression claim follows from these samples.

Reproduce from a checkout after building both release binaries:

```sh
cargo build --release --locked -p adoc-cli -p adoc-mcp
python3 scripts/qualification/benchmark.py --bin-dir target/release \
  --source-revision "$(git rev-parse HEAD)" --sizes 100,1000,10000 \
  --samples 5 --embeddings --output performance.json
python3 -m unittest discover -s scripts/qualification -p 'test_*.py'
```

Use a clean checkout when recording a commit as the binary source. The recorded
run used a frozen working-tree identity, with its patch and all four successful
Rust checks retained in the local qualification audit trail. The raw JSON keeps
that original identity rather than silently relabeling the measurement.

The harness uses Python's standard library and bounds CLI commands at 180
seconds and MCP calls at 30 seconds. It uses a disposable corpus/model cache,
terminates its MCP children and leaves the user's project unchanged. Network is
needed for initial model provisioning. Omit `--embeddings` for the lexical path.
