# Task 3 Step 2 — Audit B (Diff / Boundaries)

> Status: **PASS**
> Date: 2026-07-10
> Auditor: B (diff / boundaries / regression surface)
> Scope: recommendations removal call-chain completeness

## Evidence

- Call-chain cleaned: navigate branch, actions.refreshAds, route render, subtitle map, Ad* helpers.
- Dead CSS for `.recommend-hero` / `.ad-*` removed with page.
- Local scripts UI still gated by `SCRIPT_MARKET_DISABLED`; market hide path intact.

## Findings

- No dangling TypeScript references to Ads types after removal; `tsc --noEmit` green later in Task 3.

## Open issues

- Non-blocking: backend `load_ads` handler retained for compatibility.
