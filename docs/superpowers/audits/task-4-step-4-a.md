# Task 4 Step 4 — Audit A (Requirements)

> Status: **PASS**
> Date: 2026-07-10
> Auditor: A (requirements / observable behavior)
> Scope: Key-first atomic save-and-enable + UI

## Evidence

- Command `save_and_enable_chimera_hub` rejects empty/whitespace Key; does not create settings or mutate live config/auth.
- Valid Key saves ChimeraHub profile (active + PureApi + `/v1`), writes config/auth via existing apply path, returns configured.
- Error/success messages do not echo the Key.
- UI: Relay screen shows Key input +「保存并启用」only in Chimera key-first state; empty Key disables button; existing non-empty Key users do not see auto switch panel.

## Findings

- Plan Step 4 observable contract met.

## Open issues

- None for Step 4.
