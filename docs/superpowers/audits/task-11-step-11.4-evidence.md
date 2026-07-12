# Task 11 Step 11.4 Evidence

## Scope

- Windows 桌面只保留 `Chimera++.lnk`；开始菜单保留 `Chimera++ 管理工具.lnk`。
- 覆盖升级删除旧桌面管理入口；安装事务失败时恢复安装前快捷方式状态。
- 单一桌面入口先判定更新、settings 完整性、活动中转、官方登录、未应用 Key 与全新 Key-first 状态，再决定启动 Codex 或管理器目标页。

## Initial Red And Green

- 中断前先增加纯 `LaunchRoute` 矩阵和 NSIS 单桌面契约；在 enum/判定/单入口实现前，新增契约不能通过。
- Green 增加 `LaunchRoute`、`LaunchRouteInput`、`select_launch_route`，固定优先级为：更新、settings 恢复、已就绪 Codex、已有 Key 待应用、全新 Key-first。
- Green 将 launcher 的路由判定放在 single-instance guard 和 Codex 启动之前；四个非 Codex 路由分别传递 `--show-update`、`--recover-settings`、`--configure-relay`、`--chimera-key-first`。
- manager 将参数转换为受限 `startRoute=maintenance|relay`，前端只接受既有 `maintenance` / `relay` 页面，不引入 GitHub 或外部页面。
- NSIS 只创建一个桌面快捷方式，删除历史/当前管理工具桌面入口，开始菜单仍保留管理工具；现有备份/回滚事务覆盖被删除的桌面管理入口。
- Windows 运行时 `install_shortcuts` 同样先删除管理桌面入口，只创建主入口。

## Audit-Discovered TDD Remediation

### Active profile identity

- Red：`cargo test -p codex-plus-core --test relay_config live_relay_match_requires_the_active_profile_provider_url_and_key -- --exact` 因缺少 `relay_profile_matches_live_config_from_home` 报 `E0432`。
- Green：仅当实时 provider ID、Base URL、Key 与活动 profile 一致且实时配置满足既有 `configured` 契约时才视为已应用；Key 只在内存中比较，不写入日志或错误文本。
- Fixture 校正：首次 Green 暴露测试 profile 未走生产规范化、因此没有生成 PureApi `auth.json`；三份 fixture 改为先调用 `normalize_relay_profile_for_storage`，与真实保存/apply 路径一致。
- Red：篡改实时 `requires_openai_auth = false` 后，新增负向断言先失败。
- Green：复用 `relay_config_status_from_home(home).configured` fail closed；最终 targeted `1 passed`。
- Red：保留匹配 provider/Base URL/Key/`requires_openai_auth`，但追加无关非法 TOML 时，逐行 matcher 仍错误返回 true。
- Green：普通 profile 的 expected/live identity 均先经过 `parse_toml_document`，结构化读取 provider table 并要求 `requires_openai_auth = true`；非法 TOML 直接 fail closed。完整 `relay_config` 回归 `97 passed`。

### Official login gating

- Red：`official_login_only_bypasses_setup_when_official_mode_is_active` 因缺少 `official_login_can_launch` 报 `E0432`。
- Green：ChatGPT token 仅在“中转关闭”或“活动 profile 为非混合 Official”时可直接启动；PureApi、混合 Official 和未认证状态均不能绕过配置。Targeted `1 passed`。

### Source-order test hardening

- 原测试对完整 `main.rs` 搜索，断言字符串会搜索到测试自身，存在假阳性。
- Red：把搜索范围截断在 `mod tests {` 之前后，旧 `relay_config_status_from_home` 断言立即失败，`0 passed / 1 failed`。
- Green：契约改为要求生产代码使用 `relay_profile_matches_live_config_from_home` 与 `official_login_can_launch`；最终 launcher `7 passed`。

### Key sources

- `relay_profile_has_usable_key` 契约覆盖 profile `api_key`、`authContents.OPENAI_API_KEY`、活动 provider 的 `experimental_bearer_token`。
- ChatGPT account token 与空白 `OPENAI_API_KEY` 均不得误判为中转 Key。

### Manager single-instance route delivery

- 第一版 loopback route listener 被独立 A/B 判定为 fallback 无 listener、端口碰撞缺少 manager 身份和 frontend 订阅前事件丢失。
- Red：原子请求文件/过期与非法 payload 测试在实现前失败；Green 改为 single-instance 文件锁身份、原子 JSON request、ACK 和白名单验证，固定端口只负责实例锁，不再承载路由消息。
- Red：`pending: Option<String>` 的多请求审计反例证明多个已 ACK 路由会互相覆盖；Green 改为 `VecDeque<String>`，frontend 注册 listener 后调用 ready，后端一次性排空全部 pending route。
- Red：端到端文件入口测试得到 `maintenance, relay`，而创建时间要求为 `relay, maintenance`；Green 在 `take_start_route_requests` 中按 `created_at_ms`、再按 request id 稳定排序后入队。
- Red：dispatch 后删除 `.json` 失败会让已 ACK 请求被轮询线程重复执行；Green 在验证后先原子 claim 为 `.processing`，再 dispatch、清理和写 ACK，清理失败也不会再次进入 `.json` 扫描。
- Red：批量扫描先 claim 有效请求、再遇到不可读 `.json` 目录时整体返回错误，已 claim 请求被遗失；Green 将 entry/metadata/read/rename 错误隔离到单条请求，已有 claims 仍排序返回。
- Red：ready drain 在锁外批量 emit 时，新 route 可在旧 pending 前直接 emit；Green 改为逐条 `next_pending_or_mark_ready`，队列真正清空前始终保持 not-ready。React cleanup 先调用 `manager_frontend_not_ready`，确认后才卸载 listener。
- Red：2 秒 ACK 超时删除尚未被冷启动主实例 claim 的 `.json`；Green 超时只返回状态，保留请求由主实例在 30 秒可信窗口内继续消费。
- Red：guard 通用错误改绑随机端口会 fail-open 启动多实例；Green 只有 `WouldBlock/AddrInUse` 分类为已有实例，权限等其他错误直接终止启动。
- Red：崩溃或迟到 ACK 会遗留 `.tmp/.processing/.ack`；Green 启动时和每 30 秒清理过期辅助文件，pending `.json` 不受该清理影响。
- 请求文件使用 create-new 临时文件、flush、`sync_all` 和同目录 rename；消费端拒绝超限、损坏、过期、文件 stem/id 不一致及非白名单 route。

