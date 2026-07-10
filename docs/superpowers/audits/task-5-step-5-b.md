# Task 5 Step 5 — Audit B (Diff / boundary)

> Status: **PASS**
> Date: 2026-07-10
> Auditor: B (diff / boundary / regression)
> Scope: ReleaseAsset fields, temp write, IPC contract

## Evidence

- `ReleaseAsset` / `Release` / `UpdateCheck` gain `sha256`/`size` (and Option mirrors on Release/UpdateCheck).
- `download_asset_to` writes `{name}.part`, verifies, `rename`s; on error removes temp and final.
- Manager `check_update` JSON includes `assetSha256`/`assetSize`; `performUpdate` refuses to build release without both.
- Serde `#[serde(default)]` on new Release fields keeps older IPC payloads deserializing, but download still fails closed without checksums.

## Findings

- No launch-on-failure path remains in `perform_update`.

## Open issues

- None for Step 5.
