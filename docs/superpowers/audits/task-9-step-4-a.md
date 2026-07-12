# Task 9 Step 4 Audit A — T28 drill readiness (requirements)

> Status: **PASS (deferred live drills)**
> Date: 2026-07-10
> Scope: Conflict / gate / hash failure drills (T28)

## Live drill status

Cannot safely manufacture a real merge conflict or Actions Issue against `main` in this session:

- `CHIMERA_AUTOMATION_TOKEN` is not yet configured for live workflow runs (D11 remainder).
- Local worktree is dirty with unrelated Task 8/docs WIP; apply mode correctly refuses dirty trees.
- Upstream `v1.2.34` is already represented as `1.2.34-chimera.1` (idempotent noop).

## DryRun evidence (substitutes for live conflict manufacture)

- DryRun exit 0; printed plan for `v1.2.34` → `sync/upstream-v1.2.34` / `1.2.34-chimera.1`
- HEAD/refs unchanged; sync script/workflow hashes unchanged
- Script conflict path implements: record files → `git merge --abort` → exit 2
- Workflow Issue path implements exact-title dedup `[sync:vX.Y.Z] upstream sync blocked`
- Workflow contains no `gh release create`

## Drill checklist — 待首次启用 token 后执行

1. Re-run schedule/dispatch on already-synced tag → no new PR / Issue / Release.
2. Force a conflicting sync branch (or temporary conflicting upstream fixture) → merge abort, single Issue, `main` unchanged, no Release.
3. Simulate gate/test failure (exit 3) → Issue upsert, no Release, `latest.json` untouched.
4. Simulate hash/publish failure on release workflow separately → previous `latest` retained (Task 8/10).
5. Happy path once a newer upstream formal Release exists → PR → checks → auto-merge → build-first publish.

## Decision

T28 accepted as **documented deferred drills** with DryRun + static path evidence; live execution gated on token enablement.
