# Task 8 Step 5 Audit A — PR build parity (requirements)

> Status: **PASS**
> Date: 2026-07-10
> Scope: `pr-build.yml` same build/naming/verify; gates; no Release

## Requirements checklist

| Requirement | Evidence | Result |
|---|---|---|
| Same ChimeraCodex naming | Windows zip/setup + macOS dmg/zip patterns match release | PASS |
| Same macOS bundle verify + arch | Shared verify script shape + `lipo -archs` | PASS |
| `npm ci` | All frontend install steps use `npm ci` | PASS |
| `generate-branding -Check` | `gates` job runs `scripts/generate-branding.ps1 -Check` | PASS |
| `verify-no-upstream-ads` | `gates` job runs scanner | PASS |
| Rust tests | `cargo test --workspace` in `gates` | PASS |
| Frontend typecheck + build | `npm run check` + `npm run vite:build` in `gates` | PASS |
| Artifacts only / no Release | Only `upload-artifact`; `permissions.contents: read` | PASS |
| Cargo version for names | `resolve-version` job feeds `VERSION` | PASS |

## Decision

Step 5 requirements met.
