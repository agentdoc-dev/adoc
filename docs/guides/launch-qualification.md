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

## Intel macOS Cargo installation prerequisite

The default `cargo install` path cannot obtain ONNX Runtime 1.28.0 for
`x86_64-apple-darwin`: the resolved `ort-sys 2.0.0-rc.13` distribution has no
Intel macOS runtime. Before installing on Intel macOS, build the pinned
upstream source commit `da9b5e364c465de65c49d91e696cd6485270757f` with
`scripts/qualification/build-intel-runtime.sh`, then retain its staged `lib`
directory. Install and link dynamically with (repeat for `crates/adoc-mcp`):

```sh
ORT_LIB_LOCATION=/path/to/lib ORT_PREFER_DYNAMIC_LINK=1 \
  RUSTFLAGS="-C link-arg=-Wl,-rpath,@executable_path/lib" \
  cargo install --path crates/adoc-cli --locked --root "$ADOC_INSTALL_ROOT"
```

The installed executable also needs an `@executable_path/lib` runtime search
path and the staged `libonnxruntime*.dylib` files (including symlinks) in `$ADOC_INSTALL_ROOT/bin/lib`. The release workflow performs those packaging steps; plain Cargo install
does not. This remains unverified on a hosted Intel native build.


## Pristine Ubuntu installation (HN-M4.T4)

On 2026-09-18, a fresh QEMU VM booted the official Ubuntu 24.04.5 ARM64 cloud
image (kernel 6.8.0-139-generic). Image SHA-256:
`7b682958a67ff5de068e36de6af8b75fa645d296af5a70d6500527f6a33781db`.
It had four virtual CPUs, 8 GiB RAM and a fresh 35 GB copy-on-write disk.
No host directory, Cargo cache, model cache or host credentials were mounted.
An ephemeral SSH key was used only to administer this disposable VM.

Source was `2bf8f2819e0ed6ea16274a6d24fe5f5fa8afe229` with the reviewed
Cargo.lock overlay (SHA-256
`8691ad8218e4a398be7cb5844006b298693a8bb2b71ffb4a3bb4c16d243488c5`).
Bootstrap installed `build-essential pkg-config libssl-dev curl ca-certificates
git python3`, then Rust 1.95.0 minimal from the official rustup installer.
Both default release-profile Cargo path installations succeeded.

The first combined harness run failed because an upstream model warning on
stderr was incorrectly included in JSON parsing. After the smoke harness was
corrected to parse stdout only, both the ordinary and real-embedding journeys
passed. With network disabled at the VM's virtual network device, the ordinary
CLI/MCP journey passed again. Network was then restored.

| Installed executable | SHA-256 |
|---|---|
| adoc | `561087b6bb8e9c023bdbe9300d62aa900269e06969a517e68ffc2855a81eb9a2` |
| adoc-mcp | `ad71772521d658e598b6aa48a17359f068971a5a6aafbecf8b99eb7ae9e73acc` |

This proves a pristine Ubuntu ARM64 installation for that source/lockfile pair.
It does not establish pristine macOS or Windows setup. A subsequent installation
of the repaired candidate in the same VM is a warm regression check; its cached
compiler/model state must not be presented as another pristine installation.

To reproduce the install inside a newly created Ubuntu VM, install the listed
packages and Rust toolchain, copy the selected checkout, and run the two Cargo
installation and smoke commands above. Use an empty HOME/model cache initially;
disable networking only after dependency installation and model provisioning.
The official image, checksum, boot arguments, bootstrap/install transcripts and
network-off result are retained in the local qualification audit trail.
