# Task 3 (T43) Audit A — Spec/行为覆盖

**Date:** 2026-07-26
**Scope:** Step 3.1–3.6（供应商优先的客户前端）
**Commits:** `4681735` feat(task-3): Provider-first UI, `5d4c7d2` feat(task-3): implement all 5 screens from the Pencil design file, `181beef` feat: wire the Tauri command layer
**Auditor:** Independent A（只看 Spec/行为覆盖，不参考 Audit B 的结论）

方法：对照 Plan Step 3.1–3.6 与 Spec 6.1/6.2/7.2/15.1 逐条核对可观察行为，引用文件:行号证据，标注 covered / partial / missing。

## Step 3.1 — ChimeraHub Key-first

要求（Plan/Spec 7.2）：首次运行只显示 ChimeraHub Key 输入；保存前不写 live config；验证成功后才激活。

| 子要求 | 证据 | 状态 |
|---|---|---|
| 拒绝空/空白 Key，给出可操作 recovery | `providerForm.ts:26-34` `validateChimeraHubKey`；测试 `providerForm.test.ts:14-41`（5 个） | covered |
| 首次运行只显示 ChimeraHub | 未找到任何"首次运行检测"逻辑。`App.tsx` 直接渲染 5 个 tab，`ProvidersFeature` 的 Add 表单默认显示 `<select>` 同时列出 `chimera_hub` 和 `custom` 两个选项（`providers/index.tsx:206-213`），从未根据"是否已有供应商"限制只出现 ChimeraHub | **missing** |
| 保存前不写 live config | `handleAdd`（`providers/index.tsx:51-72`）只做本地 `setState`，不调用任何 `invoke()`。这条件成立，但成立的原因是"整个保存路径都没有接触 config"，不是"先验证再写"这一设计意图的体现 | covered（弱） |
| **验证成功后才激活** | `handleAdd` 只跑格式校验（`validateChimeraHubKey`/`validateCustomProviderInput`，纯字符串检查），随后立即 `setState(s => addProvider(s, entry))`——`addProvider`（`providerState.ts:39-50`）在列表为空时把新条目直接设为 `activeId`。**全程没有调用 `test_provider` 或任何网络探测**，Key/URL 是否真的有效从未被检查就已激活 | **missing** |

Step 3.1 结论：**partial**。格式验证层被测试良好覆盖，但 Spec 明确写的"验证成功后才激活"这条核心行为没有被实现——这不是遗漏了测试，是功能本身没有做探测这一步。

## Step 3.2 — 两字段自定义供应商

要求（Spec 7.2 步骤 1-7）：默认只显示 URL + Key；探测后展示最终解析 URL 和模型摘要；一次确认后保存；探测不确定时展开"高级设置"而不是保存猜测结果。

| 子要求 | 证据 | 状态 |
|---|---|---|
| URL/Key 校验：HTTPS 强制、userinfo 禁止、origin-only 警告 | `providerForm.ts:60-127`；测试覆盖 7 个场景（`providerForm.test.ts:45-93`） | covered |
| 探测摘要（"展示最终解析的 URL 和模型摘要"） | 未实现。`handleAdd` 不调用 `test_provider`，因此没有探测结果可展示；`ProviderDetail` 里的 "Test connection" 按钮（`providers/index.tsx:289-299`）是加完供应商*之后*才能点的独立操作，不是添加流程的一部分 | **missing** |
| 探测失败展开"高级设置" | 代码中不存在"高级设置"面板。表单只有 URL/Key 两个输入 + 一个总是可见的 kind 下拉，没有"展开/收起"状态 | **missing** |
| 一次确认后保存 | 表单提交即保存，这点是满足的，但因为前面没有探测环节，"确认"确认的只是格式，不是连通性 | covered（弱） |

Step 3.2 结论：**partial**，与 3.1 同样的根因——探测环节整体缺失。

## Step 3.3 — 多供应商列表、切换、Codex 重启、Official restore

要求（Plan）：多供应商列表、当前状态、切换确认、"切换并重启"、Official restore。

| 子要求 | 证据 | 状态 |
|---|---|---|
| 纯状态机：add/switch/delete/setHealth/selectActive | `providerState.ts` 全部函数；测试 13 个全绿（`providerState.test.ts`，已重跑确认 25/25 TS 测试通过，含 providerForm 12 个） | covered |
| 列表 UI：role=tab、当前态高亮、健康点 | `providers/index.tsx:96-158`，`role="tab"` + `aria-selected` + `dotColor()` | covered |
| **切换确认对话框** | 不存在。`handleSwitch`（`providers/index.tsx:74-79`）点击后直接调用，没有任何确认步骤（对比：删除操作确实用了 `window.confirm`，`providers/index.tsx:319`，但切换没有） | **missing** |
| **实际切换生效（端到端）** | `commands.rs:152-158`：`switch_provider` 命令**无条件返回 `Err`**（"Provider switching is not enabled in this build. Task 6 connects the config transaction."）。前端 `handleSwitch` 捕获这个错误后只 `console.error`，**不更新 UI 状态、不提示用户**——点击供应商 tab 在界面上什么都不会发生 | **missing** |
| "切换并重启 Codex" | 不存在对应 UI 或命令；`launch_codex` 命令本身也是 stub（`commands.rs:164-176`，已安装时返回"launching is not enabled"） | **missing** |
| Official restore（`switchProvider(state, null)`） | 纯函数层面正确（`switchProvider` 测试覆盖 null 分支），但走的是与上面相同的失败 `invoke` 路径，同样不生效 | **partial**（仅纯函数层） |

