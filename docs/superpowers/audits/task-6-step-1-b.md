# Task 6 Step 1 — Audit B (Diff / Boundary)

> Status: **PASS**
> Date: 2026-07-10
> Auditor: B (diff / boundary / regression)
> Scope: mod.rs constant wiring and companion path resolution

## Evidence

| Check | Result |
|-------|--------|
| Constants alias `crate::branding::*` rather than hardcode drift | confirmed in `install/mod.rs` |
| `companion_binary_path_from_exe` uses new Chimera app names | tests updated and PASS |
| No change to provider id / protocol / state dir | out of scope, untouched |

## Findings

- Legacy constants are separate from display constants; no accidental reuse for new shortcuts.
- Companion resolution follows Chimera `.app` names only (legacy apps are detection-only).

## Open issues

- None for Step 1.
