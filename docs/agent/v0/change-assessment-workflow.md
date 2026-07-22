# AgentDoc Local Change Assessment Workflow

V9.2.1 provides the report-only `adoc.change_assessment.v0` workflow for a repository that keeps AgentDoc knowledge beside its code. Run:

```text
adoc assess-changes --base main --as-of 2026-07-22 --format json
```

Use `--head <commit>` for an immutable comparison, as a CI adapter will. Omit it for local worktree assessment; the envelope records the current HEAD and `worktree_state: clean|dirty`.

A pull-request adapter must use the event's exact base and head commit SHAs,
not branch names or GitHub's synthetic merge checkout, and must capture one UTC
date for the whole run:

```text
adoc assess-changes \
  --base 0123456789abcdef0123456789abcdef01234567 \
  --head 89abcdef0123456789abcdef0123456789abcdef \
  --as-of 2026-07-22 \
  --format json
```

Hash the exact JSON bytes only after validating the schema, legal
completeness/outcome tuple, evaluation date, and requested/comparison/head
commits. Keep GitHub metadata outside this envelope. The AgentDoc GitHub Action
places the validated assessment beside an `adoc.pr_assessment_receipt.v0`
wrapper and exposes both paths and SHA-256 digests; the consuming workflow
chooses whether and how long to retain them.

The command resolves the requested refs, requires one merge base, materializes immutable snapshots, loads each snapshot's own `agentdoc.config.yaml`, and compiles with the same explicit evaluation date. The comparison-base configuration is effective for the current change. Head exclusions and output changes are prospective, reported in `policy_changes`, and cannot hide code introduced by the same pull request.

Missing Git repository context or an unavailable mutable-worktree status emits a structured `error/not_evaluated` envelope with `assessment.snapshot_failed` and exits 2.

Interpret outcomes as follows:

- `pass`: complete and empty or fully excluded, with no deterministic review signal; this is not a semantic correctness claim.
- `review_required`: linked authoritative knowledge, knowledge changes, policy changes, lifecycle signals, reviewers, or proof obligations require a human decision.
- `uncovered`: at least one path is uncovered or linked only to provisional knowledge.
- `invalid`: head configuration or AgentDoc Source is invalid.
- `not_evaluated`: the changed set or required comparison facts were unavailable.

Do not infer missing facts from empty arrays. Check each section's availability marker. Treat body-free deletion tombstones as review inputs: deleting authoritative knowledge cannot delete its owner, authority, source, hash, or proof obligation from the assessment.

The optional configuration is:

```yaml
assessment:
  exclude_paths:
    - vendor/
    - generated/
```

Entries are exact files or directory prefixes ending in `/`; there are no globs. AgentDoc Source, `agentdoc.config.yaml`, and comparison-base generated outputs have higher exclusion precedence. `adoc init` intentionally does not create this optional block.

Invalid exclusion entries emit `assessment.invalid_config_path`; they never collapse into a generic successful or empty assessment.

Required reviewer identities come only from Knowledge Object metadata. Policy changes retain a human-review obligation for `agentdoc.config.yaml`; use repository CODEOWNERS to select the responsible reviewer.

There is no assessment MCP tool in V9.2.1. Agents and adapters may read the published workflow and schema resources; CI invokes the CLI and consumes its JSON envelope.
