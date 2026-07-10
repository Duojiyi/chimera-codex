# Task 6 Step 3 — Audit B (Diff / Boundary)

> Status: **PASS**
> Date: 2026-07-10
> Auditor: B (diff / boundary / regression)
> Scope: NSIS upgrade safety and uninstall symmetry

## Evidence

| Check | Result |
|-------|--------|
| Binaries staged before Delete/Rename | both exe use `.new` |
| Failure before Rename leaves prior binaries | staging order correct |
| Uninstall cleans Codex++ and Chimera start-menu folders | both RMDir |
| Uninstall deletes both Uninstall registry keys | Codex++ and CodexPlusPlus |
| No BigPizzaV3 Publisher | removed |

## Findings

- Start menu folder renamed to `Chimera Codex` while InstallDir stays legacy path.
- Unicode Chinese shortcut names used; mojibake only in Delete cleanup.

## Open issues

- None for Step 3.
