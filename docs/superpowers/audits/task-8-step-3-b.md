# Task 8 Step 3 Audit B — Naming (boundary / allowlist)

> Status: **PASS**
> Date: 2026-07-10
> Scope: Allowlist cleanup; NSIS/DMG script contract

## Boundary review

| Check | Observation | Result |
|---|---|---|
| NSIS OutFile already ChimeraCodex | Workflow passes `/DVERSION=` only; does not rename setup | PASS |
| DMG script already ChimeraCodex | `package-dmg.sh` DMG path; workflow asserts file exists | PASS |
| Allowlist Task 8 placeholders removed | `verify-allowlist.txt` no longer lists workflow `CodexPlusPlus-` lines | PASS |
| `verify-no-upstream-ads.ps1` | Local run exit 0 after allowlist edit | PASS |
| Zip included but not required by updater | Extra assets OK; setup/dmg remain primary | PASS |

## Decision

Step 3 boundary review pass.
