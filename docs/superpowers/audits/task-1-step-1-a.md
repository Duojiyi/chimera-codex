# Task 1 Step 1 — Audit A (Requirements / Spec)

**Status:** pass

## Evidence

- Spec §4.1 要求生成 Rust/TS 品牌常量，并由测试/门禁约束公开仓库与去上游。
- Plan Task 1 Step 1 规定 integration test：`crates/codex-plus-core/tests/branding.rs`，断言 ADS_ENABLED、显示名、Publisher、REPOSITORY/LATEST_JSON_URL、DEFAULT_RELAY_*、ARTIFACT_PREFIX。
- 实际文件存在且覆盖 Plan 样例全部断言；并额外断言 `chimera-org`/`example`、精确 `Duojiyi/chimera-codex` 与精确 latest.json URL（强于样例，仍满足验收）。

## Findings

- 失败测试（integration test）已按 Plan 落地，路径与可观察行为正确。
- 测试不断言 `MACOS_BUILD_NUMBER`：与 Plan Step 1 样例一致；`macos_build_number` 属 Step 3 / product.toml 范围，不构成本 Step 失败。

## Open issues

- 无
