**❌ 1 errors · ⚠️ 1 warnings**

**`broken.adoc`**

- ❌ `ref.broken` (line 7) — depends&#95;on target &#96;missing.object&#96; does not resolve to a declared Knowledge Object
  - object_id: `missing.object`
  - help: Relation targets must name an existing Knowledge Object. Supported relation fields: &#96;depends&#95;on&#96;, &#96;supersedes&#96;, &#96;related&#95;to&#96;.

**`overdue.adoc`**

- ⚠️ `task.overdue` (line 6) — open task &#96;ci.update-runbook&#96; is overdue (due 2020-01-01)
  - object_id: `ci.update-runbook`
  - help: Complete the task and set &#96;status: done&#96;, or move its &#96;due&#96; date.

Suggested action: fix the errors above, then re-run `adoc check`.
