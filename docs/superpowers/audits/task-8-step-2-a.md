# Task 8 Step 2 Audit A — Build-all-platforms-first (requirements)

> Status: **PASS**
> Date: 2026-07-10
> Scope: Windows x64 + macOS x64/arm64 build, npm ci, bundle verify, no early Release

## Requirements checklist

| Requirement | Evidence | Result |
|---|---|---|
| Windows x64 build + artifact upload | `windows-installer` → `actions/upload-artifact` `chimera-codex-windows-x64` | PASS |
| macOS x64 + arm64 matrix | `macos-15-intel` / `macos-14`, targets `x86_64` / `aarch64` | PASS |
| `npm ci` + lockfile | Both platform jobs use `npm ci` with `cache-dependency-path` lockfile | PASS |
| Chimera bundle paths + plist + ad-hoc | Verify loop on `Chimera Codex.app` / `Chimera Codex 管理工具.app`; codesign; “not notarized” | PASS |
| Arch check | `lipo -archs` must contain matrix `lipo_arch` | PASS |
| Matrix failure blocks publish | `fail-fast: true`; `publish-release` needs both build jobs | PASS |
| No Release during build jobs | Builds only `upload-artifact`; no `gh release` / softprops | PASS |

## Decision

Step 2 requirements met.
