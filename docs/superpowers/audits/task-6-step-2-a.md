# Task 6 Step 2 — Audit A (Requirements)

> Status: **PASS**
> Date: 2026-07-10
> Auditor: A (requirements / observable behavior)
> Scope: windows.rs DisplayName / Publisher / shortcuts / legacy cleanup

## Evidence

| Requirement | Test / observation | Result |
|-------------|-------------------|--------|
| New shortcuts use Chimera names | `windows_entrypoint_plan_creates_chimera_shortcuts_not_legacy` | PASS |
| Publisher = ChimeraHub, DisplayName = Chimera Codex | plan fields asserted | PASS |
| Legacy uninstall keys retained | `uninstall_key=CodexPlusPlus`, `legacy_uninstall_key=Codex++` | PASS |
| Legacy + mojibake cleanup list | `windows_legacy_shortcut_cleanup_lists_old_and_mojibake_names` | PASS |
| `default_install_root` remains desktop known-folder (not NSIS Programs) | strategy test | PASS |

## Findings

- Install path creates Chimera `.lnk` after deleting legacy/mojibake shortcuts.
- Uninstall removes both Chimera and legacy shortcut sets.

## Open issues

- None for Step 2.
