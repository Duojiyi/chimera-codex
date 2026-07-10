# Task 9 Step 1 Audit A — sync-upstream.ps1 (requirements)

> Status: **PASS**
> Date: 2026-07-10
> Scope: `scripts/sync-upstream.ps1` behavior vs Plan Task 9 Step 1

## Requirements checklist

| Requirement | Evidence | Result |
|---|---|---|
| Validate clean worktree / non-shallow / origin+upstream URLs / no in-progress git op | `Assert-Remotes`, shallow check, dirty/in-progress refuse on apply | PASS |
| Query latest formal Release; ignore draft/prerelease | GitHub releases API loop skips draft/prerelease; tag must be X.Y.Z | PASS |
| Idempotent if Chimera tag / sync branch / workspace already at version | `Test-IdempotentAlreadySynced` | PASS |
| Isolated worktree + `sync/upstream-vX.Y.Z` | `git worktree add -b` under temp path | PASS |
| Merge conflict → record files, `merge --abort`, exit 2 | conflict path sets `conflict_files`, abort, exit 2 | PASS |
| Set `X.Y.Z-chimera.1` + branding/gates | `Set-WorkspaceChimeraVersion` + `Invoke-Gates` | PASS |
| Script does not modify main / create Release | no checkout main write; no `gh release` | PASS |
| `-DryRun` read-only; exit 0/2/3/4 | DryRun uses get-url/status/ls-remote/API only; documented exits | PASS |

## Decision

Step 1 requirements met.
