# Upstream v1.2.36 Resolution Evidence

Date: 2026-07-14

## Scope

- Merge upstream formal release `v1.2.36` (`91a83acd8bdf79a388a106c7c1ea76f9df6bcea9`) into the Chimera sync branch.
- Preserve Chimera `1.2.36-chimera.1`, macOS build `7`, branding, ChimeraHub defaults, forced updates, one Windows desktop entry, relay latency, and removal of customer promotions/About/recommendations.
- Adopt upstream portable companion launch, manager activation, Windows window scoring, V2 pet real-mouse support, UI updates, and macOS bundle launch fixes.

## Conflict Resolution

- All index conflicts were resolved; `git diff --name-only --diff-filter=U` returned no paths.
- The promotional repository helper/tests and sponsor/community image additions were excluded from the merge result.
- The Windows desktop shortcut remains `Chimera++.lnk` targeting `codex-plus-plus-manager.exe`; the Start Menu keeps the explicit manager entry.
- macOS translocation tests use the branded `Chimera++.app` and `Chimera++ 管理工具.app` names and reject legacy `Codex++` names.
- `apps/codex-plus-manager/src-tauri/Cargo.toml` has only a working-tree line-ending indication, no content diff, and was intentionally not staged.

## README TDD

- Red: added Chinese and English assertions that the Windows desktop entry opens the manager and Codex launches from the manager. A dependency-free contract probe failed first with `RED: Chinese README does not say desktop entry opens manager`.
- Green: corrected both customer README sections and their platform-specific entry descriptions. The same probe then printed `README manager-entry contract: PASS`.
- Regression: `generate-branding.ps1 -Check`, `verify-no-upstream-ads.ps1`, `verify-license.ps1`, `cargo fmt --all -- --check`, and scoped diff checks passed.

## Lightweight Regression Gates

- `generate-branding.ps1 -SelfTest`: PASS
- `generate-branding.ps1 -Check`: PASS
- `verify-no-upstream-ads.ps1`: PASS
- `verify-license.ps1`: PASS
- `test-sync-upstream.ps1`: PASS
- `test-verify-allowlist.ps1`: PASS
- `verify-brand-icons.ps1 -SelfTest`: PASS
- `cargo fmt --all -- --check`: PASS
- `git diff --cached --check`: PASS before the final documentation stage

No local dependency installation, frontend build, Rust workspace build, or packaging build was run. Those remain cloud required-check gates as requested.

## Residual Verification

- GitHub required checks must compile/test/package the Windows x64 and macOS x64/arm64 candidates.
- The release must still be validated for tag target, exact eight assets, manifest metadata, digests, sizes, SHA-256 values, and anonymous downloads.
- Task 16 real Windows/macOS install, upgrade, shortcut/icon-cache, launch, and uninstall acceptance remains user-side work and is not claimed complete.
