# Task 12 Step 12.1 Evidence

## Scope

- 普通用户界面删除 `about` route、导航项、About screen、项目主页、Issues、GitHub 文案与手动检查/下载安装按钮。
- 旧 `showUpdate=1` 和 `#about` 深链兼容迁移到 `maintenance`，不再恢复 About 页面。
- 日志、诊断与既有更新安装进度迁入“安装维护”；更新进度保留可见，但不提供手动触发控件。
- 后台 updater 命令与可信 Release 解析未删除，后续强制更新状态机仍归 Task 13。

## Red

- 新增 `manager_ui_removes_about_github_and_manual_update_controls` 静态行为契约后，旧实现因 `about` route/nav/screen、GitHub 项目链接、手动更新 action 与 update-dot 仍存在而失败。
- 删除 About 后，全量 Manager 回归暴露旧 `manager_update_install_keeps_visible_progress_bar` 仍要求“下载并运行安装包”按钮；将契约改为禁止手动按钮、要求维护页接收并渲染 `updateInstallProgress`。
- `cargo test -p codex-plus-manager --test windows_subsystem manager_update_install_keeps_visible_progress_bar --locked -- --exact`：Red 为 `0 passed / 1 failed`，失败点为维护页尚无 `安装包更新进度`。
- 审计 A 发现脚本市场仍通过动态 `script.homepage` 暴露任意项目主页，可绕过 GitHub 固定字符串扫描；将 `actions.openExternalUrl(script.homepage)` 加入禁止契约后，精确测试 Red 为 `0 passed / 1 failed`。
- 审计 B 发现 `checkUpdate(false)` 仍把 backend `result.message` 原样显示，更新清单校验错误可能在运行时带回 `GitHub Release` 术语；新增契约禁止 `checkUpdate` 读取 raw message 后，精确测试 Red 为 `0 passed / 1 failed`。
- 审计 B 同时指出旧 `showUpdate=1` / `#about` 迁移没有专门回归断言；静态契约现明确要求两种入口均 `return "maintenance"` 且不得返回 `about`。早期 Green 前的 TypeScript Red 已因残留 `return "about"` 报 route 类型错误。

## Green

- `Route`、导航、route subtitle 与页面渲染中移除 `about`；删除 `AboutScreen`。
- `goLogs()`、startup update 路由和旧 `#about` 深链统一进入 `maintenance`。
- `MaintenanceScreen` 接收日志、诊断和 `updateInstallProgress`，渲染 `LogsPanel`、`DiagnosticsPanel` 与只读进度条。
- 用户可见 GitHub Release 文案改为“版本更新”或“远程清单”；删除项目主页、Issues、检查更新和下载安装按钮。
- 删除 `MarketScriptCard` 的动态 homepage 外链按钮，脚本安装、更新、作者与描述信息保持不变。
- 更新检查提示只根据结构化 `status` / `updateAvailable` 生成中性文案；失败时固定显示“版本更新服务暂时不可用”，不再把 backend 自由文本透传到普通 UI。
- 为四条新增中性文案补充英文翻译与 key manifest；About/GitHub 等 23 个死 i18n key 按计划留给 Step 12.3 的统一死键清理。

## Targeted Regression

- `cargo test -p codex-plus-manager --test windows_subsystem manager_ui_removes_about_github_and_manual_update_controls --locked -- --exact`：`1 passed`。
- `cargo test -p codex-plus-manager --test windows_subsystem manager_update_install_keeps_visible_progress_bar --locked -- --exact`：`1 passed`。
- 精确扫描 `AboutScreen|REPOSITORY|actions.checkUpdate()|actions.performUpdate()|update-dot|about route returns|下载并运行安装包`：无命中，`rg` exit `1`。
- `npm run check`：PASS。
- `npm run vite:build`：PASS，`1608 modules transformed`。
- `cargo test -p codex-plus-manager --locked`：lib `54 passed`，`windows_subsystem 41 passed`，bin/doc tests `0 failed`。
- `git diff --check --` 四个 Step 文件：PASS；仅提示现有 Git 行尾规范将在后续 checkout 时转换，不存在 whitespace error。

## Deferred Boundary

- `node tools/i18n-verify.mjs` 已确认本 Step 新增文案没有 missing key；当前仅报告 `19` 个 plain 与 `4` 个 template stale key，均来自本轮删除的 About/手动更新/项目主页表面，按 Plan 明确归 Step 12.3 清理。
- 注入菜单与仓库全局变量归 Step 12.2；生产级推广/第三方图标扫描器及全部死键归 Step 12.3。
- 本 Step 不推送、不发布，也不宣称 Task 12 已完成。
