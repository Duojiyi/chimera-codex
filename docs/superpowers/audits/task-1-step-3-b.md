# Task 1 Step 3 — Audit B (Implementation / Security / Drift)

## Status
PASS

## Evidence
### 真相源 `brand/product.toml`
- 含 Plan 要求字段：显示名、Publisher、`repository=Duojiyi/chimera-codex`、`latest_json_url`、`ads_enabled=false`、ChimeraHub URL、`default_relay_model=gpt-5.5`、`artifact_prefix=ChimeraCodex`、`macos_build_number=1`（正整数）、`website_url`/`api_key_url`。
- 扫描无 token/secret/password/`ghp_`/`sk-` 等密钥；仅有 `api_key_url`（获取 Key 的页面 URL，非凭据）。

### `scripts/generate-branding.ps1` 门禁（静态）
- 占位门禁：`TBD` / `example` / `chimera-org` / `BigPizzaV3/CodexPlusPlus` 扫关键字符串字段。
- repository 硬约束：必须精确 `Duojiyi/chimera-codex`。
- latest URL 推导：`https://github.com/$repository/releases/latest/download/latest.json` 并强制相等。
- `ads_enabled` 必须为 `$false`。
- `-Check`：写入临时目录，`Compare-FilesExact` 逐字节比较，不改工作树；`finally` 清理临时目录。
- `Write-Utf8NoBom`：`UTF8Encoding($false)`。
- 生成物实测：`branding.rs` / `branding.generated.ts` 均无 BOM。

### 生成一致性 / 手改
- 生成文件含 `@generated ... DO NOT EDIT BY HAND`。
- 字段与 toml 一一对应（含 `WEBSITE_URL`/`API_KEY_URL`/`MACOS_BUILD_NUMBER`）。
- `lib.rs` 已按字母序插入 `pub mod branding;`（位于 `assets` 与 `bridge` 之间）。

### 一期二进制名
- branding toml/rs/ts/脚本中无 bin 改名字段。
- `install` 仍为 `codex-plus-plus*`（未纳入 branding 改名）。

### 运行验证（只读）
- `pwsh -File scripts/generate-branding.ps1 -Check` → `PASS`
- `cargo test -p codex-plus-core --test branding` → 1 passed

## Findings
- 实现满足 Task 1 Step 3 的核心可观察行为与安全门禁；无密钥写入 branding。
- Plan 文案提到 NSIS/DMG/Actions 改读 TOML：不在 Task 1 Files 列表，属后续安装/发布任务。

## Open issues
- 低优先级：字符串转义缺少回归测试；当前品牌值无特殊转义字符，不构成 FAIL。
- 低优先级：`website_url` 与 `api_key_url` 同为 `https://api.chimerahub.org/`——属产品 URL 真实性议题，非本 Step 实现缺陷。
