# Launch qualification evidence

This is engineering evidence for specific source revisions and environments.
It does not certify all platforms or every security/accessibility property.
The [qualification roadmap](../roadmap/HN-LAUNCH-QUALIFICATION.md) defines the
nine accepted follow-up slices. Implementation is in progress.

## Cargo installation: macOS ARM64 (HN-M4.T1)

On 2026-09-18, source `c7541ae2b258797aa9c5da26daaa3d1c7ce9d5a2`
passed both Cargo path installations with Rust/Cargo 1.95.0 on native
macOS 27.0 ARM64. A detached checkout, empty compiler-output directory and
isolated installation root were used. Cargo's normal registry downloads and
FastEmbed model cache were shared; this was not a pristine OS test.

Commands, from that checkout, with `CARGO_TARGET_DIR` set to the empty build directory:

```sh
cargo install --path crates/adoc-cli --locked --root "$ADOC_INSTALL_ROOT"
cargo install --path crates/adoc-mcp --locked --root "$ADOC_INSTALL_ROOT"
cargo install --list --root "$ADOC_INSTALL_ROOT"
"$ADOC_INSTALL_ROOT/bin/adoc" --version
python3 scripts/smoke-test.py --bin-dir "$ADOC_INSTALL_ROOT/bin"
python3 scripts/smoke-test.py --bin-dir "$ADOC_INSTALL_ROOT/bin" --embeddings
```

All exited zero. Cargo used its default release profile and recorded adoc-cli
0.4.0 and adoc-mcp 0.1.0; the CLI printed `adoc 0.4.0`. The two crate versions
are existing metadata. Both executables came from the same source revision.
The explicit installation paths prevent accidentally testing a global binary.

Offline checks covered initialization/refusal without overwriting source,
strict validation, generated HTML/graph, lexical search, source citations and
broken-reference rejection. MCP initialization, tool discovery, project status,
object retrieval and disabled patch application passed. Actual FastEmbed build
and semantic search passed. A separate retained fixture in a directory with
spaces and Unicode confirmed `adoc.graph.v6` and `adoc.search.v2` explicitly.
The source checkout and Cargo.lock remained unchanged; the installation root
was supplied explicitly throughout, leaving the user Cargo bin directory alone.

| Artifact | SHA-256 |
|---|---|
| Cargo.lock | `1e03c88dc973a14f13422787acb2422f9eac978e4b538cd883b3f31078ccd5f1` |
| Installed adoc | `8a96e988e9dbe01ed15d6db3e3a4b6950a5021f85f852d0cbb9b9ba826e50004` |
| Installed adoc-mcp | `4990df9650be9098e3d3d8b21decced489dc09ad906568f9a4dce5de18dd82b8` |

Local raw commands, exits, metadata, environment and digests are retained under
`.git/agentic-workflow/hn-launch-qualification/install.json` and `install-*.log`.
These local paths are an audit trail, not downloadable release assets.
Subsequent documentation-only planning commit `2bf8f281` does not change these
binary inputs. Native Intel macOS, Windows and clean-OS checks are separate slices.
