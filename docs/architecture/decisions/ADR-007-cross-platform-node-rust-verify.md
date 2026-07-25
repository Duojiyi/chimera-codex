# ADR-007: Cross-Platform Node/Rust Verification via Single Entry Point

**Status:** Accepted  
**Date:** 2026-07-26

## Context

Chimera++ 1.x verification scripts were PowerShell-only (`verify-license.ps1`, `verify-no-upstream-ads.ps1`, etc.). macOS CI runners cannot execute them without installing PowerShell, and maintaining `.sh` equivalents creates behavioral divergence that grows over time.

## Decision

All verification scripts V9–V15 are **Node.js ESM `.mjs` files** that:
- Use only Node built-ins (`fs`, `path`, `child_process`, `process`)
- Accept the same CLI flags on Windows and macOS
- Are invoked via `node scripts/<name>.mjs` — no shell wrapper needed

The single top-level entry point is `node scripts/verify-v2.mjs`, which orchestrates all checks in sequence or via `--only`/`--skip` flags.

### Windows-specific helpers
Operations that require Windows-native tools (Authenticode signature inspection via `signtool`, MSIX manifest extraction, registry queries) **may** be `.ps1` files, but:
- They are never listed as required CI checks themselves
- They are invoked explicitly by the Node orchestrator on a Windows job only
- They must accept all inputs as parameters (no env-var-only coupling)
- They have Node-side fixture/stub equivalents for unit testing

### macOS CI
- Does not install PowerShell
- Runs the same `node scripts/verify-v2.mjs` command
- Windows-specific `.ps1` helpers simply are not called

## Consequences

- No `.sh` equivalents to maintain for verification logic
- New checks added to V9–V15 must be implemented as `.mjs`
- Behavioral divergence between platforms is eliminated for the verification surface
- Windows-native signing/MSIX operations remain PowerShell but are isolated as sub-helpers, not required check gates
- CI YAML uses `node scripts/verify-v2.mjs` as the single verification command on both Windows and macOS matrix legs
