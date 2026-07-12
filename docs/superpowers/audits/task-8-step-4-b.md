# Task 8 Step 4 Audit B — Publish job (diff / failure modes)

> Status: **PASS**
> Date: 2026-07-10
> Scope: Failure modes, permissions, client contract

## Failure / boundary review

| Mode | Behavior | Result |
|---|---|---|
| Build job failed | `publish-release` skipped; no tag/Release | PASS |
| Draft create OK, upload fails | Draft retained; `/releases/latest` unchanged | PASS |
| Publish succeeds, CDN lag | Smoke retries latest.json up to 6 attempts | PASS |
| Permissions | `contents: write` only; uses `secrets.GITHUB_TOKEN` | PASS |
| Client parse contract | Matches `update.rs` required sha256/size fields | PASS |
| Asset count guard | Requires ≥4 ChimeraCodex files before draft | PASS |

## Decision

Step 4 failure-mode review pass.
