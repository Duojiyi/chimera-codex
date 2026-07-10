# Task 5 Step 2 — Audit B (Diff / boundary)

> Status: **PASS**
> Date: 2026-07-10
> Auditor: B (diff / boundary / regression)
> Scope: Version sync script and dependency wiring

## Evidence

- Root `Cargo.toml` adds `semver = "1"` workspace dep; core crate depends on it.
- `scripts/generate-branding.ps1` adds `Assert-VersionSync` and `Assert-MacosBuildNumberProgress` (git tag `v*-chimera.*` → `brand/product.toml`).
- First-release path: no chimera tags → only positive integer required (current `macos_build_number = 1`).

## Findings

- No hand-edited branding constants for version; sync is Check-gated.
- package-lock root package version updated to match.

## Open issues

- None for Step 2.
