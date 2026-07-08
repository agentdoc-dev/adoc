## adoc check: 1 errors, 1 warnings

### `./broken.adoc`

- ❌ `ref.broken` (line 7) — depends_on target `missing.object` does not resolve to a declared Knowledge Object
  - object_id: `missing.object`
  - help: Relation targets must name an existing Knowledge Object. Supported relation fields: `depends_on`, `supersedes`, `related_to`.

### `./overdue.adoc`

- ⚠️ `task.overdue` (line 6) — open task `ci.update-runbook` is overdue (due 2020-01-01)
  - object_id: `ci.update-runbook`
  - help: Complete the task and set `status: done`, or move its `due` date.

Suggested action: fix the errors above, then re-run `adoc check`.
