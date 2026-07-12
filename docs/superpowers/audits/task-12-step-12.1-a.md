# Task 12 Step 12.1 Audit A

## Independent requirements and behavior audit

结论：**PASS**。

本轮只按产品规格、Plan Step 12.1、TDD evidence、当前实现与可观察行为独立检查；未读取或引用审计 B，也未修改实现、Plan、TODO 或其他文件。审计过程中发现的动态脚本主页外链和更新失败自由文本泄露均已按 TDD 关闭，最终快照未发现新的阻断项。

## Requirements review

### About and navigation removal

- `Route` 类型、左侧导航、标题/副标题和页面渲染均不再包含 `about`；`AboutScreen` 已删除。
- App 源码不再包含项目仓库、GitHub、Issues、项目地址或 About 的可点击入口和用户可见文案。
- 旧 `showUpdate=1` 与 `#about` 兼容入口都返回 `maintenance`，不会恢复已删除的 About 页面。
- `chimera-start-route` 的 update/maintenance 路由进入安装维护页，受限 route 没有引入外部页面。

### External links

- 原实现的 About 固定仓库链接已删除。
- 审计发现 `MarketScriptCard` 仍可通过任意 `script.homepage` 显示动态“主页”按钮，可能绕过 GitHub 字面扫描；补充 Red 后该按钮已删除。
- 当前 `App.tsx` 唯一的 `openExternalUrl(...)` UI 调用是品牌真相源提供的 ChimeraHub `API_KEY_URL`，未发现其他动态项目主页或 GitHub 外链入口。
- 脚本安装、更新、作者、标签和描述仍保留，删除主页按钮没有破坏脚本市场核心操作。

### Update surface

- 手动“检查更新”“下载并运行安装包”、update-dot 和对应 action 调用均已从普通 UI 删除。
- 后台 `check_update` / `perform_update` 命令与可信更新解析仍保留，为 Task 13 自动更新状态机提供基础，不属于本 Step 的删除范围。
- 安装维护页只显示 `updateInstallProgress` 和上次更新结果，没有用户手动触发安装包下载的控件。
- 审计进一步确认 `checkUpdate(false)` 原先会把 backend `result.message` 原样显示，底层校验错误可能携带 GitHub Release/asset 术语。最终实现只按结构化 `status` / `updateAvailable` 生成“服务暂时不可用 / 发现可用更新 / 当前已是最新版本”等中性文案，不再透传自由文本。

### Logs and diagnostics migration

- `goLogs()` 进入 `maintenance`；进入维护页会刷新 overview、watcher、日志和诊断。
- `MaintenanceScreen` 明确接收并渲染 `LogsPanel`、`DiagnosticsPanel` 与只读 `TaskProgressBox`。
- 日志刷新/复制、诊断重新生成/复制功能仍可达；删除 About 没有移除故障排查能力。

### i18n boundary

- 本 Step 新增的中性更新和维护文案已进入英文翻译与 key manifest，TypeScript 和 Vite 构建通过。
- 当前 i18n verifier 只报告 `19` 个 plain 与 `4` 个 template stale key，来源是已删除的 About、手动更新和项目主页表面。
- Plan Step 12.3 明确负责统一清理全部死键，因此这些未引用条目不阻断 Step 12.1；在 Step 12.3 完成前不得宣称 Task 12 聚合完成。

## TDD evidence review

- 初始静态契约先因 About route/nav/screen、仓库链接、手动更新 action 与 update-dot 失败，再完成基础 Green。
- 维护进度契约先因维护页缺少“安装包更新进度”失败，迁移 `updateInstallProgress` 后 Green。
- 动态 `script.homepage` 被加入禁止契约后精确 Red 为 `0/1`；删除主页按钮后 Green 为 `1/1`。
- 更新检查 raw message 被加入禁止契约后精确 Red 为 `0/1`；改用结构化状态和中性文案后 Green 为 `1/1`。
- 旧 `showUpdate=1` / `#about` 迁移契约要求两者返回 `maintenance` 且禁止返回 `about`，与当前实现一致。

## Independent verification

- `manager_ui_removes_about_github_and_manual_update_controls`：`1 passed`。
- `manager_update_install_keeps_visible_progress_bar`：`1 passed`。
- `npm run check`：PASS。
- `npm run vite:build`：PASS，`1608 modules transformed`。
- Evidence 记录的完整 Manager 回归：lib `54 passed`、`windows_subsystem 41 passed`、bin/doc tests `0 failed`。
- 最终 App 扫描无 `AboutScreen`、About route、GitHub/Issues/项目地址、手动更新 action、update-dot 或 `script.homepage` UI 调用；仅保留 ChimeraHub API Key 外链。
- Step 文件 `git diff --check`：PASS；行尾转换提示不属于 whitespace error。

## Deferred scope

- 注入菜单的 About/提出问题/仓库全局变量属于 Step 12.2。
- stale i18n、生产级推荐/赞助/交流群/GitHub 扫描与第三方图标引用属于 Step 12.3。
- `minimum_supported_version`、普通/强制更新决策和平台自动安装属于 Task 13。

从审计 A 侧，Step 12.1 已满足关闭条件；仍须按项目流程由独立审计 B 同样通过后才能勾选该 Step。
