# Task 4 Step 1 — Audit A (Requirements)

> Status: **PASS**
> Date: 2026-07-10
> Auditor: A (requirements / observable behavior)
> Scope: Add complete ChimeraHub preset

## Evidence

- `presets.ts` includes `id: "chimerahub"`, name ChimeraHub, `protocol: "responses"`, `modelList` with branding default model.
- Base URL / website / apiKey URLs come from `branding.generated.ts` (`DEFAULT_RELAY_BASE_URL`, `WEBSITE_URL`, `API_KEY_URL`), so `/v1` is present.
- `windows_subsystem` test `provider_presets_include_chimerahub_and_drop_jojo_promo` passes.

## Findings

- Plan Step 1 observable contract met.

## Open issues

- None for Step 1.
