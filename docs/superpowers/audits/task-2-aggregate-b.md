# Task 2 Audit B — Diff, Boundary, Failure Recovery

**Date:** 2026-07-26  
**Scope:** Step 2.1–2.5 (Provider Engine)  
**Auditor:** Independent B (diff/boundary only, no reference to A)

## Step 2.1 — SQLite Repository

**Boundary checks:**
- `schema_version` 从0升到1时幂等：`INSERT INTO _schema_version` 只在 `ver < 1` 时执行 ✓
- WAL模式: `PRAGMA journal_mode=WAL` 在每次 `open()` 时设置，不受连接重置影响 ✓
- `get_by_id` 返回 `Option<ProviderRow>` — 不存在不报错 ✓

**Risk note:** `uuid::Uuid::parse_str().unwrap()` 在 DB 数据损坏时会 panic。应改为 `?` 传播——标记为 Step 2.6 open item（P2，不阻断）。

## Step 2.2 — URL 验证

**Boundary checks:**
- HTTP loopback 判断：检查 host_str() 是否为 "127.0.0.1" | "localhost" | "::1"。IPv6 loopback (::1) 已覆盖 ✓
- `ftp://` 和 `file://` 被 "不是 https 也不是 http" 分支拒绝 ✓
- 空字符串 → Empty 错误，不 panic ✓
- Origin-only URL (`https://api.example.com`)：path 为 "/" 时产生 v1_candidate，不静默写入 ✓

**Risk note:** URL 的有界 `/v1` 候选探测在 `execute_with_pre_cas_hook` 测试中未覆盖网络部分——这是预期的（网络探测在 Step 2.2 adapter 实现阶段）。

## Step 2.3 — Keychain Port

**Diff check:**
- `MemoryKeychain` 存储使用 `Arc<Mutex<HashMap>>` — 多线程安全 ✓
- `SecretRef` 实现了 `Clone + PartialEq + Eq + Hash` — 可用作 Map key ✓
- `Debug` impl 只打印引用路径，不打印值 ✓

**Failure boundary:**
- `delete` 对不存在的 key 不报错（remove 是幂等的）— 可接受 ✓

## Step 2.4 — Config 投影

**Diff check:**
- `apply_provider_projection` 用 `toml_edit::DocumentMut` 修改——保留注释和格式 ✓
- `revert_provider_projection` 只删 `chimera_managed=true` 的文件中的 Chimera keys ✓
- `model` key 在 revert 时不删除（用户可能自己设了 model）✓

**Failure boundary:**
- TOML parse error 返回 `ProjectionError::TomlParse`，不 panic ✓
- `chimera_managed` flag 缺失时 revert 为 no-op（安全降级）✓

**Risk note:** `doc["api_key"] = value(key)` 在 Official 模式下不应执行——当前实现对 Official 模式无特殊处理。标记为 Step 2.4 open item（P2）。

## Step 2.5 — CAS 事务

**Diff check:**
- Lock 在 execute 第一步获取，在 `_guard` drop 时自动释放（RAII）✓
- Journal 写入使用 `tmp → rename` 原子写，不会产生半写 journal ✓
- CAS 比较使用 SHA-256 内容 hash，不依赖 mtime（mtime 在 Windows 上不可靠）✓
- 外部改写检测：`pre_cas_hook` 模拟了"snapshot 后文件被外部修改"场景 ✓
- CAS 冲突时：staged 文件被删除，原文件不改变，返回 Conflict 而非 Err ✓

**Failure boundary:**
- Lock 失败 → `TxError::Lock`，不执行任何写操作 ✓
- Secret 不存在 → `TxError::SecretMissing`，在 journal 写入前返回 ✓
- Staged rename 失败（如磁盘满）→ 原文件未修改，staged 文件留在磁盘（下次启动可清理）

**Open gate P1：** 磁盘满导致 staged rename 失败后，journal 状态为 Pending，staged 文件存在。下次启动的恢复逻辑（replay journal）尚未实现——需要在 Task 2 完整版或 Task 9 中补充。此为文档层面的预期 gap，不阻断当前步骤。

## 结论

**PASS（含 open items）。** 3个 open items（uuid unwrap P2、Official 模式投影 P2、journal replay P1）均已记录，不阻断 Task 2 推进到 Task 3。P1 在 Task 9 自更新/诊断中关闭。
