# Task 7 Step 2 Audit B — Scanner gate (diff / boundary)

> Status: **PASS**
> Date: 2026-07-10
> Scope: Implementation pitfalls and allowlist tightness

## Implementation notes

- Matching uses `String.Contains` (case-sensitive) to avoid PowerShell `-like` false positives on `Codex++ manager` comments.
- Nested `apps/**/dist` and `src-tauri/gen` are excluded so build outputs cannot red-fail CI.
- Test trees are exempt from production-only promo patterns; fixtures that live inside `src/**` `#[cfg(test)]` must be allowlisted line-family style.

## Allowlist review

| Entry | Why allowed | Whole-dir? |
|---|---|---|
| `provider_import.rs` + jojocode.com | cfg(test) URL fixture | No |
| `install/mod.rs` + Codex++ 管理工具 | LEGACY_* constants | No |
| README* migration strings | User upgrade docs | No |
| NSIS Delete legacy shortcuts | Cleanup only; CreateShortcut is Chimera | No |
| package-dmg.sh migration README | Legacy app names in DMG notes | No |
| workflow `CodexPlusPlus-` zip names | Pending Task 8 rename | No |

## Residual risk

- Workflow zip names still upstream-prefixed until Task 8 clears allowlist entries.
- Do not broaden allowlist to directories.

## Decision

Scanner boundaries are tight enough for Task 7; remaining packaging zip rename is explicitly deferred.
