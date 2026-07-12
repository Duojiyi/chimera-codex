# Task 14 Aggregate Gate - T33

> Date: 2026-07-12
> Scope: original Chimera++ master icon, platform exports, provenance, security gate, CI integration and independent aggregate audits

## Evidence

- Three original SVG concepts and three small-size iterations were produced without third-party image inputs; concept 1 is the recorded final source in `brand/icon/logo.svg`.
- `brand/icon/PROVENANCE.md` records the generation boundary, selected concept, tool/model and AGPL-3.0-only release terms.
- The 1024 PNG and multi-entry ICO are reproduced exactly from the master SVG by the repository-locked Tauri CLI and are byte-identical across all six distribution locations.
- The icon gate rejects DTD/external resolution, non-allowlisted SVG elements and attributes, active values, malformed ICO ranges, non-decodable PNG payloads and directory/payload dimension mismatches.
- `-SelfTest` includes safe and malicious SVG fixtures, a zero-payload ICO and a signature-only truncated PNG ICO. Both release workflows run it and the real gate after `npm ci`; the Rust contract locks that order.

## Verification

| Gate | Result |
|---|---|
| `verify-brand-icons.ps1 -SelfTest` | PASS |
| `verify-brand-icons.ps1` with fresh SVG exports | PASS |
| installer contracts | PASS, 26/26 |
| branding generator check | PASS |
| upstream ads scanner | PASS |
| targeted diff check | PASS |
| independent aggregate audit A | PASS |
| independent aggregate audit B | PASS AFTER REMEDIATION |

## Deferred Platform Scope

Windows executable/NSIS embedding and macOS x64/arm64 ICNS/app-bundle output remain required in Task 15 CI. Installed shortcut, Start Menu, Finder and Dock appearance remain Task 16 real-platform checks.

## Gate

**PASS.** Task 14 is complete and T33 may be checked.
