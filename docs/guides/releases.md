# Versions and releases

## Choose a version

This checkout is the **0.4.0 source preview**, not a published 0.4.0 release.
Build the CLI and MCP gateway from the same checkout. The current source emits
`adoc.graph.v6` and `adoc.search.v2`.

The latest published release at this update is
[v0.3.4](https://github.com/agentdoc-dev/adoc/releases/tag/v0.3.4), with Linux CLI
archives only. It emits graph v5/search v1. Use
[its versioned README](https://github.com/agentdoc-dev/adoc/blob/v0.3.4/README.md)
when running that version. Do not mix its artifacts with a gateway built from
current source. See [migration to graph v6](graph-v6-migration.md) before upgrading.

Version numbers for roadmap milestones are not CLI release versions. The
[product index](../product/README.md) distinguishes accepted direction from
shipped behavior; it is not a release manifest.

## Candidate dependency baseline

The launch branch carries the OpenSSL 0.10.81 / openssl-sys 0.9.117 lockfile update
from [PR #244](https://github.com/agentdoc-dev/adoc/pull/244), original commit
`d05390ee0939879a7616ccece219b502c050c8f8`. This removes the vulnerable OpenSSL
version from the candidate's Linux dependency graph for
[GHSA-phqj-4mhp-q6mq](https://github.com/advisories/GHSA-phqj-4mhp-q6mq).
It does not change an already-published release or close the default branch's
Dependabot alert before merge. No reachable exploit in AgentDoc was established
by the launch audit.

## Verification is version-specific

Release archives have companion SHA-256 files. The current release workflow also
produces provenance attestations; verify an attestation for the exact archive
before relying on that guarantee. Historical v0.3.4 checksum verification passed
on 2026-09-18, but its attestation lookup returned HTTP 404.

Do not treat current workflow code, passing main CI, or a roadmap completion as
proof that an older downloadable archive was built with those checks.

## Release qualification

Pull requests run core CI. Merge-queue candidates additionally build and smoke
Linux x64/ARM64 and Apple Silicon macOS packages and run Windows source-install
qualification against the candidate SHA. Manual native workflows remain available.
Only version-tag pushes publish release assets. Intel macOS is deferred for launch.
