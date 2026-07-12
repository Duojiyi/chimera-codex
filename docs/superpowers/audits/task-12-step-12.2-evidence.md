# Task 12 Step 12.2 Evidence

## Scope

- Codex 注入菜单不显示 About、GitHub、Issues 或“提出问题”入口。
- 注入脚本不接收、不定义 repository 名称或 repository URL 全局变量。
- 后台 updater 的独立可信更新源保持不变；本 Step 只删除普通用户可见与 renderer 可访问的仓库表面。

## Red

- 将最终 `assets::injection_script` 契约改为禁止 `__CODEX_PLUS_REPOSITORY__`、`__CODEX_PLUS_REPOSITORY_URL__`、`codexPlusRepository`、本项目 GitHub URL、About CSS、Issues 按钮/处理器和“提出问题”文案。
- 首次 Red 被旧品牌 fixture `Chimera Codex` 提前截断；按 Task 11 已完成的品牌真相源校正为 `Chimera++` 后重新运行。
- `cargo test -p codex-plus-core --test cdp_bridge injection_script_prefixes_helper_url_without_sponsor_images --locked -- --exact`：`0 passed / 1 failed`，准确命中 `window.__CODEX_PLUS_REPOSITORY__`。
- 审计 A 发现 renderer 的 display-name fallback 仍为旧 `Chimera Codex`；加入禁止契约后精确测试再次 Red 为 `0 passed / 1 failed`，准确命中旧 fallback。

## Green

- `crates/codex-plus-core/src/assets.rs` 不再构造 repository URL，也不再向 renderer 注入两个 repository 全局变量。
- `assets/inject/renderer-inject.js` 删除 repository 常量、About/Build/GitHub DOM、Issues DOM、click handler 与专用 CSS。
- renderer display-name fallback 改为当前品牌真相 `Chimera++`；兼容技术变量名保持不变。
- 最终注入还会拼接 `stepwise-inject.js`；契约继续捕获其中三处旧 `Chimera Codex` 状态/诊断文案，统一改为 `Chimera++`。
- `codexPlusBuild` 和 `__CODEX_PLUS_BUILD__` 保留用于 `script_loaded` 诊断事件；旧 DOM fixture 改为验证 `build: codexPlusBuild`，诊断覆盖未削弱。
- 后台 `crates/codex-plus-core/src/update.rs` 的 `DEFAULT_LATEST_JSON_URL`、可信仓库校验与 Release asset 身份校验未修改。

## Regression

- 精确 Green：`injection_script_prefixes_helper_url_without_sponsor_images`，`1 passed`。
- `cargo test -p codex-plus-core --test cdp_bridge --locked`：`69 passed`。
- `cargo test -p codex-plus-core --test bridge_routes --locked`：`26 passed`。
- 精确源码扫描 `Chimera Codex|codexPlusRepository|__CODEX_PLUS_REPOSITORY|codex-plus-about|codex-plus-issue|提出问题|GitHub Issues|github.com/Duojiyi/chimera-codex`：renderer、stepwise 与 `assets.rs` 无命中。
- `pwsh -NoProfile -File scripts/verify-no-upstream-ads.ps1`：PASS。
- `pwsh -NoProfile -File scripts/test-verify-allowlist.ps1`：PASS，含 allowlist 与两个图片目录 fail-closed fixtures。
- `cargo fmt --all -- --check`：PASS。
- `git diff --check --` 四个 Step 文件：PASS；仅存在 Git 行尾转换提示，无 whitespace error。

## Boundary

- 注入菜单仍保留打开管理工具、DevTools、配置开关和用户脚本等产品功能；只删除客户不需要的项目/反馈信息。
- 生产扫描器的规则扩展与全仓死键/第三方图标引用清理归 Step 12.3。
- 本 Step 不修改 updater 信任模型、不推送、不发布。
