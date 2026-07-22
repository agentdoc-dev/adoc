# CI Integration

Run AgentDoc on every pull request to assess the exact requested base, merge
base, and head revisions. The Action posts one in-place-updated **AgentDoc PR
Report** and exposes the deterministic assessment plus an Action-owned receipt.

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
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7
        with:
          fetch-depth: 0
          persist-credentials: false
      - id: agentdoc
        uses: agentdoc-dev/action@9aed946ec7c6c29edb1fbd784d04833d7383c2e1 # v1.5.1
        with:
          adoc-version: v0.3.1
          enforcement: advisory
          propose: false
      - name: Verify assessment evidence
        if: always()
        shell: bash
        env:
          ASSESSMENT_PATH: ${{ steps.agentdoc.outputs.assessment-path }}
          ASSESSMENT_SHA256: ${{ steps.agentdoc.outputs.assessment-sha256 }}
          RECEIPT_PATH: ${{ steps.agentdoc.outputs.assessment-receipt-path }}
          RECEIPT_SHA256: ${{ steps.agentdoc.outputs.assessment-receipt-sha256 }}
        run: |
          set -euo pipefail
          test -n "$RECEIPT_PATH" && test -f "$RECEIPT_PATH"
          test "sha256:$(sha256sum "$RECEIPT_PATH" | cut -d ' ' -f 1)" = "$RECEIPT_SHA256"
          jq -e '.schema_version == "adoc.pr_assessment_receipt.v0"' "$RECEIPT_PATH" >/dev/null
          if [ "$(jq -r .run_status "$RECEIPT_PATH")" = completed ]; then
            test -n "$ASSESSMENT_PATH" && test -f "$ASSESSMENT_PATH"
            test "sha256:$(sha256sum "$ASSESSMENT_PATH" | cut -d ' ' -f 1)" = "$ASSESSMENT_SHA256"
            test "$(jq -r .assessment.sha256 "$RECEIPT_PATH")" = "$ASSESSMENT_SHA256"
          else
            jq -e '.run_status == "failed"' "$RECEIPT_PATH" >/dev/null
            test -z "$ASSESSMENT_PATH" && test -z "$ASSESSMENT_SHA256"
          fi
      - name: Retain assessment evidence
        if: always() && steps.agentdoc.outputs.assessment-receipt-path != ''
        uses: actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a # v7.0.1
        with:
          name: agentdoc-${{ steps.agentdoc.outputs.assessment-invocation-id }}
          path: |
            ${{ steps.agentdoc.outputs.assessment-path }}
            ${{ steps.agentdoc.outputs.assessment-receipt-path }}
          if-no-files-found: error
```

The Action owns `adoc.pr_assessment_receipt.v0`; the caller owns retention.
The receipt references the adjacent `adoc.change_assessment.v0` artifact by
SHA-256, and the Action exposes both paths and digests. Repository artifact
retention is bounded and deletable; it is not an organization-wide audit
store.

Start in `advisory` mode; flip to `enforcement: strict` after a clean week.
Keep measured workflows pinned to immutable Action and AgentDoc versions. See
the [action's README](https://github.com/agentdoc-dev/action) for all inputs,
outputs, fork-PR behavior, and security notes.

This repository's own `.github/workflows/adoc-pr.yml` is the continuously
tested copy of this snippet.

## Appendix: raw workflow (non-GitHub CI, GitHub Enterprise)

Where the Marketplace action is unavailable, run the compiler assessment
directly. This produces `adoc.change_assessment.v0`; it does not recreate the
GitHub-specific Action receipt.

```sh
# install a released binary (or: cargo install --path crates/adoc-cli --locked)
curl -fsSLO https://github.com/agentdoc-dev/adoc/releases/download/v0.3.1/adoc-v0.3.1-x86_64-unknown-linux-gnu.tar.gz
curl -fsSLO https://github.com/agentdoc-dev/adoc/releases/download/v0.3.1/adoc-v0.3.1-x86_64-unknown-linux-gnu.tar.gz.sha256
sha256sum -c adoc-v0.3.1-x86_64-unknown-linux-gnu.tar.gz.sha256
tar -xzf adoc-v0.3.1-x86_64-unknown-linux-gnu.tar.gz

# run from the directory holding agentdoc.config.yaml
evaluation_date="$(date -u +%F)"
./adoc assess-changes \
  --base "$requested_base_sha" \
  --head "$head_sha" \
  --as-of "$evaluation_date" \
  --format json > assessment.json
sha256sum assessment.json
```

The caller must resolve and fetch both exact commits before invoking the
command. Keep the assessment bytes and digest together under the CI system's
retention policy; do not synthesize `adoc.pr_assessment_receipt.v0` outside the
Action.
