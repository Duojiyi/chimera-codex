# Task 2 Audit A — Requirements Coverage

**Date:** 2026-07-26  
**Scope:** Step 2.1–2.5 (Provider Engine)  
**Auditor:** Independent A (requirements only, no reference to B)

## Step 2.1 — SQLite Repository

| Requirement | Evidence | Status |
|---|---|---|
| ChimeraHub 是唯一内置模板 | `chimera_hub_is_only_builtin_kind` test: 只允许1行 ChimeraHub | PASS |
| Provider schema 支持 Custom/ChimeraHub | ProviderRow.kind 枚举，DB 两种 kind 均可存取 | PASS |
| Key 不进 DB | ProviderRow 无 `api_key` 字段，compile-time 测试验证 | PASS |
| secret_ref 健康更新后保留 | `update_health_does_not_touch_secret_ref` | PASS |
| Schema migration 幂等 | `reopening_db_applies_pending_migrations` | PASS |
| WAL 模式 | `PRAGMA journal_mode=WAL` 在 open() 中执行 | PASS |

## Step 2.2 — URL 安全验证

| Requirement | Evidence | Status |
|---|---|---|
| 非 HTTPS 拒绝 | `http_url_is_rejected_outside_loopback` | PASS |
| loopback HTTP 仅 dev mode 允许 | `http_loopback_allowed_only_in_dev_mode` | PASS |
| userinfo 禁止 | `userinfo_in_url_is_rejected` | PASS |
| fragment 禁止 | `fragment_in_url_is_rejected` | PASS |
| origin-only 暴露 /v1 候选，不静默写入 | `url_with_origin_only_returns_candidate_with_v1` (v1_candidate 字段) | PASS |
| 显式路径保留 | `url_with_explicit_v1_path_is_accepted_verbatim` | PASS |

## Step 2.3 — Keychain Port

| Requirement | Evidence | Status |
|---|---|---|
| Key 不进 DB，只存 secret_ref | `KeychainPort` trait 分离，`MemoryKeychain` 测试替身 | PASS |
| SecretRef debug 不暴露 key | `secret_ref_debug_does_not_expose_key` | PASS |
| 删除清除 secret | `delete_removes_secret` | PASS |
| 测试隔离 | `memory_keychain_is_isolated_per_instance` | PASS |

## Step 2.4 — Codex config.toml 投影

| Requirement | Evidence | Status |
|---|---|---|
| 未知字段和 MCP 保留 | `unknown_fields_are_preserved_after_projection` (some_user_custom_section, mcp_servers) | PASS |
| 官方登录 [auth] 不被覆盖 | `official_login_section_is_not_overwritten` | PASS |
| 空 config 可安全初始化 | `projection_on_empty_config_produces_valid_toml` | PASS |
| revert 只删 Chimera 字段 | `revert_restores_pre_projection_state` | PASS |
| Key 出现在投影后的 config | `api_key_in_plain_text_must_be_injected_as_env_or_direct` | PASS |

## Step 2.5 — CAS 事务切换 + Journal

| Requirement | Evidence | Status |
|---|---|---|
| 正常切换提交并清空 journal | `happy_path_switch_updates_config_and_clears_journal` | PASS |
| snapshot 后外部改写被 CAS 检测到 | `cas_detects_external_write_after_snapshot` (external_change = true 保留) | PASS |
| Journal 在 atomic rename 前写入 | `journal_written_before_atomic_rename` | PASS |
| 锁防止并发切换 | `second_transaction_fails_while_first_holds_lock` | PASS |
| snapshot_hash 检测字节变化 | `snapshot_hash_detects_byte_change` | PASS |

## 结论

**PASS。** Steps 2.1–2.5 全部需求已通过测试覆盖。  
Open note: Step 2.2 URL 探测（网络能力探测，模型列表发现）属于实现门，在此文档层面标记为
"待 Task 2 网络探测 adapter 实现"，不阻断当前 Step 审计。
