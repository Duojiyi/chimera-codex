# Task 11 Step 11.4 独立审计 B

## 结论

**PASS**

基于当前待验收工作树，独立按 diff、任务边界、失败路径与回归面复核后，未发现阻断 Step 11.4 关闭的问题。此前暴露的 active relay 误判、manager 已运行时路由丢失、installer 非事务清理和普通更新被提前提升为强制更新等风险，当前实现均已修复或被明确收束到后续任务边界。

本审计未读取、未引用 `task-11-step-11.4-a.md` 或其他 A 审计结论；判断仅来自设计/计划/TODO、当前实现与测试、Step 11.4 evidence 及本次独立回归。

## 边界核对

- `docs/superpowers/plans/2026-07-11-chimera-plus-product-refresh.md:53` 将 Step 11.4 定义为纯 `LaunchRoute` 优先级、单桌面入口、开始菜单管理入口及 launcher/installer 回归。
- 同计划 `:65-75` 将可信 `minimum_supported_version`、普通/强制更新状态机和 launcher/manager 阻断 UI 接线明确留给 Task 13，TODO 的 T32 也仍未完成。
- 因此 `apps/codex-plus-launcher/src/main.rs:245` 当前传入 `mandatory_update: false` 是可见且受控的后续接线点，不代表强制更新生产路径已经完成；纯路由仍保留 mandatory update 最高优先级。Step 11.4 不应越界伪造尚不存在的更新状态，也不因该后续接线点判 FAIL。

## 独立复核

### 1. LaunchRoute 与生产接线

- `crates/codex-plus-core/src/launcher.rs:55-85` 的纯判定顺序为：mandatory update、损坏 settings 恢复、活动中转或合规官方登录直接启动 Codex、已有 Key 但未应用时配置中转、最后 Key-first。分支互斥且没有低优先级状态越过高优先级状态。
- `apps/codex-plus-launcher/src/main.rs:208-258` 使用 `SettingsStore::load_strict()`；settings 损坏时 fail closed 到 recovery。路由在 launcher single-instance guard 与 Codex 启动前完成，非 Codex 分支分别映射到 `--show-update`、`--recover-settings`、`--configure-relay`、`--chimera-key-first`。
- 当前 production 不再把普通 `update_available` 直接映射成 mandatory。launcher 回归同时固定普通更新不得提前占用 mandatory 路由，符合 Step 11.4 与 Task 13 的边界。

### 2. Active relay、Official 与 Key 判定

- active relay 仅在 `relay_profiles_enabled` 时参与直接启动。aggregate profile 必须匹配本地 proxy identity；普通 profile 必须具有支持的 Key 来源，并与 live provider 的 ID、Base URL、Key 和 `requires_openai_auth = true` 严格一致。
- `crates/codex-plus-core/src/relay_config.rs:230-328` 对 expected/live config 均使用 TOML 结构化解析；任意非法 TOML、缺失 provider、错误 URL/Key 或 `requires_openai_auth != true` 均 fail closed。该实现不把 Key 写入日志或错误信息。
- `crates/codex-plus-core/src/settings.rs:682-697` 只接受 profile `api_key`、活动 provider 的 `experimental_bearer_token`、`authContents.OPENAI_API_KEY`；空白 API Key 和 ChatGPT account token 不会被误认成中转 Key。
- official login 只有在中转关闭，或活动 profile 为非混合 Official 时才允许直接启动；PureApi、混合 Official 和无认证状态不能借官方 token 绕过中转配置。

### 3. 单桌面入口与 installer 事务

- `scripts/installer/windows/CodexPlusPlus.nsi:333-347` 在任何 mutation 前备份 current、legacy、乱码和 compat 快捷方式；`:365-379` 桌面只创建 `Chimera++.lnk`，开始菜单保留主入口、管理工具和卸载入口；`:405-414` 清理旧入口；`:460-475` 使用相同 slot 对称回滚。
- legacy ARP 整键删除位于全部可回滚步骤之后的提交阶段；URL protocol 卸载只删除 owned values，再以 `/ifempty` 从深到浅清理空键，未知值/类型/子键不会被整键误删。卸载器本体在元数据事务成功前保留。
- NSIS install/uninstall 与 Rust runtime 共用 `Local\ChimeraPlusPlus.Setup.Transaction` named mutex；NSIS 使用 `CreateMutexW(..., initialOwner=1, ...)`，Rust 在 snapshot 前获取 guard，避免两条安装路径并发修改同一组元数据。
- `crates/codex-plus-core/src/install/windows.rs:144-185` 对 install/uninstall 均先捕获快捷方式原始 bytes 与 registry raw name/type/data，apply 失败统一恢复 snapshot；rollback 失败会与原始 apply 错误一起上报。运行时 install 同样只创建主桌面入口，并清理受管 manager 入口。

