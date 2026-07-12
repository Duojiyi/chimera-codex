# Task 4 Step 3 — Audit B (Diff / boundary)

> Status: **PASS**
> Date: 2026-07-10
> Auditor: B (diff / boundary / regression)
> Scope: First-run constructor vs serde Default

## Evidence

- `SettingsStore::load` NotFound branch and `load_raw_object` NotFound branch both use `chimera_first_run_settings()`.
- Corrupt JSON path still uses `unwrap_or_default()` → `BackendSettings::default()`, not Chimera first-run.
- Unit tests: missing / existing / bad-json / default-unchanged all green.

## Findings

- Upgrade path protected; only true absence of settings file selects Chimera.

## Open issues

- None for Step 3.
