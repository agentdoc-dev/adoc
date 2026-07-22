# ADR-0050: Local Change Assessment Contract

- Status: Accepted
- Date: 2026-07-22
- Roadmap: V9.2.1

## Context

The existing `diff`, `review`, and `impacted-by` commands answer useful parts of the local change-review question, but no single deterministic artifact binds the Git comparison, effective project configuration, lifecycle evaluation date, changed paths, governing knowledge, validation state, and review requirements. The GitHub Action must not reconstruct those facts independently because that would create a second policy implementation.

V9.2.1 therefore introduces one report-only local command and one experimental envelope. The envelope is designed to be retained and hashed by V9.2.2, but this slice does not add GitHub metadata, a gate, a central service, or an MCP tool.

## Decision

### Command and semantic inputs

The canonical command is:

```text
adoc assess-changes --base <git-ref>
  [--head <git-ref>]
  [--as-of <YYYY-MM-DD>]
  [--format auto|plain|styled|json|markdown]
```

`--base` is required. An explicit head is an immutable commit; an omitted head is the current worktree and records its clean or dirty state. The comparison base is the unique `git merge-base` of the resolved requested base and resolved head commit. Changed paths, the base-side Knowledge Object set, and the base-side configuration all use that same commit.

The evaluation date is a semantic input formatted as `YYYY-MM-DD`. It defaults once to the current UTC date and is then passed unchanged through assessment compilation and every date-sensitive projection. `check`, `build`, `patch --check`, and `patch --apply` accept the same optional input so callers can pin a workflow to one date.

### Authority

Only the following authored kind/status pairs govern a path authoritatively:

| Kind | Authored status |
| --- | --- |
| `claim` | `verified` |
| `decision` | `accepted` |
| `api` | `verified` |
| `policy` | `active` |
| `procedure` | `verified` |

Every other supported pair is provisional. `agent_instruction` can be reported as an informational or provisional linkage, but it never grants runtime authority.

Existing `adoc.review.v0` and `adoc.impacted.v0` behavior remains unchanged. Their historical claim/decision/API authority filter is not widened by this decision.

### Configuration and effective policy

`agentdoc.config.yaml` gains one optional additive block:

```yaml
assessment:
  exclude_paths:
    - vendor/
    - generated/
```

A missing block means an empty list. An entry without a trailing slash is an exact file. An entry ending in `/` is a component-boundary-aware directory prefix. Globs are not supported.

Entries are normalized, sorted, deduplicated, and stripped of entries shadowed by an earlier directory prefix. Empty, dot-only, absolute, drive-prefixed, parent-escaping, backslash-containing, control-character-containing, NUL-containing, or edge-whitespace values are invalid.

The comparison-base policy is effective for the current assessment. The head policy is prospective and is reported separately. Consequently, a pull request cannot hide its own code by adding an exclusion. `adoc init` does not emit the optional block.

### Changed-path classification

Every normalized changed path appears exactly once. The first matching rule wins:

1. An invalid raw Git path makes the whole assessment `error/not_evaluated`.
2. AgentDoc sources from the union of base and head inventories are `excluded` with reason `knowledge_source`.
3. `agentdoc.config.yaml` is `excluded` with reason `configuration`.
4. Generated output from the comparison-base configuration is `excluded` with reason `generated_output`.
5. A comparison-base configured exclusion is `excluded` with reason `configured_exclusion:<entry>`.
6. One or more authoritative exact-path matches make the path `covered`; every match and reason is retained.
7. Only non-authoritative exact-path matches make the path `provisional`; every match and reason is retained.
8. Otherwise the path is `uncovered`.

The only linkage rules in V9.2.1 are exact `impacts:` paths and exact source-code/test evidence paths, including paths resolved through a referenced `source` object. There is no glob, symbol, AST, or model inference.

### Envelope

The schema version is `adoc.change_assessment.v0`. The stable top-level sections are:

