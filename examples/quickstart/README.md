# Quickstart fixture

This synthetic draft refund policy is the CLI and MCP smoke-test fixture.
The release qualification exercises it on native Windows, macOS and Linux.

Run from the repository root after building both executables:

```sh
python3 scripts/smoke-test.py --bin-dir target/debug
```

The default smoke run needs no embedding-model download. Add `--embeddings`
to also check FastEmbed build and semantic retrieval. The smoke test creates
its own disposable project and leaves this source fixture unchanged.
