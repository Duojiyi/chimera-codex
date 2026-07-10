# Task 6 Step 3 — Audit A (Requirements)

> Status: **PASS**
> Date: 2026-07-10
> Auditor: A (requirements / observable behavior)
> Scope: NSIS Name / DisplayName / Publisher / OutFile / InstallDir / cleanup / staging

## Evidence

| Requirement | Observation in `CodexPlusPlus.nsi` | Result |
|-------------|--------------------------------------|--------|
| Name = Chimera Codex | `Name "Chimera Codex"` | PASS |
| OutFile ChimeraCodex-*-windows-x64-setup.exe | present | PASS |
| InstallDir stays Programs\Codex++ | `$LOCALAPPDATA\Programs\Codex++` | PASS |
| InstallDirRegKey / Uninstall\Codex++ kept | present; DisplayName/Publisher updated | PASS |
| Staging `.exe.new` before replace | present | PASS |
| Clean old+new+mojibake shortcuts | Delete lists cover all | PASS |
| Create only Chimera shortcuts | CreateShortcut Chimera names | PASS |

## Findings

- `windows_nsi_uses_chimera_branding_keeps_install_dir_and_cleans_legacy` PASS.
- Garbled cleanup literal retained intentionally.

## Open issues

- None for Step 3. Full NSIS smoke install remains Task 10.
