# Task 12 / T31 Independent Aggregate Audit B

## Conclusion

**PASS**

基于当前最终工作树，独立从 diff、边界条件、回归面与发布风险复核 Task 12（Step 12.1-12.3）后，未发现阻断 T31 关闭的问题。Manager 普通用户界面已无 About route/nav/screen、项目主页、Issues、GitHub 入口和手动更新控件；日志、诊断与只读安装进度已迁入维护页；注入菜单和 bridge 不再暴露 About/Issues/repository/广告链路；生产扫描器、i18n 精确门禁和后台 updater 信任回归全部通过。

本审计未读取或引用 `task-12-aggregate-a.md`，也未与另一聚合审计沟通。审计只新增本文件，未修改实现、Plan、TODO 或其他文件。

## Scope Reviewed

- 对照产品刷新规格的第 2、4、7 节、Plan Task 12、TODO T31，核对“普通 UI 去 GitHub/推广”和“后台更新基础设施继续保留”的边界。
- 核对 Step 12.1-12.3 的 evidence、A/B 记录、最终源码和对应静态/运行时测试；Step 记录均有 Red、Green、针对性回归和双盲结论。
- 检查 Manager route、导航、维护页、启动深链、更新状态/进度、动态外链 consumer、Tauri command 注册和 script-market payload。
- 检查 renderer/stepwise 注入资产、assets 拼接、bridge routes/runtime trait/launcher、广告 module export 与品牌开关。
- 检查生产 scanner 的客户 UI 路径分类、大小写匹配、generated definition/consumer 分界、精确 allowlist、图片目录 fail-closed 规则及自测 fixture。
- 复跑 updater 全量回归，确认固定 Chimera Release 源、版本化资产身份、平台/架构、大小、SHA-256、超时和下载发布边界没有因移除可见 GitHub UI 被削弱。

## Findings

未发现阻断项。

### UI and routing boundary

- `Route`、导航和 screen dispatch 不包含 `about`；旧 `showUpdate=1` 与 `#about` 只兼容迁移到 `maintenance`，不会恢复 About 页面。
- Maintenance 同时承接日志、诊断和只读 `updateInstallProgress`；不存在“检查更新”“下载并运行安装包”或 update dot 等手动触发控件。
- `checkUpdate` 不向普通 UI透传 backend raw message。内部 `performUpdate` 准备逻辑仍为后续自动更新接线保留，但当前没有按钮/action consumer，不构成手动更新表面。
- Manager 唯一现存 `openExternalUrl` 客户调用用于 ChimeraHub Key 页面；动态 `script.homepage` 不再进入前端类型、payload 或外链 sink。

### Injection and recommendation boundary

- 两个注入脚本中未命中 About/关于、Issues/提出问题、GitHub、项目主页、推荐、赞助或交流群入口。
- Manager backend 无 `AdsPayload`、`load_ads`、`ads_payload` 或 command registration；script-market payload 不发送 homepage。
- Core module tree 不再导出 ads，runtime trait/launcher/routes 不再接线 `/ads`；负向行为要求 `/ads` 返回 unknown path，而不是隐藏成功响应。
- `crates/codex-plus-core/src/ads.rs` 是未导出的空兼容 tombstone。它没有运行时可达性；单文件删除仍受仓库“删除前确认”规则约束，不阻断当前行为门。

### Scanner and release boundary

- customer UI 路径覆盖 Manager `index.html`、全部 `src/` 和注入资产；GitHub 与第三方 icon/logo/徽标/标志规则使用大小写不敏感匹配。
- generated `LATEST_JSON_URL` 定义与 UI consumer 分开处理。唯一 allowlist 是 `apps/codex-plus-manager/src/branding.generated.ts:7` 的固定后台 URL，绑定精确路径、pattern、行号和完整行；移动、改写、重复消费或未使用都会失败。
- docs/assets 图片目录采用精确文件名 allowlist，新文件、改名、嵌套和大小写变化 fail closed。
- 后台 `REPOSITORY`、`LATEST_JSON_URL`、manifest/asset 身份校验保持可用；普通 Manager/注入 UI 不能消费这些生成常量，也不显示 GitHub/Release 实现入口。

## Independent Command Evidence

- `pwsh -NoProfile -File scripts/verify-no-upstream-ads.ps1`：PASS。
- `pwsh -NoProfile -File scripts/test-verify-allowlist.ps1`：PASS；allowlist、docs/images、assets/images、customer surface fixture 全部通过。
- `node tools/i18n-verify.mjs`：plain `563/563`、template `36/36`，manifest 同为 `563/563` 与 `36/36`。
- `pwsh -NoProfile -File scripts/generate-branding.ps1 -Check`：PASS。
- `npm run check`（`apps/codex-plus-manager`）：PASS。首次从仓库根目录调用因根目录没有 `package.json` 得到预期 ENOENT，切换到实际前端目录后通过，不是产品或测试失败。
- `npm run vite:build`（`apps/codex-plus-manager`）：PASS，`1608 modules transformed`。
- `cargo test -p codex-plus-core --test ads --test branding --test bridge_routes --test cdp_bridge --test updater --locked`：ads `1`、branding `3`、bridge_routes `26`、cdp_bridge `69`、updater `36`，全部通过。
- `cargo test -p codex-plus-manager --locked`：lib `53`、windows_subsystem `42`，bin/doc tests `0 failed`。
- `cargo test -p codex-plus-launcher --locked`：`8 passed`。
- `cargo fmt --all -- --check`：PASS。
- `git diff --check`：PASS；仅有现有工作树行尾转换提示，没有 whitespace error。

## Remaining Risks and Deferred Work

- 当前 UI/注入验收以源码契约、最终拼接文本、TypeScript 和生产构建为主，没有执行真实 Codex WebView 的点击/截图级验收。删除入口的 DOM、样式、handler、globals 和 bridge route 均已消失，风险可接受；实机 UI 冒烟仍应在 Task 16 完成。
- scanner 属于文本和路径门禁，不能证明二进制图标原创或视觉不相似。原创主 SVG、ICO/PNG/ICNS 全平台替换、provenance 和像素/目视验证仍属于 Task 14。
- Task 12 只保留并验证后台 updater 基础设施。启动自动下载、`minimum_supported_version`、强制更新阻断与平台安装状态机仍属于 Task 13；本 PASS 不表示这些行为已经完成。
- 本轮未推送、未发布，也未验证远端 GitHub Release。远端同步和发布门仍属于 Task 15/16。

## Gate Decision

从独立审计 B 侧，Task 12 已满足 T31 聚合关闭条件；仍须聚合审计 A 独立通过后，才可更新 `task-12-aggregate.md` 并勾选 T31。