- `schema_version`, `completeness`, `outcome`, and `evaluation_date`;
- `snapshots` for requested base, comparison base, and head;
- `knowledge_snapshot` with exact graph and canonical object-set digests;
- `assessment_config` with normalized base/head config and effective/proposed policy digests;
- `summary` and validation attribution counts;
- one classified record per changed path;
- body-free implicated head `objects` and body-free `knowledge_changes`, including deletion tombstones;
- `policy_changes`, assessment-specific reviewers and proof obligations;
- lifecycle, contradiction, anchor, and validation signals;
- stable diagnostics.

Unavailable sections are represented explicitly and are never replaced by a misleading empty collection. Collections are ordered deterministically by their identifying fields.

The envelope contains no timestamp, GitHub repository or actor, raw diff, object body, prompt, or model data. It has no self digest. V9.2.2 hashes the exact final serialized bytes.

`graph_sha256` is computed over the exact head `CompileArtifacts.graph_json` bytes used by the assessment. `object_set_sha256` is computed over compact JSON of sorted `{id,content_hash}` pairs. Configuration and policy digests use compact deterministic DTO serialization. The outer assessment-config digest excludes its own digest field.

### Knowledge and validation facts

An implicated head object includes its ID, kind, authored and effective status, content hash, owner, evidence quality, source coordinate, `changed_in_pr: yes|no|unknown`, and all path/reason matches. Object bodies are forbidden.

Knowledge creates and changes carry head hashes; changes also carry base hashes. A deletion is a metadata-complete tombstone from the base graph, including authority and a kind-correct deletion review disposition. It cannot disappear merely because it has no head node.

Source diagnostics are attributed to the complete changed set as `yes`, `no`, or `unknown`. The validation summary reports full, changed, unchanged, and unattributed error counts. Unknown attribution is fail-closed for a diff-scoped consumer.

### Completeness, outcome, and exit status

Only these tuples are legal:

- `complete/pass`
- `complete/review_required`
- `complete/uncovered`
- `partial/not_evaluated`
- `error/invalid`
- `error/not_evaluated`

Precedence is:

1. An unavailable changed set or head snapshot yields `error/not_evaluated`.
   Missing repository context and an unreadable mutable-worktree state are snapshot failures and use the same structured envelope.
2. Invalid head configuration or head knowledge yields `error/invalid`.
3. An unavailable base compile/diff with trustworthy head and changed-set facts yields `partial/not_evaluated`.
4. Any uncovered or provisional path yields `complete/uncovered`.
5. Any authoritative impact, Knowledge Object change, policy change, lifecycle/contradiction/anchor signal, reviewer, or proof obligation yields `complete/review_required`.
6. Otherwise the result is `complete/pass`.

`pass` means only that a complete empty or fully excluded change set has no deterministic authoritative review signal. It is not a semantic correctness claim.

The CLI exits zero for every complete assessment because this slice is advisory. It exits 2 for partial/error envelopes, invalid command input, or invalid configuration. JSON always uses the existing pretty serializer and a trailing newline. Markdown is heading-free so it can be embedded in a larger pull-request comment.

### Diagnostics

Assessment failures use these stable codes:

- `assessment.invalid_config_path`
- `assessment.ref_unresolved`
- `assessment.snapshot_failed`
- `assessment.comparison_base_unavailable`
- `assessment.base_partial`
- `assessment.changed_set_failed`
- `assessment.invalid_changed_path`
- `assessment.head_invalid`
- `assessment.graph_failed`

## Consequences

The local binary becomes the sole owner of deterministic assessment semantics. The Action can consume one artifact without parsing human text or rebuilding policy. Existing commands and envelopes remain compatible, while users who add the optional configuration require a V9.2.1-capable binary because older strict config parsers will reject the new key.

The deliberately mechanical exact-path model leaves semantic inference, globs, language parsing, runtime policy, managed knowledge, and an assessment MCP tool for later evidence-backed slices.
