# Task 6 Step 2 — Audit B (Diff / Boundary)

> Status: **PASS**
> Date: 2026-07-10
> Auditor: B (diff / boundary / regression)
> Scope: windows.rs registration and protocol text

## Evidence

| Check | Result |
|-------|--------|
| `WindowsEntrypointPlan` gained `display_name` / `publisher` | present |
| Protocol id remains `codexplusplus` | unchanged subkey |
| Protocol display string uses Chimera silent name | `URL:{SILENT_NAME} Import Protocol` |
| Install still writes `Uninstall\CodexPlusPlus` and clears legacy `Codex++` key | retained |

## Findings

- No confusion between desktop shortcut root and NSIS `Programs\Codex++`.
- Mojibake literal kept only in cleanup path list.

## Open issues

- None for Step 2.
