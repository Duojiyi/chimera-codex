# Task 4 Step 2 — Audit A (Requirements)

> Status: **PASS**
> Date: 2026-07-10
> Auditor: A (requirements / observable behavior)
> Scope: Remove promo presets and invite-code URLs

## Evidence

- `jojocode` / `jojocode-max` removed from `presets.ts`.
- SiliconFlow invite path `/i/drGuwc9k` replaced with official account AK page.
- Zhipu invite query `ic=RRVJPB5SII` replaced with official open.bigmodel.cn API key entry.
- Preset scan test asserts absence of jojocode and invite fragments.

## Findings

- Plan Step 2 observable contract met.

## Open issues

- None for Step 2.
