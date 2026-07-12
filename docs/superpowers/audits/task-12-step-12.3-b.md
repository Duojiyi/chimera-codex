# Task 12 Step 12.3 独立审计 B

## 结论

**PASS**

基于当前最终工作树，独立按 diff、生产扫描边界、运行时调用链、回归面与测试充分性复核后，未发现阻断 Step 12.3 关闭的问题。推荐/赞助/交流群/GitHub UI 的生产表面已清理，广告命令与 core bridge/launcher 链路已断开，i18n 字典和 manifest 恢复精确一致；最终 scanner remediation 也关闭了大小写 GitHub、manager `index.html`、generated 常量 consumer 与动态 homepage sink 的绕过面。

本审计未读取任何 Step 12.3 审计 A 文件，也未引用其他 A 审计结论。

## 独立复核

### 1. Scanner 规则与 fail-closed 自测

- `Get-CustomerSurfacePatterns` 同时供真实扫描与 self-test 使用，不存在自测一套规则、生产扫描另一套规则的分叉。
- 客户 UI 中任意 `github` 大小写变体均按 `OrdinalIgnoreCase` 匹配；路径范围覆盖 manager `src/`、manager `index.html` 和 `assets/inject/`，避免入口 HTML 或注入脚本成为旁路。
- generated definition 与实际 consumer 被分开处理：`branding.generated.ts` 中固定后台 `latest.json` URL只允许精确路径、行号和完整行；其他客户 UI 文件使用 `REPOSITORY`、`LATEST_JSON_URL` 或 `script.homepage` 均会失败。因此改写为 `href={script.homepage}`、不同打开函数或导入 generated URL 常量不能绕过旧的单一调用 token。
- ChatGPT/OpenAI/Microsoft Store 的 icon/logo/徽标/标志规则使用大小写不敏感匹配；self-test 包含大写 `GITHUB.COM`、小写 `chatgpt logo`、中文徽标、generated URL consumer、动态 homepage sink 和 manager `index.html` 分类 fixture。
- docs/assets 图片目录采用文件名精确 allowlist，新文件、改名、嵌套或大小写变化默认拒绝；文本 allowlist 同时绑定精确相对路径、pattern、行号和完整 trimmed line，重复消费与未使用条目均失败。

### 2. Manager 广告与动态入口

- Manager backend 不再定义 `AdsPayload`、`load_ads` 或 `ads_payload`，Tauri command 注册表也不再暴露广告命令。
- 前端没有广告状态、推荐面板或广告加载调用；旧 About/GitHub/手动更新相关 i18n 键已删除。
- script-market payload、前端类型和 user-script inventory 不再向 renderer 暴露 homepage。内部脚本安装元数据仍可保存 homepage，用于读取旧配置与安装兼容，但该字段不会穿过客户 UI/bridge 数据边界。
- `presets.ts` 中的源码仓库 URL 已删除；manager customer source 对通用 GitHub 规则仅保留 generated `latest.json` 的精确单行豁免。

### 3. Core、bridge 与 launcher 链路

- core `lib.rs` 不再 `pub mod ads`；runtime trait 与 launcher implementation 不再提供 ads 方法；routes 不再分派 `/ads`。
- `/ads` 的行为回归明确要求返回 `Unknown bridge path`，不是空数组或隐藏成功响应，证明调用面已真正移除。
- `crates/codex-plus-core/src/ads.rs` 只剩一行空兼容 tombstone，且未被 module tree 导出。负向测试同时禁止 module export、route 字符串和 ads handler；未来重新接线会触发测试或 scanner。
- 品牌真相源、生成器、Rust branding 和 manager generated branding 均不再包含 `ads_enabled` / `ADS_ENABLED`，广告能力不能通过旧 feature flag 被重新打开。

### 4. i18n 与文档

- About、项目主页、GitHub Release、手动检查/下载和旧资源标签对应的 `19` 个 plain、`4` 个 template 死键已删除。
- 本次独立运行 `node tools/i18n-verify.mjs`：plain `563/563`、template `36/36`，manifest 同为 `563/563` 与 `36/36`，无 missing 或 stale。
- README 只以“去除推广、赞助和社区导流入口”描述产品状态，不保留可点击交流群或推广入口；“推荐步骤”和 Provider Doctor 的“处理建议”属于操作指引/诊断语义，不是广告推荐链，scanner 也没有误伤这些合法用法。

## Updater 与 Task 14 边界

- 后台 updater 的 `latest.json`、版本化 Release asset URL 和信任校验仍保留。固定 URL 在 generated source 中存在，但客户 UI consumer 被禁止；普通 UI 不显示 GitHub、Release、asset 或 SHA 实现术语。
- 第三方图标规则当前防止新增 ChatGPT/OpenAI/Microsoft Store 图标来源、提取说明和未批准图片文件；它不宣称已经完成原创图标设计、像素替换或视觉相似性审计。
- 原创 SVG、全平台 PNG/ICO/ICNS 替换、provenance 和多尺寸目视验证仍明确属于 Task 14。当前 scanner 的文本/文件清单门不能替代像素级或人工视觉审计。

## Evidence 与回归

`docs/superpowers/audits/task-12-step-12.3-evidence.md` 的最终 Red/Green、第二轮 scanner remediation、测试计数和边界说明与当前实现一致。独立复核结果：

- `cargo test -p codex-plus-core --test ads --test branding --locked`：ads `1 passed`、branding `3 passed`。
- `/ads` unknown-path 精确 bridge 回归：`1 passed`；evidence 记录完整 `bridge_routes 26 passed`、`cdp_bridge 69 passed`。
- Manager recommendation command surface 精确回归：`1 passed`；evidence 记录完整 lib `53 passed`、`windows_subsystem 42 passed`。
- `cargo test -p codex-plus-launcher --locked`：`8 passed`。
- `npm run check`：PASS；`npm run vite:build`：PASS，`1608 modules transformed`。
- `pwsh -NoProfile -File scripts/verify-no-upstream-ads.ps1`：PASS。
- `pwsh -NoProfile -File scripts/test-verify-allowlist.ps1`：PASS，customer surface、docs/images、assets/images fail-closed fixtures 均 OK。
- `pwsh -NoProfile -File scripts/generate-branding.ps1 -Check`、`cargo fmt --all -- --check`：PASS。
- 聚焦 `git diff --check`：PASS；仅有工作树换行符转换提示，没有 whitespace error。

剩余风险是 scanner 本质上属于文本和路径门禁，不执行浏览器 DOM 交互，也不检测二进制图标的视觉相似度。当前已通过 consumer 标识、运行时负向契约、真实生产扫描和完整前端构建形成多层覆盖；DOM/像素级最终验收分别由后续实机 Gate 与 Task 14 承担，因此不构成本 Step 阻断。
