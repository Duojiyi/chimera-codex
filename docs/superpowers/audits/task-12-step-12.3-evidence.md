# Task 12 Step 12.3 Evidence

## Scope

- 生产扫描器 fail closed 覆盖推荐、赞助、交流群、客户 GitHub UI、动态仓库/Issues 入口和第三方 ChatGPT/OpenAI 图标引用。
- 删除已无 UI 消费者的广告命令、bridge route、runtime trait、品牌开关和相关实现；`/ads` 返回未知路径。
- 清理 About/GitHub/手动更新/项目主页产生的全部 i18n 死键，字典与 manifest 恢复精确一致。

## Scanner Red And Green

- Red：`scripts/test-verify-allowlist.ps1` 先要求 `customer surface fail-closed fixtures: OK`；旧 scanner 自测因没有该门而 exit `1`。
- Green：新增 `Get-CustomerSurfacePatterns`，由真实扫描和 self-test 共用；五类 fixture 分别覆盖 recommendation、sponsor、community、GitHub UI 和 third-party icon。
- 自测 Green 后首次真实扫描继续 Red：`verify-no-upstream-ads: FAILED (14 finding(s))`，命中 README 交流群措辞、10 个 GitHub/反馈死键、Manager `load_ads`、core sponsor runtime、`pub mod ads` 和 bridge `/ads` route。
- 扫描规则同时禁止 `ads_enabled` / `ADS_ENABLED`、repository globals、Issues hook、动态 script homepage，以及中英文 ChatGPT/OpenAI/Microsoft Store 图标引用。
- 审计 A 发现通用 `github.com/...` 客户链接和 `logo/徽标/标志` 变体可绕过；新增 generic GitHub URL、OpenAI logo 与 ChatGPT 徽标 fixture 后 self-test Red，首先准确报告 generic GitHub URL bypass。
- Green 增加 customer UI 路径定向 `github.com/` / `GitHub` 规则及 icon/logo/徽标/标志变体；后台 `branding.generated.ts:7` 的固定 `latest.json` URL 使用精确路径、行号和全文 allowlist，真实扫描只允许该行。
- 最终复核继续发现大小写、`index.html`、生成常量 consumer 和动态 homepage sink 绕过；加入 `GITHUB.COM`、`chatgpt logo`、`LATEST_JSON_URL`、`href={script.homepage}` 与 index path fixture 后，self-test Red 命中 index 未分类。
- Green 引入共享 `Test-RuleContains` 大小写不敏感匹配，区分 generated definition 与实际 UI consumer；Manager `src/`、`index.html` 和注入资产均受通用 GitHub 门约束，consumer 额外禁止 `REPOSITORY`、`LATEST_JSON_URL` 和任意 `script.homepage` 使用。

## Behavior Red And Green

- Bridge Red：`runtime_status_and_devtools_routes_are_dispatched_without_ads_surface` 期望 `/ads` 为 unknown path，旧实现仍返回 runtime ad payload，`0 passed / 1 failed`。
- Manager Red：`manager_backend_has_no_recommendation_command_surface` 因 `AdsPayload` / `load_ads` / `ads_payload` / 推荐文案仍存在而 `0 passed / 1 failed`。
- Branding Red：`branding_source_has_no_ads_feature_toggle` 因 `brand/product.toml` 仍有 `ads_enabled` 而 `0 passed / 1 failed`。
- Homepage Red：扩展 Manager 契约后因 script-market payload 仍有 `"homepage": script.homepage` 而 `0 passed / 1 failed`；bridge inventory 契约改为 homepage 必须为 null 后同样 `0 passed / 1 failed`。
- Green 删除 Manager command/registration/payload、core module export、bridge route/trait/launcher implementation和品牌生成开关；`src/ads.rs` 只保留空兼容 tombstone，原 `tests/ads.rs` 改为负向运行时表面契约。
- Manager script-market payload、前端类型和 user-script inventory 不再暴露 homepage；内部安装元数据字段保留用于读取旧配置和脚本安装兼容。
- README 的客户说明改为“社区导流入口”，避免生产文档保留交流群关键词。

## I18n Cleanup

- 删除 `19` 个 plain 与 `4` 个 template 死键，包括 About、项目主页、GitHub Release、手动检查/下载和旧资源标签。
- 运行 `node tools/i18n-codemod.mjs` 机械重建 `tools/i18n-keys.json`。
- `node tools/i18n-verify.mjs`：plain `563 referenced / 563 translated`；template `36 / 36`；manifest 同为 `563 / 563` 与 `36 / 36`。

## Regression

- Core 聚焦回归：`ads 1 passed`、`branding 3 passed`、`bridge_routes 26 passed`、`cdp_bridge 69 passed`。
- Manager：lib `53 passed`、`windows_subsystem 42 passed`，bin/doc tests `0 failed`。
- Launcher：`8 passed`。
- `npm run check`：PASS；`npm run vite:build`：PASS，`1608 modules transformed`。
- `pwsh -NoProfile -File scripts/verify-no-upstream-ads.ps1`：PASS。
- `pwsh -NoProfile -File scripts/test-verify-allowlist.ps1`：PASS，输出 customer surface、docs/images 与 assets/images fail-closed fixtures 均 OK。
- `pwsh -NoProfile -File scripts/generate-branding.ps1 -Check`：PASS。
- `cargo fmt --all -- --check` 与 `git diff --check`：PASS；仅有现有 Git 行尾转换提示，无 whitespace error。

## Boundary

- 后台 updater 的 `latest.json`、版本化 Release asset URL 和信任校验未删除；普通用户 UI 不显示这些实现术语。
- 第三方图标规则防止未来提交 ChatGPT/OpenAI/Microsoft Store 图标来源或提取说明；原创图标生成和全平台资源替换仍归 Task 14。
- 本 Step 不推送、不发布。
