# Task 12 Step 12.2 Audit A

## Independent requirements and behavior audit

结论：**PASS**。

本轮只按产品规格、Plan Step 12.2、TDD evidence、当前注入资产与可观察行为独立检查；未读取或引用审计 B，也未修改实现、Plan、TODO 或其他文件。审计发现的 renderer 旧品牌 fallback 已按 TDD 修复，最终快照未发现新的阻断。

## Requirements review

### Injected customer surface

- 最终 `renderer-inject.js` 不再包含 About/Build/GitHub 信息区、Issues/“提出问题”按钮、点击处理器或专用 CSS。
- 注入菜单保留的“主页”是配置功能主面板，不是项目主页，也没有仓库或反馈外链。
- 打开管理工具、DevTools、配置开关、用户脚本、会话和插件增强仍保留；删除项目/反馈表面没有破坏既有产品能力。
- renderer 和 stepwise 注入资产中均无 `Chimera Codex` 旧显示名，菜单标题、插件标签、状态与诊断 fallback 统一为 `Chimera++`。

### Repository data boundary

- `assets.rs` 不再构造 renderer repository URL，也不再注入 `__CODEX_PLUS_REPOSITORY__` 或 `__CODEX_PLUS_REPOSITORY_URL__`。
- `renderer-inject.js` 不定义或读取 `codexPlusRepository`，最终拼接后的注入脚本也不包含本项目或上游仓库 URL。
- 精确扫描确认 renderer、stepwise 与 `assets.rs` 无 repository globals、About/Issues 类名、提出问题文案、GitHub Issues 或本项目仓库地址。
- `__CODEX_PLUS_BUILD__` 与 `codexPlusBuild` 继续用于 `script_loaded` 诊断，不再作为 About DOM 内容；诊断能力未被误删。

### Branding fallback

- 审计发现 renderer 的 `codexPlusDisplayName` fallback 仍为 `Chimera Codex`。该值会进入菜单标题、ARIA、插件市场标签和提示，属于可观察旧品牌残留。
- 加入禁止旧显示名的契约后，精确测试 Red 为 `0/1`；fallback 改为 `Chimera++` 后 Green 为 `1/1`。
- 因最终 injection 还拼接 `stepwise-inject.js`，同一契约继续捕获并关闭 stepwise 中三处旧品牌状态/诊断文案，避免只修 renderer 产生伪绿。

### Updater preservation

- `crates/codex-plus-core/src/update.rs` 仍保留 `DEFAULT_REPOSITORY`、`DEFAULT_LATEST_JSON_URL`、`check_for_update` 和 `validate_manifest_asset_url`。
- 更新清单继续来自品牌真相源；平台资产 URL 仍经过 HTTPS、固定 GitHub host、固定仓库路径、版本和文件名身份校验。
- 本 Step 删除的是 renderer 可访问和普通用户可见的仓库表面，没有删除或放宽后台 updater 的信任模型。

## TDD evidence review

- 初始契约先因 repository globals、renderer repository 常量和项目/反馈 DOM 失败；删除后形成基础 Green。
- About DOM 删除后仍保留 `codexPlusBuild` 诊断路径，并由 `script_loaded` / `build: codexPlusBuild` 契约覆盖。
- 审计新增旧品牌 fallback 负向契约后先 Red，再最小替换 renderer 与最终拼接脚本中的旧品牌文本。
- evidence 的 Red、Green、最终测试数和当前源码扫描结果一致。

## Independent verification

- `injection_script_prefixes_helper_url_without_sponsor_images`：最终 `1 passed`。
- `cargo test -p codex-plus-core --test cdp_bridge --locked`：`69 passed`。
- `cargo test -p codex-plus-core --test bridge_routes --locked`：`26 passed`。
- `pwsh -NoProfile -File scripts/verify-no-upstream-ads.ps1`：PASS。
- `pwsh -NoProfile -File scripts/test-verify-allowlist.ps1`：PASS，包含 allowlist 与两个图片目录的 fail-closed fixture。
- 最终源码扫描无 `Chimera Codex`、repository globals、About/Issues DOM、提出问题、GitHub Issues 或项目仓库 URL。
- 聚焦 `git diff --check`：PASS；仅有 Git 行尾转换提示，无 whitespace error。

## Deferred scope

- 生产扫描器规则扩展、全仓 stale i18n、推荐/赞助/交流群残留和第三方图标引用统一归 Step 12.3。
- GitHub 作为后台更新基础设施的固定 URL 仍允许存在于 updater/branding 发布路径，不应被普通 UI 或 renderer 重新暴露。
- 本 Step 不推送、不发布，也不表示 Task 12 聚合已经完成。

从审计 A 侧，Step 12.2 已满足关闭条件；仍须按项目流程由独立审计 B 同样通过后才能勾选该 Step。
