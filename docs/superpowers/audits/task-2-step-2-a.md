# Task 2 Step 2 — Audit A (RED)

> Status: **PASS**
> Date: 2026-07-10
> Auditor: A (requirements / observable behavior)
> Scope: RED evidence before implementation

## Evidence

- Terminal: `terminals/535075.txt`
- Command: `cargo test -p codex-plus-core --test ads`
- Result: **3 passed; 5 failed** (`exit_code: 101`)

Failed tests:

1. `production_ad_list_urls_are_empty`
2. `ads_disabled_returns_empty_list_without_network`
3. `normalizes_remote_ads_without_appending_builtins`
4. `normalize_does_not_append_builtin_sponsors_after_remote_sponsors`
5. `fetch_ad_list_tries_backup_url_when_primary_fails`

## Findings

- RED phase confirmed: new/updated ads assertions fail against pre-change implementation as expected.
- Failure set aligns with empty URLs / no-builtins / short-circuit requirements.

## Open issues

- Non-blocking: inject dead ad render code (out of RED ads scope).
- Non-blocking: `ads.rs` remote logo includes for normalize fixtures.