### 4. Manager 已运行实例的路由交付

- `apps/codex-plus-manager/src-tauri/src/lib.rs` 使用同目录 `.tmp -> .json` 原子发布和 `.json -> .processing` 原子 claim；请求按 `created_at_ms`、再按 id 稳定排序，单条 entry/metadata/read/rename 错误隔离，不丢弃已 claim 的有效请求。
- pending 状态为 `VecDeque`；`next_pending_or_mark_ready` 在队列真正清空前保持 not-ready，避免新 route 越过旧 pending。React cleanup 先调用 `manager_frontend_not_ready`，settle 后再卸载 listener。
- ACK 顺序为 claim、dispatch/queue、清理 processing、写 ACK。发送方 2 秒超时不会删除仍归其所有的 `.json`；`.tmp/.processing/.ack` 仅在超过 30 秒后清理，待消费 `.json` 保留在可信窗口内。
- guard 只有 `WouldBlock`/`AddrInUse` 归类为已有实例；权限等 fatal error 直接失败，不再通过随机端口 fail-open 启动第二个 manager。该实现关闭了“唯一桌面入口点击后因 manager 已运行而丢失目标路由”的原阻断面。

## Evidence 真实性与回归

`docs/superpowers/audits/task-11-step-11.4-evidence.md` 中记录的 Red/Green 轨迹可由当前测试名、实现分支和计数交叉验证。evidence 的 launcher 历史运行耗时为 `563.47s`，本次独立运行耗时为 `450.17s`；两次均为 `66 passed`，耗时差异属于不同运行，不构成证据冲突。

本次在当前工作树实际执行并通过：

- `cargo test -p codex-plus-core --test branding --test installers --test relay_config --locked`：`2 + 23 + 97 passed`。
- `cargo test -p codex-plus-core --test launcher --locked`：`66 passed`，`0 failed`。
- `cargo test -p codex-plus-launcher --locked`：`8 passed`。
- `cargo test -p codex-plus-manager --locked`：lib `54 passed`、`windows_subsystem 40 passed`，bin/doc `0 failed`。
- `cargo test -p codex-plus-core --lib install::windows::tests --locked`：`4 passed`，含 mutex 并发和 Windows registry/shortcut round-trip。
- `npm run check`、`npm run vite:build`：PASS，Vite 构建转换 `1608 modules`。
- branding、去推广、allowlist、license 与 license self-test 脚本：PASS。
- `cargo metadata --locked --no-deps`：四个 workspace package 均为 `AGPL-3.0-only`。
- `cargo fmt --all -- --check`、`git diff --check`：PASS；后者只有工作树换行符转换提示，无 whitespace error。

## 未覆盖风险

以下风险真实存在，但不阻断 Step 11.4：

1. 本机缺少 `makensis`，本审计只能验证 NSIS 静态事务契约，不能把它表述为真实 NSIS 编译、覆盖安装或 fault-injection 已通过；这些属于 Release CI 和计划中的 Windows 实机 Gate。
2. manager IPC 已有文件协议、队列、超时、异常条目和 orphan cleanup 测试，但未执行真实双进程 Tauri/WebView 崩溃注入。claim 后进程立即崩溃采用 at-most-once 与过期清理的明确取舍。
3. Rust mutex 的 `WAIT_FAILED` 路径在关闭 handle 后读取 Win32 last-error，可能降低极端失败时的诊断精度；路径本身仍 fail closed，不会绕过互斥继续安装。
4. `minimum_supported_version` 和 mandatory update 的生产计算仍由 Task 13 完成。Step 11.4 只证明纯路由优先级与当前非误判接线，不宣称强制更新端到端完成。
5. 本次未运行整个 `cargo test --workspace --locked`；已覆盖 Step 11.4 直接涉及的 core launcher、relay、installer、launcher app、manager backend/frontend 与脚本回归。

以上未覆盖项均已明确归属后续 Gate、故障注入或 Task 13，当前没有证据表明 Step 11.4 的可观察行为存在未关闭阻断。
