# Task 9 Step 2 Audit B — sync-upstream.yml (diff / boundary)

> Status: **PASS**
> Date: 2026-07-10
> Scope: Independent workflow boundary review

## Boundary / regression

| Check | Observation | Result |
|---|---|---|
| Secret leakage | Only secret *name* referenced; header documents configure-in-Settings | PASS |
| Release recursion | Sync job never publishes Release; comment points to `release-assets.yml` on main | PASS |
| Remote hardening in CI | Forces origin/upstream URLs + `no_push://upstream` | PASS |
| Exit handling | Script exits 2/3 → Issue upsert then fail job; exit 0 noop/prepared OK; other → fail early | PASS |
| PR check trigger | Checkout/push use automation token (not default GITHUB_TOKEN) so checks can run | PASS |
| Idempotent PR | Lists open PR by head branch before create; edits if exists | PASS |
| Linear history | Auto-merge uses `--squash` | PASS |

## Residual risk

- Workflow cannot be live-exercised until `CHIMERA_AUTOMATION_TOKEN` is configured in repo secrets (D11 remainder).
- Required status check names still depend on Task 8 branch protection wiring.

## Decision

Step 2 boundary review PASS.
