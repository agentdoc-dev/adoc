#!/usr/bin/env bash
set -euo pipefail
export TZ=UTC

signer_digest=d9eed215042cf31eea101ef005a5ecb78e537ded
rekor_timestamp=2026-08-26T20:50:49Z

for contract in docs/pilots/g1a/evidence-contract-v*.yaml; do
  gh attestation verify "$contract" \
    --repo agentdoc-dev/adoc \
    --signer-workflow agentdoc-dev/adoc/.github/workflows/evidence-contract-seal.yml \
    --signer-digest "$signer_digest" \
    --source-digest "$signer_digest" \
    --source-ref refs/pull/177/merge \
    --format json |
    jq -e --arg timestamp "$rekor_timestamp" '
      length > 0 and all(.[].verificationResult.verifiedTimestamps;
        any(.[];
          .type == "Tlog" and
          .uri == "https://rekor.sigstore.dev" and
          .timestamp == $timestamp
        )
      )
    ' >/dev/null
done
