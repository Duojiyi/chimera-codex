# Task 6 Step 4 — Audit A (Requirements)

> Status: **PASS**
> Date: 2026-07-10
> Auditor: A (requirements / observable behavior)
> Scope: package-dmg.sh Chimera names, numeric plist, legacy tip, ad-hoc, workflow paths

## Evidence

| Requirement | Evidence | Result |
|-------------|----------|--------|
| DMG `ChimeraCodex-*-macos-*.dmg` | script + installer test | PASS |
| Apps `Chimera Codex.app` / `Chimera Codex 管理工具.app` | create_app calls | PASS |
| Bundle ID / executable unchanged | `com.bigpizzav3.codexplusplus*`, CodexPlusPlus* | PASS |
| ShortVersion X.Y.Z, Version = macos_build_number | SHORT_VERSION / MACOS_BUILD_NUMBER | PASS |
| Ad-hoc only, no notarization claim | comments + `codesign --sign -` | PASS |
| Legacy detection without delete | `macos_detects_legacy_apps_without_deleting_them` | PASS |
| Manager tip + Open Applications | overview fields + UI banner + command | PASS |
| release-assets / pr-build verify Chimera paths | workflow updated | PASS |

## Findings

- DMG ships README.txt with Gatekeeper / legacy migration steps.
- Core plist builder also uses short version + build number.

## Open issues

- None for Step 4. Live DMG build remains Task 8/10.