Step 3.3 结论：**partial**。孤立的状态机单元测试全部通过，但把这些单元接到真实 UI 交互后，切换功能在当前 build 里完全不工作，且失败是静默的（用户点击后没有任何反馈）。这比"功能未实现"更值得关注，因为它看起来像是工作的（按钮可点、有 loading 状态），实际上什么也没发生。

## Step 3.4 — 系统托盘快速切换

要求：托盘快速切换、当前供应商、启动 Codex；托盘与主窗复用同一 command/state。

证据：`src-tauri/Cargo.toml:21` 声明了 `tray-icon` feature，但 `grep -r "TrayIcon\|tray::"` 在 `src-tauri/src/` 下无任何匹配；`lib.rs` 的 `invoke_handler!` 列表（9 个命令）里没有任何托盘相关命令；无测试。

Step 3.4 结论：**missing**。这是 Cargo feature 声明了但从未被使用的典型信号——依赖已引入，实现完全没有。

## Step 3.5 — Playwright + axe-core 可访问性门禁

要求：核心流程零 serious/critical violation；自动覆盖键盘陷阱、焦点可见、accessible name、对比度、200% 缩放、1280x720、长 URL/名称、离线、中文错误文案。

证据：
- `package.json:12`（HEAD 提交状态）：`"test:a11y": "node scripts/test-a11y.mjs"`。
- `scripts/test-a11y.mjs` **在 git HEAD 中不存在**（`git show HEAD:scripts/test-a11y.mjs` → `fatal: path does not exist`）。
- 实际运行 `npm run test:a11y`：`Error: Cannot find module 'D:\Desktop\codex plus plus\scripts\test-a11y.mjs'`。
- 唯一带 axe-core 依赖的目录 `scripts/design-verify/` 是**工作区未提交的改动**（`git status` 显示为 `??` untracked），不属于 `4681735`/`5d4c7d2` 任何一个 Task 3 commit,也未被任何 CI workflow 引用。

标注的手工可访问性属性（`role="tab"`、`aria-label`、`aria-selected`、Appearance 皮肤列表的 `onKeyDown` Enter/Space 处理）体现了良好的编码习惯，但这些都是**未经自动化验证**的断言——没有 axe-core 扫描报告、没有 200% 缩放测试、没有对比度测试、没有长文本/离线场景测试。

Step 3.5 结论：**missing**。且这不只是"缺一个功能"，是**当前 CI 配置的 V8 门禁本身会失败**（`node scripts/verify-v2.mjs --only=V8` 实测：`V8 FAIL`，因为 `npm test && npm run test:a11y` 链条在 `test:a11y` 处以 `MODULE_NOT_FOUND` 中断）。

## Step 3.6 — Windows NVDA smoke + Task 3 聚合审计

要求：Windows NVDA 完成首次运行/添加切换/更新回滚修复 smoke；形成读屏检查清单。

证据：仓库内无 NVDA checklist 文档，无相关 fixture 或截图证据。此前不存在 Task 3 聚合审计文件（本文档是首次补写）。

Step 3.6 结论：**missing**（实机测试属于后续里程碑，此阶段未开始符合预期，但因此本 Task 尚不满足"完成聚合审计后勾选"的前置条件——本文档正是在补这个缺口，但结论仍需反映 3.4/3.5 未完成的事实）。

## 与提供的摘要的差异

调用方摘要称 Step 3.1–3.3 的行为已经"实现"（"Provider-first UI — form validation, state machine, React components"），这在**纯函数/组件渲染**层面是准确的（25 个 TS 测试确实全部通过），但摘要没有提到：

1. Add-provider 流程从未调用探测（`test_provider`），"验证成功后才激活"这条 Spec 硬性要求未满足。
2. `switch_provider` 命令在当前 build 里恒定失败，且失败对用户不可见——这比"尚未实现"更容易被误认为已经工作。
3. `package.json` 里的 `test:a11y` 脚本指向一个不存在的文件，V8 门禁在当前 HEAD 状态下是**红的**，不是"尚未添加"，而是"配置已提交但已损坏"。

## 结论

**FAIL（不满足勾选条件）。**

Step 3.1/3.2/3.3 的核心可观察行为（验证后激活、探测摘要、切换确认、切换生效）均未达成，Step 3.4（托盘）、3.5（a11y 自动化门禁）完全空白，且 3.5 对应的 CI 门禁在当前提交状态下会主动报错而非仅仅"跳过"。根据 Plan 第 4 条规则，A/B 两份审计都 PASS 才能勾选 T43；本审计给出 FAIL，Task 3 不应勾选。已完成且值得保留的部分：`providerForm`/`providerState` 两个纯模块的 25 个单元测试、`design/tokens.ts` 与 V16 门禁（已用自建的注入-违规/还原两次验证证实自测机制有效）、5 个屏幕的视觉实现对 .pen 文件保真。
