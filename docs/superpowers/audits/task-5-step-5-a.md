# Task 5 Step 5 — Audit A (Requirements)

> Status: **PASS**
> Date: 2026-07-10
> Auditor: A (requirements / observable behavior)
> Scope: latest.json sha256/size and download verification

## Evidence

| Case | Test | Result |
|------|------|--------|
| Missing sha256/size rejected | `latest_json_rejects_missing_checksum_fields` | PASS |
| Matching hash+size writes final file | `download_asset_to_verifies_sha256_and_size` | PASS |
| Wrong hash cleans temp, no final | `download_asset_to_rejects_wrong_hash_and_cleans_temp` | PASS |
| Wrong size cleans temp, no final | `download_asset_to_rejects_wrong_size_and_cleans_temp` | PASS |
| Path traversal rejected | `safe_asset_name_rejects_path_traversal` | PASS |
| Frontend/IPC pass sha256+size | `App.tsx` + `commands.rs` payload | reviewed |

## Findings

- `perform_update` verifies before `launch_installer`; failure deletes `.part` / final and does not launch.

## Open issues

- None for Step 5. Interrupted HTTP download is covered by reqwest error before write; no installer launch on that path.
