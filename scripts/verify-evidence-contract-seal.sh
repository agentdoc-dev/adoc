#!/usr/bin/env bash
set -euo pipefail
export TZ=UTC

signer_digest=95accb87113acb7a7d764f496ec591c384f690b1
rekor_timestamp=2026-09-01T12:02:19Z

for contract in docs/pilots/g1a/evidence-contract-v*.yaml; do
  gh attestation verify "$contract" \
    --repo agentdoc-dev/adoc \
    --signer-workflow agentdoc-dev/adoc/.github/workflows/evidence-contract-seal.yml \
    --signer-digest "$signer_digest" \
    --source-digest "$signer_digest" \
    --source-ref refs/pull/190/merge \
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