### Installer transaction hardening

- NSIS install 对 5 个 current Chimera 入口和 10 个 legacy/compat 入口建立相同 slot 的 backup/delete/rollback 三段配对；legacy ARP 整键删除移到所有可回滚步骤完成后的最终提交点。
- Rust runtime snapshot 保存快捷方式原始字节和 registry value 原始 name/type/bytes；动态枚举错误 fail closed，Windows 行为测试验证 `.lnk` 与 `REG_EXPAND_SZ` 往返不变。
- Red：Rust install/uninstall/repair 可由多个 `spawn_blocking` 并发进入；Green 在 capture 前获取 `Local\\ChimeraPlusPlus.Setup.Transaction` named mutex，RAII guard 覆盖 apply 与 rollback，全周期并发测试通过。
- Red：NSIS `CreateMutexW` 使用 `initialOwner=0` 时 Rust wait 不会被阻塞；Green 将 install/uninstall 两处改为 `initialOwner=1`，与 Rust `WaitForSingleObject(INFINITE)` 形成双向串行。
- Red：NSIS URL scheme 整键删除只能恢复三个已知字符串值；Green 改为先删除 command 默认值、根 `URL Protocol` 和根默认值，再使用 `/ifempty` 按 command→open→shell→root 清理空键，未知值、类型和子键保持不变。

## Shortcut Transaction Review

- NSIS 成功路径：创建 `$DESKTOP\\Chimera++.lnk`，删除 `$DESKTOP\\Chimera++ 管理工具.lnk`，桌面 `CreateShortcut` 精确计数为 1。
- 开始菜单：创建主入口、管理工具和卸载入口。
- 覆盖清理：删除旧 `Codex++.lnk`、旧/乱码管理入口和兼容开始菜单项。
- 回滚：安装前备份全部 current/legacy/compat 快捷方式；任何快捷方式或注册表步骤失败时分别恢复原始字节和存在/不存在状态。
- 卸载：清理新旧桌面与开始菜单入口，只删除 owned registry values 并保守清理空键；卸载器在元数据成功前保留。

## Regression

- `cargo test -p codex-plus-core --test installers`：`23 passed`。
- `cargo test -p codex-plus-core --lib install::windows::tests::`：`4 passed`，含并发 mutex 和真实 Windows registry/shortcut round-trip。
- `cargo test -p codex-plus-core --test branding --locked`：`2 passed`。
- `cargo test -p codex-plus-core --test launcher`：`66 passed`，耗时 `563.47s`。
- `cargo test -p codex-plus-core --test relay_config live_relay_match_requires_the_active_profile_provider_url_and_key -- --exact`：最终 `1 passed`。
- `cargo test -p codex-plus-core --test relay_config`：`97 passed`。
- `cargo test -p codex-plus-launcher`：`8 passed`，非零测试数已确认。
- `cargo test -p codex-plus-manager --locked`：lib `54 passed`、`windows_subsystem 40 passed`、bin/doc tests `0 failed`。
- `npm run check`、`npm run vite:build`：PASS。
- `pwsh -NoProfile -File scripts/generate-branding.ps1 -Check`：PASS。
- `pwsh -NoProfile -File scripts/verify-no-upstream-ads.ps1`：PASS。
- `pwsh -NoProfile -File scripts/test-verify-allowlist.ps1`：PASS。
- `pwsh -NoProfile -File scripts/verify-license.ps1` 与 `-SelfTest`：PASS。
- `cargo metadata --locked --no-deps`：PASS，workspace 四包均为 `AGPL-3.0-only`。
- `cargo fmt --all -- --check`、`git diff --check`：PASS。

## Environment Limit

- 本机没有 `makensis`，因此不能把 Rust 静态契约冒充为 NSIS 实际编译；Windows installer 的真实编译与安装/覆盖/失败回滚冒烟仍由后续 Release CI 和实机 Gate 验收。
- 本 Step 不发布、不推送、不修改远端。

## Final Independent Audits

- `task-11-step-11.4-a.md`：PASS。按需求、测试与可观察行为复核 strict relay identity、路由优先级、单桌面入口、manager IPC 与 installer 事务。
- `task-11-step-11.4-b.md`：PASS。独立按 diff、边界与回归面复核；core launcher `66/66`、relay_config `97/97`、installers `23/23`、manager `54/54 + 40/40` 等均通过。
- 两份审计均保留相同非阻断环境限制：本机没有 `makensis`，NSIS 实编与真实安装故障注入留待 Task 16；Task 13 才负责 mandatory update 的完整生产状态机接线。
