# Task 4 Aggregate Audit — ChimeraHub preset + Key-first first-run

> Status: **PASS**
> Date: 2026-07-10
> Scope: Task 4 Steps 1–5 (preset, promo cleanup, first-run settings, Key-first command/UI, tests)

## Evidence Ledger

| Step | Audit | File | Focus | Result |
|------|-------|------|-------|--------|
| 1 | A | `task-4-step-1-a.md` | ChimeraHub preset complete | PASS |
| 1 | B | `task-4-step-1-b.md` | Branding constants / no binary rename | PASS |
| 2 | A | `task-4-step-2-a.md` | jojocode + invite codes removed | PASS |
| 2 | B | `task-4-step-2-b.md` | Localized preset deletion | PASS |
| 3 | A | `task-4-step-3-a.md` | Missing-file first-run only | PASS |
| 3 | B | `task-4-step-3-b.md` | Serde Default unchanged | PASS |
| 4 | A | `task-4-step-4-a.md` | Key-first command + UI | PASS |
| 4 | B | `task-4-step-4-b.md` | IPC/logging/atomic write | PASS |
| 5 | A | `task-4-step-5-a.md` | Required test matrix | PASS |
| 5 | B | `task-4-step-5-b.md` | Isolation + secret hygiene | PASS |

### Key command evidence

| Check | Result |
|-------|--------|
| `cargo test -p codex-plus-core --lib chimera_first_run` (+ related settings load tests) | PASS |
| `cargo test -p codex-plus-manager --lib save_and_enable_chimera_hub` | PASS |
| `cargo test -p codex-plus-manager --test windows_subsystem provider_presets` | PASS |
| `npm run check` (tsc --noEmit) | exit 0 |
| `node tools/i18n-verify.mjs` | exit 0 |

## Dual-blind note

Audits A and B were written as independent perspectives (requirements/observable vs diff/boundary/regression) without cross-citing findings before this aggregate.

## Open issues (non-blocking)

- Frontend `defaultSettings` placeholder still uses legacy `default` profile until `load_settings` returns; runtime first-run comes from backend.
- Real ChimeraHub Key smoke against live API remains a pre-Release manual gate (spec), not covered by unit tests.

## Decision

**Task 4 passed.** T13–T16 may be marked complete. Proceed to Task 5 when ready.
