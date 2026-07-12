# Task 8 Step 3 Audit A — ChimeraCodex artifact naming (requirements)

> Status: **PASS**
> Date: 2026-07-10
> Scope: Strict `ChimeraCodex-*` published / PR artifact names

## Requirements checklist

| Asset | Pattern in workflow | Result |
|---|---|---|
| Windows setup | `ChimeraCodex-$version-windows-x64-setup.exe` | PASS |
| Windows zip | `ChimeraCodex-$version-windows-x64.zip` | PASS |
| macOS DMG | `ChimeraCodex-$version-macos-{x64,arm64}.dmg` | PASS |
| macOS zip | `ChimeraCodex-$version-macos-{x64,arm64}.zip` | PASS |
| No `CodexPlusPlus-` prefix | Grep clean on both workflow files | PASS |
| Updater matcher alignment | setup.exe / `*-macos-*.dmg` match `update.rs` strict matchers | PASS |

## Decision

Step 3 naming requirements met.
