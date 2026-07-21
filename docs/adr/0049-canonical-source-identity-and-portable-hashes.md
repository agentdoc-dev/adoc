# ADR-0049: Canonical Source Identity and Portable Hashes

**Status:** Accepted
**Date:** 2026-07-21
**Slice:** V9.1.1

## Context

AgentDoc currently uses one path for three different concerns: reading source
bytes, deriving path-based page identity, and publishing source coordinates.
`FsSourceProvider` therefore lets checkout-specific absolute paths reach
diagnostics and Graph Artifact spans. Those spans participate in Knowledge
Object `content_hash` values. Review hides the absolute path by rebasing it to
the synthetic `<review>` prefix, but that produces a second identity instead
of the same portable identity as a normal build. Review also compiles an
entire temporary worktree rather than the configured documentation tree.

Consequently, identical repository revisions can produce different graph
objects and hashes in another clone or a review worktree. Patch base hashes
from normal builds are not reliably interchangeable with assessment hashes,
and serialized paths can be mistaken for host paths during patch placement.

## Decision

1. A loaded source carries three explicit coordinates:
   - **physical path**: canonical host path used only for filesystem access;
   - **identity path**: documentation-root-relative path used for path-derived
     page IDs;
   - **logical path**: validated project-relative path used in diagnostics,
     Graph Artifact spans, diffs, hashes, and later receipts.
2. Logical paths are non-empty relative UTF-8 paths serialized with `/`.
   Absolute paths, drive prefixes, empty components, `.`, `..`, backslashes,
   NUL/control characters, and leading or trailing whitespace are rejected.
   Failure is explicit; no absolute-path fallback exists.
3. Project-bound `check` and `build` discover `agentdoc.config.yaml`. Their
   physical documentation root is the configured `docs_path`, identity paths
   are relative to that root, and logical paths are relative to the discovered
   project root. A caller-selected file or directory inside that configured
   tree remains project-bound and receives the same coordinates as the
   configuration default.
4. Standalone explicit directory input uses that directory as both invocation
   root and coordinate root. Standalone explicit file input uses its parent;
   the file name is its identity and logical path. Standalone builds publish
   no repository identity.
5. Repository review, diff, and assessment require a discovered project
   configuration. Each snapshot compiles `<snapshot>/<docs_path>` physically
   while retaining the same project-relative logical prefix. The synthetic
   `<review>` coordinate is removed.
6. Patch check/apply resolves serialized logical coordinates beneath the
   validated physical project snapshot. Logical text is never accepted as an
   arbitrary host path, and canonical containment is checked before reads or
   writes.
7. The Graph Artifact contract becomes `adoc.graph.v5`. Every document has a
   required `repository_identity` member:

   ```json
   {
     "repository_identity": {
       "kind": "local_project",
       "config_path": "agentdoc.config.yaml"
     }
   }
   ```

   Project-bound invocations emit that object. Standalone invocations emit
   `"repository_identity": null`. Repository identity describes the artifact
   but does not enter individual Knowledge Object content hashes. Logical
   source path, line, and column remain hash-bearing.
8. Readers exact-match v5. No v4/v5 overlap reader is introduced. Existing
   graph and search artifacts are rebuilt, embeddings are regenerated because
   their graph artifact hash changes, and in-flight patch documents must be
   regenerated when their base hashes no longer match.

## Rejected

- **Hashing absolute paths** — binds otherwise identical knowledge to one
  checkout.
- **Keeping `<review>` rebasing** — hides host paths but still makes review a
  different identity domain.
- **Removing line and column from hashes now** — portability does not require
  it; pilot evidence should justify that separate semantic change.
- **Inferring a repository root in the filesystem adapter** — command
  semantics decide whether an invocation is project-bound or standalone.
- **Treating `\\` as a path separator** — caller-authored logical paths have
  one portable grammar on every platform.
- **Embedding repository identity in each object hash** — it would make equal
  objects differ by invocation family without improving object provenance.
- **A tolerant v4 reader or migration layer** — no current external consumer
  requires an overlap window, and the existing exact-version failure makes
  the one-time rebuild unambiguous.

## Consequences

- Moving a checkout or compiling an unchanged snapshot in a review worktree
  leaves graph objects and Knowledge Object hashes unchanged.
- Moving a source within a repository deliberately changes its source
  coordinate and affected hashes while preserving docs-root-relative page ID
  behavior.
- Configuration and snapshot orchestration must provide coordinate context;
  source adapters no longer guess it.
- Symlinked sources whose canonical target escapes the configured root fail
  before bytes are read. Unsafe logical or patch paths fail with a stable
  diagnostic/error instead of being normalized permissively.
- `adoc.graph.v4` artifacts fail through the existing unsupported-version
  path. The release notes must call out graph rebuild, patch regeneration, and
  re-embedding.

## Relationships

- **ADR-0028 and ADR-0039** define the exact-version Graph Artifact discipline
  continued by v5.
- **ADR-0036** defines patch source-drift behavior; this ADR makes its base
  hashes portable between normal and review builds.
- **ADR-0047** governs Action packaging. The Action does not change in this
  slice and must pin a release containing v5 before publishing hash-bearing
  receipts.
- **ADR-0048** establishes project-root evidence anchoring. Project-bound
  source coordinates use the same discovered configuration root.
