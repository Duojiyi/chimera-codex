# Task 12 Step 12.1 独立审计 B

## 结论

**PASS**

基于当前最终工作树，独立按 diff、边界条件、运行时数据入口、回归面与测试充分性复核后，未发现阻断 Step 12.1 关闭的问题。About 页面、项目/GitHub/Issues 入口和手动更新控件均已从普通 manager UI 移除；日志、诊断和只读更新进度已迁入维护页；最终补丁同时关闭了动态脚本主页与 updater raw message 两条静态字符串扫描之外的旁路。

本审计未读取任何 Step 12.1 审计 A 文件，也未以其他 A 审计结论作为判断依据。

## 独立复核

### 1. About 路由与兼容入口

- `apps/codex-plus-manager/src/App.tsx:629-642` 的 `Route` union 和导航列表均不再包含 `about`；页面渲染、route subtitle 与 `AboutScreen` 实现已删除。
- `goLogs()` 进入 `maintenance`；startup `showUpdate` 也进入维护页，不再恢复 About 页面。
- `loadInitialRoute()` 明确把旧 `showUpdate=1` 和 `#about` 映射为 `maintenance`，且不可能返回已删除的 `about`。最终静态契约对两种兼容入口和返回值均有直接断言。

### 2. GitHub、项目主页与手动更新表面

- 对最终 `App.tsx` 扫描 `GitHub|github.com|Issues|AboutScreen|REPOSITORY|actions.checkUpdate()|actions.performUpdate()|update-dot|下载并运行安装包` 均无命中。
- `MarketScriptCard` 只保留脚本名称、作者、描述、标签与安装/更新动作，不再读取或打开 `script.homepage`。因此脚本清单不能通过动态 homepage 值重新引入 GitHub 或任意项目主页按钮。
- 通用 `openExternalUrl` 能力仍用于既有 `API_KEY_URL`，但没有项目主页、Issues 或脚本 homepage 调用点；保留该受控业务入口不违反本 Step 边界。
- `checkUpdate` / `performUpdate` 后台调用函数仍在代码中，符合后续 Task 13 的自动更新接线需要；它们未暴露为 `Actions` 中的手动按钮，也没有普通页面 onClick 触发点。

### 3. 更新消息运行时边界

- `checkUpdate` 不再把 backend `result.message` 透传到 notice；提示只根据结构化 `status` 与 `updateAvailable` 生成“服务暂时不可用”“发现可用更新”或“当前已是最新版本”等中性文案。
- 这关闭了更新清单校验错误把 `GitHub Release`、`asset` 等实现术语带回显式 update 路由的运行时旁路。静态契约截取 `checkUpdate` 实现并禁止读取 raw message。
- 现有安装进度继续保留，但 `MaintenanceScreen` 只渲染 `TaskProgressBox`，没有“检查更新”或“下载并运行安装包”控件。Task 13 仍负责最低支持版本、普通/强制更新状态机与平台安装策略，本 Step 不宣称自动更新端到端完成。

### 4. 日志、诊断与维护页

- `navigate("maintenance")` 会刷新 overview、watcher、logs 与 diagnostics。
- `MaintenanceScreen` 实际接收并渲染 `LogsPanel`、`DiagnosticsPanel` 和 `updateInstallProgress`，支持能力没有随 About 删除而丢失。
- 对维护页的静态测试先截取函数边界，再要求日志与诊断组件存在；更新进度测试要求父级传参、维护页渲染和完成态标题，同时禁止旧手动按钮。

## i18n 与延后边界

- 新增的中性更新文案在 `i18n-en.ts` 和 `tools/i18n-keys.json` 中均有对应项；TypeScript 检查与 Vite 构建通过，没有 missing translation/key。
- 当前 `node tools/i18n-verify.mjs` 按预期以 exit 1 报告 `19` 个 plain 与 `4` 个 template stale key；没有 missing key。23 个 stale 均来自已删除的 About、GitHub、手动更新或项目主页表面，计划明确留给 Step 12.3 统一清理，因此本审计不把该预期失败误记为全绿，也不越界要求 Step 12.1 删除它们。
- 注入菜单与仓库全局变量归 Step 12.2；生产级推广/第三方图标扫描和全部死键归 Step 12.3；后台更新决策与安装流程归 Task 13。当前 diff 没有提前宣称这些后续任务完成。

## Evidence 与回归

`docs/superpowers/audits/task-12-step-12.1-evidence.md` 的最终范围、Red/Green 轨迹和回归计数与当前实现一致：动态 homepage、raw updater message 和旧深链断言均能在最终测试中找到对应契约；i18n deferred 总数为 `19 + 4 = 23`，正文与实际输出一致。

最终回归证据如下：

- `cargo test -p codex-plus-manager --test windows_subsystem manager_ui_removes_about_github_and_manual_update_controls --locked -- --exact`：本次独立复跑 `1 passed`。
- `cargo test -p codex-plus-manager --test windows_subsystem manager_update_install_keeps_visible_progress_bar --locked -- --exact`：`1 passed`。
- `cargo test -p codex-plus-manager --locked`：lib `54 passed`、`windows_subsystem 41 passed`，bin/doc tests `0 failed`。
- `npm run check`：本次独立复跑 PASS。
- `npm run vite:build`：PASS，`1608 modules transformed`。
- 目标文件 `git diff --check`：PASS；仅有工作树换行符转换提示，没有 whitespace error。

剩余测试风险是当前 UI 契约主要采用源码结构断言而非浏览器级交互测试，旧 query/hash 路由也以函数片段断言验证。鉴于最终实现为直接枚举/条件分支、类型检查和生产构建均通过，且两条动态数据旁路已有专门负向契约，该风险不构成本 Step 阻断。
