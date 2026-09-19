**❌ 1 errors · ⚠️ 1 warnings**

- ❌ `broken.adoc:7` `ref.broken` — depends&#95;on target &#96;missing.object&#96; does not resolve to a declared Knowledge Object
- ⚠️ `overdue.adoc:6` `task.overdue` — open task &#96;ci.update-runbook&#96; is overdue (due 2020-01-01)

<details>
<summary>Remediation help (2)</summary>

- `missing.object` — Relation targets must name an existing Knowledge Object. Supported relation fields: &#96;depends&#95;on&#96;, &#96;supersedes&#96;, &#96;related&#95;to&#96;.
- `ci.update-runbook` — Complete the task and set &#96;status: done&#96;, or move its &#96;due&#96; date.

</details>

Suggested action: fix the errors above, then re-run `adoc check`.
