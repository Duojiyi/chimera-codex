# Task 12 Step 12.3 Audit A

## Independent requirements and behavior audit

结论：**PASS**。

本轮只按产品规格、Plan Step 12.3、TDD evidence、当前 scanner/运行链/i18n 与可观察行为独立检查；未读取或引用审计 B，也未修改实现、Plan、TODO 或其他文件。审计发现的通用 GitHub、大小写、UI 路径和动态 consumer 绕过均已按 TDD 关闭，最终快照未发现新的阻断。

## Scanner coverage

### Promotion and customer surfaces

- 生产扫描规则覆盖推荐命令/route/module/payload、广告开关、广告列表、sponsor runtime、Discord/Telegram/交流群/QQ群/微信群、repository global、Issues hook 和动态脚本主页。
- 客户 UI 的 `github` 匹配使用大小写不敏感规则，覆盖 `github.com/...`、`GitHub`、`GITHUB.COM` 等大小写形式。
- customer UI path 包含 Manager `src/`、`apps/codex-plus-manager/index.html` 和全部 `assets/inject/`，不会因入口 HTML 位于 `src` 外而绕过。
- generated branding definition 与实际 UI consumer 被分开处理：固定后台 `LATEST_JSON_URL` 定义只通过精确路径、行号和完整行 allowlist；实际 customer consumer 额外禁止 `REPOSITORY`、`LATEST_JSON_URL` 和任意 `script.homepage` 使用。
- allowlist 仍要求精确路径、pattern、行号、完整 trimmed line 且只能消费一次；移动、改写、大小写变化或未使用条目都会 fail closed。

### Third-party icon references

- 规则覆盖 ChatGPT/OpenAI/Microsoft Store 的 `icon`、`logo`、中文“图标”“徽标”“标志”及大小写变体。
- `docs/images` 与 `assets/images` 采用精确文件名 allowlist；新增、重命名、嵌套或大小写变化的图片会被拒绝。
- self-test 包含 Microsoft Store ChatGPT icon、lowercase `chatgpt logo` 与中文 ChatGPT 徽标 fixture，证明不同表达不会绕过规则。
- 本 Step 只阻止第三方图标来源/提取表面和未批准资产进入；原创主图设计、像素相似性和全平台替换仍属于 Task 14。

### Self-test integrity

- scanner 的真实规则由 `Get-CustomerSurfacePatterns` 提供，生产扫描和 self-test 共用同一组规则。
- self-test 覆盖 recommendation、sponsor、community、GitHub UI、通用 GitHub URL、第三方 icon/logo/badge、生成 URL consumer、动态 homepage sink 与 `index.html` 路径分类。
- 审计新增绕过 fixture 后，self-test 先 Red 并准确命中缺失规则/路径，再实现共享大小写 matcher 和 UI consumer 边界；最终 self-test 与真实扫描均 PASS。

## Advertisement runtime removal

- Manager backend 已无 `AdsPayload`、`load_ads`、`ads_payload`、推荐文案或 command registration。
- core 已无 `pub mod ads`、runtime ads trait/implementation、launcher forwarding、广告列表和 sponsor append 调用；`src/ads.rs` 仅保留未导出的空兼容 tombstone。
- bridge `/ads` 当前返回 `Unknown bridge path`，不会继续提供隐藏广告 payload。
- `brand/product.toml`、branding generator、Rust/TypeScript 生成品牌文件均无 `ads_enabled` / `ADS_ENABLED`，广告能力不能通过旧开关重新启用。
- README 客户文案使用中性的“社区导流入口”，生产扫描无推荐、赞助或交流群残留。

## I18n closure

- About、项目主页、GitHub Release、手动检查/下载和旧资源标签产生的 `19` 个 plain 与 `4` 个 template 死键已删除。
- `tools/i18n-keys.json` 由 codemod 重建，未发现手工漏项或多余 manifest 项。
- 最终 verifier：plain `563 referenced / 563 translated`、template `36 / 36`；manifest 同为 `563 / 563` 与 `36 / 36`。
- 字典与所有 `t()` / `tf()` 调用精确一致，不再依赖“允许 stale key”的临时边界。

## Updater boundary

- 后台 `DEFAULT_REPOSITORY`、`DEFAULT_LATEST_JSON_URL`、`check_for_update` 和 manifest asset 身份校验仍存在。
- 固定 GitHub Release URL 仅保留在 branding/updater 发布基础设施和精确 allowlist 定义行，普通 Manager UI、入口 HTML与注入资产均不能消费或展示它。
- scanner 加强没有误删、放宽或误报后台 updater 信任模型；品牌生成 `-Check` 继续 PASS。

## TDD evidence review

- scanner self-test 初始因缺 customer-surface fixture Red；补基础规则后真实扫描继续以 14 项实际残留 Red。
- bridge、Manager 和 branding 契约分别先命中 `/ads`、推荐 command surface 与 `ads_enabled`，随后完成最小运行链删除。
- 审计 A 补充的 generic GitHub、logo/徽标、大小写、`index.html`、generated consumer 和动态 sink fixture 均先暴露绕过，再由共享匹配器和 consumer path 规则关闭。
- evidence 的 Red/Green、最终计数、scanner 输出与当前源码结构一致。

## Independent verification

- `cargo test -p codex-plus-core --test ads --locked`：`1 passed`。
- `cargo test -p codex-plus-core --test branding --locked`：`3 passed`。
- bridge `/ads` 负向契约：`1 passed`；evidence 的完整 `bridge_routes` 为 `26 passed`。
- Manager 推荐 command 负向契约：`1 passed`；evidence 的完整 Manager 为 lib `53 passed`、`windows_subsystem 42 passed`。
- evidence 的完整 `cdp_bridge`：`69 passed`；launcher：`8 passed`。
- `node tools/i18n-verify.mjs`：`563/563` plain、`36/36` template，manifest 同步一致。
- scanner `-SelfTest`、真实扫描和 `scripts/test-verify-allowlist.ps1`：PASS，customer surface、docs/images、assets/images fixture 均 OK。
- `pwsh -NoProfile -File scripts/generate-branding.ps1 -Check`：PASS。
- 聚焦 `git diff --check`：PASS；仅有 Git 行尾转换提示，无 whitespace error。

## Deferred scope

- Task 14 仍需生成原创 Chimera++ 主图并执行像素、视觉相似性、多尺寸及全平台资源验证；当前 scanner 不替代该验收。
- Task 13 负责最低支持版本、自动更新决策和平台安装状态机；本 Step 只保证后台更新基础设施保留且不暴露给普通 UI。
- 本 Step 不推送、不发布，也不表示 Task 12 聚合已经完成。

从审计 A 侧，Step 12.3 已满足关闭条件；仍须按项目流程由独立审计 B 同样通过后才能勾选该 Step。
