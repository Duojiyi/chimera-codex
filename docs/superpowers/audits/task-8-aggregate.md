# Task 8 Aggregate Audit — Build-first public Release + PR parity

> Status: **PASS**
> Date: 2026-07-10
> Scope: Task 8 Steps 1–5 (trigger/gate, build-first, naming, publish, pr-build)

## Evidence Ledger

| Step | Audit | File | Focus | Result |
|------|-------|------|-------|--------|
| 1 | A | `task-8-step-1-a.md` | Trigger / version / tag gate / concurrency | PASS |
| 1 | B | `task-8-step-1-b.md` | Recursion / token / concurrency boundary | PASS |
| 2 | A | `task-8-step-2-a.md` | Three-platform build-first | PASS |
| 2 | B | `task-8-step-2-b.md` | npm ci / no early release | PASS |
| 3 | A | `task-8-step-3-a.md` | ChimeraCodex naming | PASS |
| 3 | B | `task-8-step-3-b.md` | Allowlist + installer contract | PASS |
| 4 | A | `task-8-step-4-a.md` | Draft → upload → publish + smoke | PASS |
| 4 | B | `task-8-step-4-b.md` | Failure modes / latest safety | PASS |
| 5 | A | `task-8-step-5-a.md` | PR gates + parity | PASS |
| 5 | B | `task-8-step-5-b.md` | Permissions / job graph | PASS |

### Key command evidence

| Check | Result |
|-------|--------|
| `js-yaml` parse `release-assets.yml` / `pr-build.yml` | OK |
| Structural asserts (no tabs, no `CodexPlusPlus-`, no hardcoded tokens, `npm ci`) | OK |
| `pwsh -File scripts/verify-no-upstream-ads.ps1` | exit 0 |
| Cargo version extract | `1.2.34-chimera.1` |

## Dual-blind note

Audits A and B were written as independent perspectives (requirements/observable vs diff/boundary/regression) without cross-citing findings before this aggregate.

## Open issues (non-blocking for Task 8)

- Live GitHub Actions run and first public Release smoke remain Task 10 / V6 / V9.
- Incomplete draft after mid-publish failure requires manual cleanup before re-run (idempotent tag gate).
- Main push runs both `pr-build` and `release-assets` (cost only).

## Decision

**Task 8 passed.** T22–T24 may be marked complete.
