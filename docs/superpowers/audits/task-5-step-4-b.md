# Task 5 Step 4 — Audit B (Diff / boundary)

> Status: **PASS**
> Date: 2026-07-10
> Auditor: B (diff / boundary / regression)
> Scope: Matcher rewrite vs legacy CodexPlusPlus loose matching

## Evidence

- Old `contains("codex") && contains("plus")` matchers removed.
- Windows requires `starts_with("{ARTIFACT_PREFIX}-")` and `ends_with("-windows-x64-setup.exe")`.
- macOS requires same prefix and exact `-macos-x64.dmg` / `-macos-arm64.dmg` suffix; native-arch rank 0 still preferred over other arch rank 1.
- `ChimeraCodexExtra-...` near-prefix fixture is rejected.

## Findings

- Legacy underscore DMG shapes are intentionally no longer accepted (Chimera-only assets).

## Open issues

- None for Step 4.
