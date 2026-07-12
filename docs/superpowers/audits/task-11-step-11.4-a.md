# Task 11 Step 11.4 Audit A

## Independent requirements and behavior audit

结论：**PASS**。

本轮仅按 Step 11.4、产品规格、TDD 证据、当前实现和可观察行为独立复核；未读取或引用最终审计 B，也未修改实现代码。此前发现的普通更新误强更、Aggregate 凭据短路和非法 live TOML 误判均已关闭，未发现新的需求级阻断。

## Requirements and behavior findings

### Single-entry route

- `select_launch_route` 的固定优先级符合规格：强制更新、损坏 settings、已就绪 active relay/官方登录、已有 Key 待应用、全新 Key-first。
- launcher 在 single-instance guard 和 Codex 启动前解析路由；非 Codex 路由分别映射到 `--show-update`、`--recover-settings`、`--configure-relay`、`--chimera-key-first`。
- Step 13 的可信最低支持版本尚未实现，因此当前集成层显式传入 `mandatory_update: false`；普通 `update_available` 不再被提升为强制更新。强更输入的纯路由优先级已保留，后续由 Step 13 接入可信 floor。
- `SettingsStore::load_strict` 失败进入恢复路由，不覆盖损坏文件；官方 ChatGPT 登录只在中转关闭或 active non-mixed Official 模式下允许直接启动。

### Relay readiness

- 普通 relay 只有在实时 provider ID、Base URL、Key 和 `requires_openai_auth = true` 与 active profile 一致时才 ready；expected/live 配置均先严格解析完整 TOML，文件任意位置非法都会 fail closed。
- Aggregate 不再要求 profile 存在用户 Key。launcher 识别已解析的 active Aggregate，并使用本地 Responses proxy URL、内部 token 和认证标志精确核验 live identity。
- Key 检测覆盖 profile `api_key`、`authContents.OPENAI_API_KEY` 和 active provider 的 `experimental_bearer_token`；ChatGPT account token、空白 Key 和不匹配 live 配置均不能误判为已应用中转。
- 比较过程未把 Key 写入日志、错误文本或审计输出。

### Manager route delivery

- manager 只接受受限内部 route `maintenance` / `relay`，未增加 GitHub、Release 或外部客户入口。
- single-instance 请求通过同目录 `.tmp -> .json -> .processing` 原子交接；请求按 `created_at_ms + id` 稳定排序，使用 FIFO 队列等待 frontend ready。
- ACK 超时保留尚未 claim 的 pending `.json`；坏条目逐项隔离，不丢弃已 claim 请求；过期清理只处理辅助 `.tmp/.processing/.ack`，不删除可信窗口内 pending 请求。
- frontend cleanup 先标记 not-ready 再卸载 listener；guard 仅把 `WouldBlock/AddrInUse` 视为已有实例，其他错误 fail closed。

### Installer and shortcuts

- NSIS 成功路径在桌面只创建一个 `Chimera++.lnk`；`Chimera++ 管理工具` 仅保留在开始菜单，覆盖安装清理当前、legacy 和兼容桌面管理入口。
- current/legacy/compat 快捷方式具有成对 backup/delete/rollback；legacy ARP 删除位于可回滚步骤后的最终提交点。
- Rust 安装事务保存快捷方式原始字节和 registry value 原始 name/type/bytes；安装、修复和卸载由 `Local\\ChimeraPlusPlus.Setup.Transaction` 串行化。
- NSIS 与 Rust 使用同名 mutex，NSIS 以 `initialOwner=1` 获取所有权；URL protocol 只删除 owned values，并用 `/ifempty` 保守清理空键，未知值、类型和子键不被整键破坏。
- 法律文件与二进制、快捷方式和注册表处于同一安装事务及回滚边界。

## TDD evidence review

- Evidence 保留了 pure route、active profile identity、official login、Key sources、Manager IPC 和 installer transaction 的 Red/Green/回归记录。
- 审计发现的非法 TOML 反例已记录 Red：旧逐行 matcher 在 provider/URL/Key 匹配但追加非法 TOML 时仍返回 true。
- Green 改为 `parse_toml_document` 后，普通 relay targeted、Aggregate identity 和完整 relay_config 回归均通过。
- 既有 Aggregate 路由阻断已由 `active_relay_has_launch_credentials`、Aggregate live matcher 和 launcher 集成分支关闭；普通更新误强更已由禁止 `update_available -> mandatory_update` 映射关闭。

## Independent verification

- 普通 relay live identity（含非法 TOML、认证标志负向）：`1 passed`。
- Aggregate local proxy identity：`1 passed`。
- LaunchRoute priority、official login gate、Aggregate credential resolution：各 `1 passed`。
- `cargo test -p codex-plus-launcher --locked`：`8 passed`。
- `cargo test -p codex-plus-manager --locked`：lib `54 passed`、`windows_subsystem 40 passed`、bin/doc tests `0 failed`。
- `cargo test -p codex-plus-core --test installers --locked`：`23 passed`。
- `cargo test -p codex-plus-core --lib install::windows::tests:: --locked`：`4 passed`。
- `npm run check`、`npm run vite:build`：PASS，Vite `1608 modules transformed`。
- `cargo fmt --all -- --check`、`git diff --check`：PASS；仅存在 Git 行尾转换提示，无 whitespace error。

## Residual environment limit

- 本机没有 `makensis`。本轮只能对 NSIS 做静态契约、Rust 行为和集成测试审计，不能声称已完成 NSIS 实际编译、全新/覆盖安装或故障回滚冒烟；这些仍由 Release CI 和 Task 16 Windows 实机 Gate 验收。
- Step 13 的真实强制更新 floor、自动安装状态机不属于 Step 11.4；当前只验证单入口能正确接收强更决策且不会把普通更新误判为强更。

从审计 A 侧，Step 11.4 的代码与文档门已满足；仍须按项目流程由另一份独立审计和 Task Gate 决定是否勾选聚合任务。
