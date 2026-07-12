# Task 9 Step 2 Audit A — sync-upstream.yml (requirements)

> Status: **PASS**
> Date: 2026-07-10
> Scope: `.github/workflows/sync-upstream.yml` vs Plan Task 9 Step 2

## Requirements checklist

| Requirement | Evidence | Result |
|---|---|---|
| schedule 2x/day + workflow_dispatch | cron `0 6` and `0 18` + dispatch | PASS |
| fetch-depth: 0 | checkout `fetch-depth: 0` | PASS |
| concurrency group | `concurrency.group: sync-upstream`, `cancel-in-progress: false` | PASS |
| Minimal permissions | `contents/pull-requests/issues: write` only | PASS |
| Automation token placeholder | Comments + `${{ secrets.CHIMERA_AUTOMATION_TOKEN }}`; no secret value in file | PASS |
| Push sync branch + PR body with tag/version/gates | Push + `gh pr create/edit` body | PASS |
| Auto-merge after checks | `gh pr merge --auto --squash` | PASS |
| Conflict/gate Issue dedup by title | Exact title `[sync:vX.Y.Z] upstream sync blocked`; create or edit | PASS |
| Close Issue on successful prepare | Success path closes matching open Issue | PASS |
| No `gh release create` in sync workflow | Grep: absent | PASS |

## Decision

Step 2 requirements met.
