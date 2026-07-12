# Task 11 Step 11.4 修复包①独立审计 B

## 结论

**PASS**

修复包已正确移除普通更新到 mandatory update 的生产映射，并保留未来 mandatory update 的路由入口；非 aggregate relay 与 Official 登录分支未发现本修复包引入的回归。复核确认 aggregate live identity 已改为先结构化解析 `config.toml`，先前报告的非法 TOML fail-open 已关闭，新负例通过。

本审计未读取、未引用对应审计 A，也未修改实现。

## 原阻断关闭复核

### B1. aggregate 非法 TOML fail-open：已关闭

- `crates/codex-plus-core/src/relay_config.rs:274-282` 现在读取 live 文件后立即调用 `parse_toml_document`；读取或 TOML 解析失败均直接返回 false。
- 同文件 `:283-311` 从 `DocumentMut` 通过 typed `Item::as_str` / `Item::as_bool` 读取活动 provider、Base URL、哨兵 token 与 `requires_openai_auth`，不再使用逐行文本扫描判定 aggregate identity。
- 因此先前报告的下列非法 TOML 会在结构化解析阶段被拒绝，不可能进入 identity 相等分支：

```toml
model_provider = "custom"

[model_providers.custom]
requires_openai_auth = true
base_url = http://127.0.0.1:48761/v1
experimental_bearer_token = codex-plus-aggregate
```

- `crates/codex-plus-core/tests/relay_config.rs:76-129` 新增 Base URL、`requires_openai_auth` 与非法未引号 URL/token 负例；每个被篡改状态都断言 `aggregate_relay_matches_live_config_from_home` 返回 false。
- 新负例直接复现先前审计示例，而不是仅以静态源码 token 代替行为验证。

**复核判定：已关闭。** 结构化解析成为 identity 比较的前置条件，非法 TOML 路径 fail-closed。

## 已通过检查

### 普通更新映射已真实移除

- `apps/codex-plus-launcher/src/main.rs` 的生产代码已不再调用 `check_for_update`，也没有读取 `update_available`；`resolve_single_entry_route` 在 `:206-247` 明确以 `mandatory_update: false` 构造当前输入。
- 旧的后台 `notify_manager_when_update_available` 路径已从 `main` 移除，普通可选更新不再阻止 Codex 启动。
- `apps/codex-plus-launcher/src/main.rs:940-946` 的静态回归断言覆盖旧 `.map(|update| update.update_available)` 形态，但该测试本身较窄；本结论以生产代码全量搜索为主。

### 未来 mandatory update 入口未被破坏

- `crates/codex-plus-core/src/launcher.rs:53-79` 仍保留 `LaunchRouteInput.mandatory_update`，且 true 时最高优先级返回 `LaunchRoute::ManagerUpdate`。
- `apps/codex-plus-launcher/src/main.rs:250-256` 仍将 `ManagerUpdate` 映射为 `--show-update`。
- `crates/codex-plus-core/tests/launcher.rs:30-48` 覆盖 mandatory 为 true 时压过 settings、relay 和 official 状态。未来最低支持版本数据可接入现有输入点，无需恢复普通更新误映射。

### aggregate identity 未泄露真实凭据

- `crates/codex-plus-core/src/relay_config.rs:274-311` 只在内存中比较本地代理 URL 与公开固定哨兵 `AGGREGATE_RELAY_BEARER_TOKEN`，没有读取 aggregate 成员真实 Key，也没有日志输出。
- `apps/codex-plus-launcher/src/main.rs:206-247` 只传递布尔结果，不记录 profile、auth 内容或 identity 值。
- 未发现该修复新增真实凭据进入错误文本、诊断日志或测试输出的路径。

### 非 aggregate 与 Official 分支未见修复包回归

- 非 aggregate 仍由 `apps/codex-plus-launcher/src/main.rs:227-232` 同时要求 usable key 与 `relay_profile_matches_live_config_from_home` 的活动 provider/Base URL/Key 匹配，没有被 aggregate 的固定 identity 分支旁路。
- `crates/codex-plus-core/src/relay_config.rs:230-271` 对读取失败、期望配置生成失败或 identity 缺项返回 false，并比较活动 provider ID、Base URL 与 Key。
- Official 登录仍由 `apps/codex-plus-launcher/src/main.rs:234-238` 调用 `official_login_can_launch`；`crates/codex-plus-core/src/launcher.rs:88-99` 只在 relay 关闭，或 relay 开启且活动模式为非混合 Official 时允许已认证登录直接启动。
- aggregate profile 存在性由 `settings.active_aggregate_relay_profile().is_some()` 决定；缺失或 ID 错配不会进入 aggregate live matcher。

## 定向验证

- `cargo test -p codex-plus-core --test relay_config aggregate_live_match_requires_the_local_proxy_identity --locked -- --exact`：复核后 **PASS**，1 passed、0 failed、96 filtered out；包含新增非法 TOML 负例。
- launcher 可选更新测试以未包含模块路径的 `--exact` 过滤执行，结果为 0 tests；不计为通过证据。收到停止长时命令要求后未继续补跑。

## 最终判定

普通更新映射移除、未来 mandatory 入口保留、aggregate fail-closed、凭据保护及既有分支隔离均满足本修复包审计范围。先前唯一阻断已经由结构化解析与对应负例关闭，本修复包①最终结论为 **PASS**。
