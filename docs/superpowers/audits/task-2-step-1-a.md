# Task 2 Step 1 — Audit A (Requirements)

> Status: **PASS**
> Date: 2026-07-10
> Auditor: A (requirements / observable behavior)
> Scope: ads + cdp tests for empty URLs, no builtins, no sponsor inject

## Evidence

- `crates/codex-plus-core/tests/ads.rs`: covers `DEFAULT_AD_LIST_URLS` empty, ads-disabled empty list, normalize without builtins, no append after remote sponsors, backup-URL path without builtin ids
- `crates/codex-plus-core/tests/cdp_bridge.rs`: `injection_script_has_no_ad_list_or_recommendation_ui`, `injection_script_prefixes_helper_url_without_sponsor_images`

## Findings

- Requirements for empty production ad URLs, no builtin sponsor append, and no Ad-List / sponsor inject UI are expressed as failing-then-passing tests.
- Observable contracts match Task 2 plan Step 1.

## Open issues

- Non-blocking: inject script may still contain dead ad-render helpers unused by UI paths.
- Non-blocking: `ads.rs` retains remote logo include paths for normalize fixtures.
