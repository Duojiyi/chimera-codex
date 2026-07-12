# Task 1 Step 3 — Audit A (Requirements / Spec)

**Status:** pass

## Evidence

对照 Spec §4.1 + Plan Task 1 Step 3 与实际文件：

| 要求 | 证据 |
|------|------|
| 真相源 `brand/product.toml` | 存在；注释声明单一真相源 |
| 生成文件 DO NOT EDIT | `branding.rs` / `branding.generated.ts` 首行含 `@generated ... DO NOT EDIT BY HAND` |
| DISPLAY_SILENT_NAME | `Chimera Codex` |
| DISPLAY_MANAGER_NAME | `Chimera Codex 管理工具` |
| PUBLISHER | `ChimeraHub` |
| REPOSITORY | `Duojiyi/chimera-codex` |
| ADS_ENABLED | `false` |
| DEFAULT_RELAY_BASE_URL | `https://api.chimerahub.org/v1` |
| DEFAULT_RELAY_MODEL | `gpt-5.5` |
| ARTIFACT_PREFIX | `ChimeraCodex` |
| macos_build_number | `1`（正整数）；生成 `MACOS_BUILD_NUMBER: u32 = 1` |
| LATEST_JSON_URL | `https://github.com/Duojiyi/chimera-codex/releases/latest/download/latest.json` |
| 无 TBD/example/chimera-org/上游 release URL | product.toml / 生成文件 / 脚本占位扫描均无命中 |
| `-Check` 不改工作树 | `if ($Check)` 仅写临时目录并 `Compare-FilesExact`；工作树写入仅在非 Check 分支 |
| `lib.rs` 字母序 | `assets` → `branding` → `bridge` |
| 生成脚本 | `scripts/generate-branding.ps1` 从 TOML 确定性生成 Rust/TS |

## Findings

- Spec §4.1 与 Plan Step 3 对 Task 1 Files 范围内的最小实现均已满足。
- Plan 文中「NSIS/DMG/Actions 改读 TOML」属后续 Task 6/8 接线；不在 Task 1 Files 列表，不作为本 Step 未关闭问题。
- 一期未在 branding 中改 bin 名（无 `codex-plus-plus` 二进制改名字段），符合 Spec。

## Open issues

- 无
