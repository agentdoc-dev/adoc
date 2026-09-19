# Security qualification

Engineering review performed on the launch branch on 2026-09-18/19. This is
source review and executable regression evidence, not independent certification
or a guarantee that the project contains no vulnerabilities. See [SECURITY.md](../../SECURITY.md)
for the operating trust boundary and private reporting route.

## History and dependency checks

Gitleaks 8.30.1 scanned all fetched reachable history in a non-shallow clone:
1,315 commits at the recorded scan snapshot. All 92 historical findings matched
previously adjudicated fixture metadata. The current-tree scan returned 49
findings: 46 fixture identifiers and three public model checksums. No exposed
credential was confirmed. Scanner exit code 1 means findings were emitted; it
is not represented as a clean scanner exit. Raw redacted reports, ref inventory
and individual dispositions remain in the local audit trail.

Deleted or inaccessible remote refs are not covered. Automated history scanning
was supplemented with manual review of security-relevant introduction commits;
every historical revision and third-party dependency source was not manually
reviewed. Later documentation-only commits are outside that history count.

The lockfile updates address the identified crossbeam-epoch, h2 and rustls
advisories and a yanked der version. License, source and dependency-ban checks
passed. The transitive `paste` 1.0.15 unmaintained advisory
[RUSTSEC-2024-0436](https://rustsec.org/advisories/RUSTSEC-2024-0436.html) remains;
no advisory suppression was added. The advisory check therefore remains nonzero.

## Repairs and evidence

The review produced repairs for atomic file permission preservation, review-date
overflow, semantic completion/request binding, referenced-evidence obligations
and duplicate evidence kinds, assessment source containment, documentation-root
policy changes, colliding object/policy obligation identifiers, and dangling-link
output containment, receipt input/output alias protection, atomic receipt output,
and diagnostic Markdown escaping. Regression checks reproduced each behavior before repair.
Independent Codex and Claude reviews covered the affected repairs. The assessment
change also updates its experimental v0 schema and tests a real producer envelope
against that schema.

Coverage records distinguish full production-code reads, structural inspection of
static registries/layouts, automated scans, and untouched material. They are not
a claim that every comment, generated fixture, historic document or dependency
implementation received line-by-line security analysis.

## Remaining boundaries

- CLI/MCP runs with the operator's OS permissions. Project path checks do not
  protect against a concurrent process with the same account replacing files.
- Some local inputs are loaded into memory without a size ceiling. Very large
  or adversarial repositories can exhaust resources; use OS resource isolation
  when evaluating untrusted material.
- Plain/styled terminal output can contain authored control characters. Use JSON
  output and a viewer that escapes controls when inspecting untrusted text.
- Semantic executor/context commands own their explicitly supplied output paths
  and remove stale outputs on failure. Use disposable artifact paths, never source
  documents, for those outputs.
- Review obligations are scoped rules, not a complete security policy. Creating
  or retyping an agent instruction does not necessarily emit the same obligation
  as changing an existing instruction's trust value; authored instructions grant
  no runtime privileges.
- Semantic context producers must be trusted to report source availability and
  completeness. A self-consistent context digest does not independently prove
  those facts; callers own the validation basis.
- Artifact hashes establish consistency, not the identity of a trusted producer.
  Use artifacts generated from the intended source and policy.

These limits remain visible rather than being replaced by broad security claims.
Detailed local evidence is retained under `.git/agentic-workflow/hn-launch-qualification/`.
