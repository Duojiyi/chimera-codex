# Task 12 Aggregate Audit A - T31

> 结论：**PASS**
> 日期：2026-07-12
> 审计口径：只按产品需求、Plan、测试证据与用户可观察行为查漏

## 独立性声明

本审计独立复核 Task 12 / T31，未读取、引用或依赖 `task-12-aggregate-b.md`，也未与另一聚合审计交换结论。审计期间只修改本记录，没有修改实现、测试、Plan 或 TODO。

## 检查范围

- 对照 `2026-07-11-chimera-plus-product-refresh-design.md`、Plan Task 12、TODO T31，复核普通用户界面、注入菜单、推荐运行链、i18n 与后台 updater 边界。
- 逐项核对 Step 12.1-12.3 的 evidence、最终 A/B 记录和当前工作树，确认各 Step 的 Red、Green、针对性回归与最终行为可以互相追溯。
- 反向扫描 Manager `src/`、入口 HTML、Tauri command surface、renderer/stepwise 注入资产、core bridge/assets/update 与生产扫描器，重点寻找固定或动态 GitHub 外链、About/Issues 入口、手动更新控件、推荐/赞助/交流群、repository globals、第三方图标引用和 updater 误删。
- 复跑生产扫描、自测、allowlist 防绕过、i18n、品牌生成、核心/Manager/Launcher 回归、TypeScript、Vite、格式和差异门禁。

## 需求与可观察行为结论

### Manager UI 与维护能力

- `Route`、导航、subtitle 和页面渲染均不再包含 About；旧 `showUpdate=1` 与 `#about` 入口明确迁移到 `maintenance`。
- 普通 UI 不再提供项目主页、Issues、GitHub、手动“检查更新”、手动下载安装包或动态 `script.homepage` 外链；更新失败提示不透传 backend 自由文本。
- 日志、诊断和只读 `updateInstallProgress` 均在安装维护页可达，删除 About 没有删除故障排查能力。

### 注入菜单与运行时推荐链

- 最终 injection script 不再接收或定义 repository 名称/URL globals，不含 About、Issues、“提出问题”、项目 URL、专用 DOM/CSS/handler 或旧 `Chimera Codex` fallback。
- `/ads` 已回到 unknown path；Manager `load_ads` / `AdsPayload` / payload helper、bridge runtime trait、launcher 实现及 `ads_enabled` 品牌开关均退出运行链。
- script-market payload 与 renderer inventory 不再向客户 surface 暴露 homepage；内部历史元数据字段只用于旧配置和安装兼容，当前没有 UI consumer。

### 扫描器与更新边界

- 生产扫描器覆盖推荐、赞助、社区导流、大小写不敏感 GitHub UI、通用 GitHub URL、动态 homepage sink、generated URL consumer，以及 ChatGPT/OpenAI/Microsoft Store 的 icon/logo/图标/徽标/标志变体。
- Manager `src/`、`index.html` 与注入资产都受 customer UI 门约束；固定后台更新 URL 仅在 `branding.generated.ts:7` 通过精确路径、行号和全文 allowlist。
- `DEFAULT_REPOSITORY`、`DEFAULT_LATEST_JSON_URL`、`check_for_update` 和 manifest asset URL/身份校验仍保留。Task 12 删除客户可见 GitHub 表面，没有削弱 Task 13 所需的后台更新基础设施。
- i18n 字典、调用点与 manifest 精确一致，About/GitHub/手动更新产生的 23 个死键已全部关闭。

## 命令证据

- `pwsh -NoProfile -File scripts/verify-no-upstream-ads.ps1`：PASS。
- `pwsh -NoProfile -File scripts/verify-no-upstream-ads.ps1 -SelfTest`：PASS；customer surface、`docs/images`、`assets/images` fail-closed fixtures 均通过。
- `pwsh -NoProfile -File scripts/test-verify-allowlist.ps1`：PASS。
- `node tools/i18n-verify.mjs`：plain `563/563`、template `36/36`，manifest 同步一致。
- `pwsh -NoProfile -File scripts/generate-branding.ps1 -Check`：PASS。
- `cargo test -p codex-plus-core --test ads --test branding --test bridge_routes --test cdp_bridge --test updater --locked`：`1 + 3 + 26 + 69 + 36` passed，`0 failed`。其中 bridge lifecycle 用例约 116 秒后正常通过，不存在挂起测试。
- `cargo test -p codex-plus-manager --locked`：lib `53 passed`、`windows_subsystem 42 passed`，bin/doc tests `0 failed`。
- `cargo test -p codex-plus-launcher --locked`：`8 passed`。
- `npm run check`：PASS。
- `npm run vite:build`：PASS，`1608 modules transformed`。
- `cargo fmt --all -- --check`：PASS。
- `git diff --check`：PASS；只有现有工作树行尾转换提示，没有 whitespace error。

## 发现与剩余风险

- **阻断发现：无。** 当前实现满足 Task 12 / T31 的产品边界与可观察行为要求。
- `apps/codex-plus-manager/src/styles.css` 仍有未引用的 `.update-dot` / `.update-dot:hover` 死样式；全局搜索只有这两个 CSS 定义，没有 DOM、组件或状态 consumer，因此不会显示手动更新提示点，不构成可观察行为缺口。后续可作为非行为样式清理移除。
- Manager 与注入菜单的主要验收仍是源码/最终拼接文本契约和构建回归，没有真实 WebView 点击及截图级验证。因为相关 DOM、handler、输入 globals 和 route 已从最终产物删除，此风险不阻断本 Gate；Task 16 的实机最终验收仍需覆盖真实安装后的界面。
- Task 13 的最低支持版本和自动安装状态机、Task 14 的原创图标替换、Task 15 的远端同步/Release、Task 16 的 Windows/macOS 实机冒烟均未由本审计提前宣称完成。

## Gate 决定

独立审计 A 判定 **PASS**。只有独立聚合审计 B 同样 PASS 后，才可更新 `task-12-aggregate.md` 并勾选 TODO `T31`；本结论不表示产品已经发行就绪。
