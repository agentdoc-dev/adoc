#!/usr/bin/env bash
set -euo pipefail
export TZ=UTC

signer_digest=0760ac11bb68e46cd649ce82f0a85c7b46d7e9a5
rekor_timestamp=2026-08-27T01:47:45Z

for contract in docs/pilots/g1a/evidence-contract-v*.yaml; do
  gh attestation verify "$contract" \
    --repo agentdoc-dev/adoc \
    --signer-workflow agentdoc-dev/adoc/.github/workflows/evidence-contract-seal.yml \
    --signer-digest "$signer_digest" \
    --source-digest "$signer_digest" \
    --source-ref refs/pull/180/merge \
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
