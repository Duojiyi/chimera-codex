# Task 11 Step 11.4 Route Remediation Audit A

## Final independent review

结论：**PASS**。

本轮只复核上一轮指出的三个 Aggregate 测试缺口及 pure helper 的生产接线，未发现新阻断。

### 已关闭项

- 错误本地代理 URL：`aggregate_live_match_requires_the_local_proxy_identity` 把 `127.0.0.1:57321` 改为其他端口并断言 matcher 返回 false。
- configured=false：同一用例把 `requires_openai_auth = true` 改为 false 并断言 matcher 返回 false；另有无效 TOML 负例，解析异常同样 fail closed。
- orphan Aggregate：新增 pure `active_relay_has_launch_credentials(active_aggregate_exists, active_profile_has_key)`；矩阵证明 active aggregate 存在可提供内部凭据语义、普通 profile Key 仍有效、两者都不存在时返回 false。

### 生产接线

- launcher 以 `settings.active_aggregate_relay_profile().is_some()` 产生 `active_aggregate`，不会仅凭 `RelayMode::Aggregate` 把 orphan profile 当作已解析的 active aggregate。
- 生产代码把该布尔值与普通 profile Key 传给 `active_relay_has_launch_credentials`；pure helper 不是仅测试使用。
- 只有 `active_aggregate=true` 才调用 `aggregate_relay_matches_live_config_from_home`。matcher 同时要求 `relay_config_status_from_home(...).configured`、精确本地 Responses proxy URL 和精确内部 token `codex-plus-aggregate`。
- active aggregate 不存在时走非 Aggregate 分支，并继续要求普通 profile 有 Key且 provider ID/Base URL/Key 与 live config 精确匹配。
- Aggregate live mismatch 时不会产生 `active_relay_live_ready=true`；因 resolved aggregate 提供配置凭据语义，会进入继续配置而非错误启动 Codex。

### 其他确认

- 普通 update_available 未重新接入 mandatory；生产路由固定传 `mandatory_update=false`，等待 Task 13 的可信最低支持版本状态。
- 纯 `select_launch_route` 仍保留 `mandatory_update=true` 的最高优先级，供 Task 13 接线。
- Official gating 与非 Aggregate 精确匹配实现未因本修复回归。

### Targeted results

- `cargo test -p codex-plus-core --test relay_config aggregate_live_match_requires_the_local_proxy_identity --locked -- --exact`：1/1 PASS。
- `cargo test -p codex-plus-core --test launcher aggregate_credentials_require_a_resolved_active_aggregate --locked -- --exact`：1/1 PASS。
- scoped `git diff --check`：PASS（仅现有工作树行尾转换提示，无 whitespace error）。

启动语义修复包①在本审计范围内可以关闭。
