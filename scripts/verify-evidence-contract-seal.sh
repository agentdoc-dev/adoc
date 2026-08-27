#!/usr/bin/env bash
set -euo pipefail
export TZ=UTC

signer_digest=931dec0dd162ba02494b263520aa42a2d26a5727
rekor_timestamp=2026-08-27T08:41:07+02:00

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
