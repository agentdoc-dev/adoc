#!/usr/bin/env bash
set -euo pipefail
export TZ=UTC

signer_digest=ad4d58e167f6723c4eff92f5fa15fb7fb5004931
rekor_timestamp=2026-08-26T23:59:41Z

for contract in docs/pilots/g1a/evidence-contract-v*.yaml; do
  gh attestation verify "$contract" \
    --repo agentdoc-dev/adoc \
    --signer-workflow agentdoc-dev/adoc/.github/workflows/evidence-contract-seal.yml \
    --signer-digest "$signer_digest" \
    --source-digest "$signer_digest" \
    --source-ref refs/pull/178/merge \
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
