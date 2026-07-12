# Task 9 Step 1 Audit B — sync-upstream.ps1 (diff / boundary)

> Status: **PASS**
> Date: 2026-07-10
> Scope: Independent boundary review of `scripts/sync-upstream.ps1`

## Boundary / regression

| Check | Observation | Result |
|---|---|---|
| Remote push block | Rejects upstream push URL that still points at BigPizzaV3 | PASS |
| DryRun write guard | Refuses in-repo `ResultPath` under DryRun; no fetch/worktree/merge | PASS |
| Main safety | Apply path never checks out or commits on `main`; worktree branch only | PASS |
| Conflict cleanup | On conflict: abort, remove worktree, delete half-baked local sync branch | PASS |
| Gate failure exit | Gate throw → cleanup → exit 3 | PASS |
| Config errors | Wrong remotes / shallow / dirty apply → exit 4 | PASS |
| Version files | Updates Cargo workspace version, package.json, tauri.conf.json, bumps `macos_build_number` | PASS |
| No secret embed | Reads token only from env for API; never prints token | PASS |

## Residual risk

- Full three-platform installer builds are left to PR checks (script gates = branding/ads/fmt/test). Acceptable per sync→PR design.
- Concurrent dirty worktrees can change `git status` during DryRun; integrity check asserts HEAD/refs (and caller may hash script files).

## Decision

Step 1 boundary review PASS.
