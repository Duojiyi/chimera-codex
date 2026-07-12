# Task 12 Step 12.2 独立审计 B

## 结论

**PASS**

基于当前最终工作树，独立按 diff、renderer 数据边界、回归面与测试充分性复核后，未发现阻断 Step 12.2 关闭的问题。最终注入脚本不再接收或定义仓库名称/URL，全套 About、GitHub、Issues 与“提出问题”DOM、样式和处理器均已删除；后台 updater 的固定更新源和资产身份校验保持独立且有效。

本审计未读取任何 Step 12.2 审计 A 文件，也未引用其他 A 审计结论。

## 独立复核

### 1. 注入参数与 renderer 全局变量

- `crates/codex-plus-core/src/assets.rs:24-46` 组装的最终 injection script 只注入 helper URL、版本、诊断 build id、显示名、图片覆盖、插件市场和功能配置，不再构造 repository URL，也没有 `__CODEX_PLUS_REPOSITORY__` / `__CODEX_PLUS_REPOSITORY_URL__`。
- 对 `renderer-inject.js`、`stepwise-inject.js` 与 `assets.rs` 精确扫描 `codexPlusRepository|__CODEX_PLUS_REPOSITORY|github.com/Duojiyi/chimera-codex` 无命中；仓库信息不能再通过注入前缀进入 Codex renderer。
- `__CODEX_PLUS_BUILD__` 与 `codexPlusBuild` 仅用于 `script_loaded` 诊断事件。最终测试要求 `build: codexPlusBuild` 仍存在，删除 About DOM 没有同时削弱诊断覆盖。
- renderer 在缺少注入前缀时的显示名 fallback 已固定为 `Chimera++`；最终拼接的 `stepwise-inject.js` 三处状态/诊断文案也已统一为 `Chimera++`。最终产物契约禁止旧 `Chimera Codex`，避免任一拼接资源重新显示旧品牌。

### 2. 菜单 DOM、样式与事件处理

- `renderer-inject.js` 已删除 About/Build/GitHub 区块、Discord/Telegram 区块、Issues/“提出问题”按钮，以及对应 `window.open` 分支。
- `.codex-plus-about`、`.codex-plus-issue-button`、`data-codex-plus-issue` 等专用选择器和样式均不存在；不存在只隐藏 DOM、但保留可触发 handler 的残余路径。
- 注入菜单仍保留打开管理工具、DevTools、设置开关、用户脚本和既有增强能力，符合“只移除项目/反馈表面，不破坏产品功能”的边界。
- 最终源码唯一包含 `issue` 的相关命中是说明上游模块兼容性的 `issue #1324` 注释，不是 DOM、URL、按钮或可见文案，不构成客户入口。

### 3. Updater 信任边界

- `DEFAULT_REPOSITORY` 与 `DEFAULT_LATEST_JSON_URL` 仍直接引用品牌真相源；当前固定清单 URL 为本项目公开 Release 的 `latest.json`，没有回退到上游仓库。
- manifest asset 校验仍要求 HTTPS、无 userinfo/query/fragment、host 精确为 `github.com`，路径精确匹配品牌 owner/repository、版本 tag 与资产名；平台资产名、版本、大小和 SHA-256 也继续校验。
- 本次独立运行 updater 全量 `36 passed`，其中覆盖固定 Chimera 更新源、外部 host/userinfo/latest 路径拒绝、跨版本或额外文件名 token 拒绝、下载大小/哈希/超时与发布身份。Step 12.2 没有以删除用户可见 GitHub 表面为由削弱后台更新安全。
- 仓库与 Release URL 仍可出现在品牌真相源、许可证/源码归属、发行工作流和 updater 信任代码中；本 Step 的精确边界是不得进入普通注入 UI，而不是删除发行基础设施或开源归属。

### 4. 扫描器与 allowlist

- 最终注入产物契约直接对 `assets::injection_script` 做负向检查，覆盖 Rust 前缀与两个 JS 资源拼接后的真实结果，而不是只扫描单个源文件。
- `verify-no-upstream-ads.ps1` 本次实际通过。allowlist matcher 绑定精确相对路径、scanner pattern、行号与完整 trimmed line，并拒绝重复使用和未使用条目；不存在通配路径、后缀匹配或部分行放行。
- `test-verify-allowlist.ps1` 本次实际通过，并验证错误路径、错误行号、错误上下文、重复消费以及图片目录 fail-closed fixtures。
- 生产扫描规则扩展、全部 GitHub UI/推广残留和第三方图标引用的全仓闭环明确归 Step 12.3；本审计不把该后续 Gate 冒充为 Step 12.2 已完成。

## Evidence 与回归

`docs/superpowers/audits/task-12-step-12.2-evidence.md` 的最终 Red/Green、fallback 修复、测试计数和边界说明与当前代码一致。独立复核结果：

- `cargo test -p codex-plus-core --test cdp_bridge --test bridge_routes --locked`：`cdp_bridge 69 passed`、`bridge_routes 26 passed`，`0 failed`。
- `cargo test -p codex-plus-core --test updater --locked`：`36 passed`，`0 failed`。
- `pwsh -NoProfile -File scripts/verify-no-upstream-ads.ps1`：PASS。
- `pwsh -NoProfile -File scripts/test-verify-allowlist.ps1`：PASS。
- `cargo fmt --all -- --check`：PASS。
- 目标文件 `git diff --check`：PASS；仅有工作树换行符转换提示，没有 whitespace error。
- 精确扫描 renderer、stepwise 与 `assets.rs` 中的 repository globals、About/Issues selectors、提出问题、项目 GitHub URL 和旧品牌 fallback：无命中。

剩余风险是本 Step 主要通过最终脚本文本契约和 bridge 回归验证菜单移除，没有在真实 Codex WebView 中执行截图/点击级验收。鉴于被删除入口的 DOM、CSS、handler 和输入 globals 均已从最终拼接产物消失，且完整 bridge 路由与注入回归通过，该风险不构成本 Step 阻断。
