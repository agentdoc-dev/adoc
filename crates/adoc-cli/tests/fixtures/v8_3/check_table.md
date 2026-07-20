**❌ 1 errors · ⚠️ 1 warnings**

|    | Location | Code | Message |
|----|----------|------|---------|
| ❌ | `broken.adoc:7` | `ref.broken` | depends_on target `missing.object` does not resolve to a declared Knowledge Object |
| ⚠️ | `overdue.adoc:6` | `task.overdue` | open task `ci.update-runbook` is overdue (due 2020-01-01) |

<details>
<summary>Remediation help (2)</summary>

- `missing.object` — Relation targets must name an existing Knowledge Object. Supported relation fields: `depends_on`, `supersedes`, `related_to`.
- `ci.update-runbook` — Complete the task and set `status: done`, or move its `due` date.

</details>

Suggested action: fix the errors above, then re-run `adoc check`.
