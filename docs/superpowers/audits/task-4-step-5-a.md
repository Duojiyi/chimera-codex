# Task 4 Step 5 — Audit A (Requirements)

> Status: **PASS**
> Date: 2026-07-10
> Auditor: A (requirements / observable behavior)
> Scope: Required tests for first-run / Key-first / upgrade

## Evidence

| Case | Test | Result |
|------|------|--------|
| Missing settings → Chimera selected, no settings write | `settings_store_load_missing_file_returns_chimera_first_run_without_writing` | PASS |
| Existing settings preserved | `settings_store_load_existing_file_preserves_active_profile_without_chimera_injection` | PASS |
| Empty Key fails, live unchanged | `save_and_enable_chimera_hub_rejects_empty_key_without_touching_live_files` | PASS |
| Valid Key updates profile + live | `save_and_enable_chimera_hub_writes_profile_and_live_files_for_valid_key` | PASS |
| Upgrade keeps active | `save_and_enable_chimera_hub_preserves_existing_non_chimera_active_on_upgrade_path` | PASS |

## Findings

- Plan Step 5 matrix covered.

## Open issues

- None for Step 5.
