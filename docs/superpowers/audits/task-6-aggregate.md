# Task 6 Aggregate Audit — Dual-platform installers + legacy upgrade

> Status: **PASS**
> Date: 2026-07-10
> Scope: Task 6 Steps 1–5 (display branding, Windows/NSIS, macOS DMG, legacy migration, tests)

## Evidence Ledger

| Step | Audit | File | Focus | Result |
|------|-------|------|-------|--------|
| 1 | A | `task-6-step-1-a.md` | Branding display constants | PASS |
| 1 | B | `task-6-step-1-b.md` | Binary stability / companion paths | PASS |
| 2 | A | `task-6-step-2-a.md` | Windows shortcuts / Publisher | PASS |
| 2 | B | `task-6-step-2-b.md` | Registry / protocol boundaries | PASS |
| 3 | A | `task-6-step-3-a.md` | NSIS branding + InstallDir | PASS |
| 3 | B | `task-6-step-3-b.md` | Staging + uninstall symmetry | PASS |
| 4 | A | `task-6-step-4-a.md` | DMG / plist / legacy tip / CI | PASS |
| 4 | B | `task-6-step-4-b.md` | Version fields + no auto-delete | PASS |
| 5 | A | `task-6-step-5-a.md` | installers 14 passed | PASS |
| 5 | B | `task-6-step-5-b.md` | Contract tests + overview | PASS |

### Key command evidence

| Check | Result |
|-------|--------|
| `cargo test -p codex-plus-core --test installers` | 14 passed |
| `cargo test -p codex-plus-manager --lib overview_contains_expected_operational_fields` | PASS |

## Dual-blind note

Audits A and B were written as independent perspectives (requirements/observable vs diff/boundary/regression) without cross-citing findings before this aggregate.

## Open issues (non-blocking)

- Live NSIS/DMG smoke and Gatekeeper UX remain Task 10.
- Release zip asset rename to ChimeraCodex-* remains Task 8.

## Decision

**Task 6 passed.** T17–T19 may be marked complete.
