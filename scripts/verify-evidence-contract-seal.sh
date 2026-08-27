#!/usr/bin/env bash
set -euo pipefail
export TZ=UTC

signer_digest=59e881f6d709d3e0a526bb4dbd320312fc4a7b27
rekor_timestamp=2026-08-27T02:55:45Z

for contract in docs/pilots/g1a/evidence-contract-v*.yaml; do
  gh attestation verify "$contract" \
    --repo agentdoc-dev/adoc \
    --signer-workflow agentdoc-dev/adoc/.github/workflows/evidence-contract-seal.yml \
    --signer-digest "$signer_digest" \
    --source-digest "$signer_digest" \
    --source-ref refs/pull/182/merge \
    --format json |
    jq -e --arg timestamp "$rekor_timestamp" '
      length > 0 and any(.[].verificationResult.verifiedTimestamps;
        any(.[];
          .type == "Tlog" and
          .uri == "https://rekor.sigstore.dev" and
          .timestamp == $timestamp
        )
      )
    ' >/dev/null
done
