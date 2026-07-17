# CI Integration

Run AgentDoc on every pull request: Strict Mode validation, impacted
knowledge, and proposed new Knowledge Objects, posted as one in-place-updated
**Review Report** comment.

## GitHub Actions (recommended)

```yaml
name: AgentDoc PR Report
on: pull_request
permissions:
  contents: read
  pull-requests: write   # sticky comment; omit → job-summary-only mode
jobs:
  report:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
        with:
          fetch-depth: 0   # required for the Impacted Query (--ref)
      - uses: agentdoc-dev/action@v1
```

Start in the default `advisory` mode; flip to `enforcement: strict` after a
clean week. See the [action's README](https://github.com/agentdoc-dev/action) for
all inputs (`enforcement`, `scope`, `adoc-version`, `working-directory`,
`github-token`), fork-PR behavior, and security notes.

This repository's own `.github/workflows/adoc-pr.yml` is the continuously
tested copy of this snippet.

## Appendix: raw workflow (non-GitHub CI, GitHub Enterprise)

Where the Marketplace action is unavailable, run the same commands directly.
The action is a thin wrapper around exactly this sequence:

```sh
# install a released binary (or: cargo install --path crates/adoc-cli --locked)
curl -fsSLO https://github.com/alex-bako/adoc/releases/download/v0.1.0/adoc-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
curl -fsSLO https://github.com/alex-bako/adoc/releases/download/v0.1.0/adoc-v0.1.0-x86_64-unknown-linux-gnu.tar.gz.sha256
sha256sum -c adoc-v0.1.0-x86_64-unknown-linux-gnu.tar.gz.sha256
tar -xzf adoc-v0.1.0-x86_64-unknown-linux-gnu.tar.gz

# validate and query (from the directory holding agentdoc.config.yaml)
./adoc build --no-embeddings
./adoc check --format markdown > check.md        # exit 1 on errors = your gate
./adoc impacted-by --ref origin/main --format markdown > impacted.md

# post/update one PR comment keyed on the marker
printf '<!-- adoc:pr-report -->\n## AgentDoc Review Report\n\n' > report.md
cat check.md impacted.md >> report.md
# upsert report.md as a comment with your CI's API of choice,
# matching an existing comment whose body starts with the marker
```

`adoc check --format markdown` writes the comment body to stdout and
`file:line:col: severity[code] message` diagnostics to stderr; the exit code
(0 clean / 1 errors) is identical across formats.
