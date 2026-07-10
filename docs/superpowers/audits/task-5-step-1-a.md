# Task 5 Step 1 — Audit A (Requirements)

> Status: **PASS**
> Date: 2026-07-10
> Auditor: A (requirements / observable behavior)
> Scope: Failing SemVer tests for Chimera channel ordering

## Evidence

| Assertion | Test | Result |
|-----------|------|--------|
| `.1 → .2` newer | `chimera_semver_comparison_orders_revision_and_upstream` | PASS |
| Cross-upstream `1.2.35-chimera.1 > 1.2.34-chimera.9` | same | PASS |
| Equal / older rejected as newer | same | PASS |
| Illegal / foreign channels rejected | `chimera_semver_rejects_illegal_and_foreign_channels` | PASS |

Red evidence: prior `parse_version_tag` truncated at `-`, so `.1→.2` would not distinguish; new tests encode required behavior.

## Findings

- Plan Step 1 SemVer order and channel rejection covered.

## Open issues

- None for Step 1.
