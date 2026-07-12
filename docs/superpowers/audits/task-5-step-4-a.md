# Task 5 Step 4 — Audit A (Requirements)

> Status: **PASS**
> Date: 2026-07-10
> Auditor: A (requirements / observable behavior)
> Scope: Strict ChimeraCodex platform/arch asset names

## Evidence

| Case | Test | Result |
|------|------|--------|
| Windows x64 setup selected | `latest_json_selects_strict_chimera_platform_assets` | PASS |
| macOS native arch preferred | `asset_selection_prefers_macos_native_arch` | PASS |
| zip / near-prefix / upstream / arm64-windows rejected | `asset_selection_rejects_zip_source_and_near_prefix` | PASS |

Accepted shapes only:
- `ChimeraCodex-<version>-windows-x64-setup.exe`
- `ChimeraCodex-<version>-macos-x64.dmg`
- `ChimeraCodex-<version>-macos-arm64.dmg`

## Findings

- Prefix uses branding `ARTIFACT_PREFIX`; substring “chimera” alone is insufficient.

## Open issues

- None for Step 4.
