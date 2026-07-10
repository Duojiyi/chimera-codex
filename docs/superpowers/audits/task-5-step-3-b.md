# Task 5 Step 3 — Audit B (Diff / boundary)

> Status: **PASS**
> Date: 2026-07-10
> Auditor: B (diff / boundary / regression)
> Scope: Constant wiring and removed GitHub API release parser

## Evidence

- `DEFAULT_*` are aliases of `crate::branding::*` (compile-time, not runtime string literals to upstream).
- `release_from_github_payload` removed; only `release_from_latest_json_payload` remains for manifest parsing.
- `fetch_latest_release` / `check_for_update` have a single URL argument path with no secondary upstream URL.

## Findings

- Call sites in manager/launcher still invoke `check_for_update` without alternate URL injection.

## Open issues

- None for Step 3.
