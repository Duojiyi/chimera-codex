# Task 7 Step 2 Audit A — Scanner gate (requirements)

> Status: **PASS**
> Date: 2026-07-10
> Scope: `scripts/verify-no-upstream-ads.ps1` + `scripts/verify-allowlist.txt`

## Requirements checklist

| Requirement | Evidence | Result |
|---|---|---|
| Scan production src, root README, packaging, workflows | `scanRoots` covers README/brand/apps/crates/assets/scripts/.github | PASS |
| Exclude `.git`/`target`/build/docs history | Excludes nested `dist|target|node_modules|gen`, `docs/superpowers` | PASS |
| Narrow allowlist, no whole-dir exemption | `path:pattern:reason` lines only | PASS |
| Ban Ad-List / ScriptMarket / jojocode in production | ProductionOnly patterns | PASS |
| Ban `append_builtin_sponsors(` calls | Regex on production `.rs` | PASS |
| Ban upstream URL in `update.rs` | Explicit file check | PASS |
| Ban sponsor image inject symbol in production | `__CODEX_PLUS_SPONSOR_IMAGES__` | PASS |
| Ban brand placeholders | product.toml token scan | PASS |
| Ban user-visible Manager legacy names except allowlisted legacy | ProductionOnly + allowlist | PASS |
| Version sync Cargo/package/tauri | Compared workspace version | PASS |
| Packaging artifact/brand drift | OutFile/Publisher + CodexPlusPlus- zip check | PASS |

## Decision

Scanner implements Task 7 gate requirements.
