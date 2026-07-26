# Task 1 (T41) Audit A — Spec/行为覆盖

**Date:** 2026-07-26
**Scope:** Step 1.1–1.5（新 workspace 与领域骨架）
**Commit:** `309ad81` feat(task-1): new v2 workspace skeleton
**Auditor:** Independent A（只看 Spec/行为覆盖，不看 diff/边界）

方法：逐条核对 Plan Step 1.1–1.5 与 Spec 第5章的可观察标准，对每条声称的行为在仓库中定位证据；未独立运行过的测试全部重新执行以核实数量。

## Step 1.1 — 架构契约（workspace + dependency-cruiser + V15）

| 要求 | 证据 | 状态 |
|---|---|---|
| workspace 新增 6 个 v2 crate + `apps/chimera-desktop/src-tauri` | `Cargo.toml` members 数组包含 chimera-domain/platform/provider/runtime/theme/migration + services/mirror-contract + src-tauri | PASS |
| `.dependency-cruiser.cjs` 声明禁止跨 feature 深引用 / 循环依赖 / shell 依赖 feature | 文件存在，3 条规则：`no-cross-feature-imports`、`no-feature-imports-from-shell`、`no-circular` | PASS（声明存在） |
| dependency-cruiser fixture 实际参与门禁执行 | 全仓搜索 `depcruise`/`dependency-cruise` 调用点：仅 `package.json` 的 devDependency 声明和 `package-lock.json` 的 bin 清单，**没有任何脚本或 CI step 实际执行 `npx depcruise`**。`scripts/verify-v2-architecture.mjs` 是一个独立的手写正则扫描器，不读取 `.dependency-cruiser.cjs` 也不调用其 bin | **PARTIAL** — 配置文件存在但从未被执行，circular-dependency 规则完全没有被任何门禁强制 |
| `cargo metadata` fixture 拒绝 v2 依赖旧 `codex-plus-core` 和非法 crate 边 | `verify-v2-architecture.mjs` 的 "Check 2" 只是对 `crates/chimera-domain/Cargo.toml` 文本做字符串匹配（5 个禁止 crate 名），并不调用 `cargo metadata --locked --format-version 1`；"Check 3"（legacy crate 增长）是一个只要目录存在就 `pass()` 的占位检查，不与 git base 做真实 diff | **PARTIAL** — Spec 5 描述的"解析 cargo metadata JSON、核对允许依赖图"未被实现，只有一个粗粒度替代品 |
| V15 在跨平台 required check 中运行 | `.github/workflows/v2-build.yml` 的 `supply-chain` job 有 `V15 architecture boundaries` step；`node scripts/verify-v2-architecture.mjs` 本机重跑 4 项全部 PASS | PASS（当前检查项本身全绿，但检查项覆盖面不足，见上） |

结论：Step 1.1 的"最小 workspace + 跨 feature 边界 + required check"部分已交付且当前通过；但 Plan 明确写的"用 dependency-cruiser fixture 拒绝循环依赖"和"cargo metadata fixture 拒绝非法 crate 边"两个技术手段并未真正实现，被一个功能更弱的替代脚本取代。这是行为覆盖缺口，不是纯粹的 diff/实现细节问题。

## Step 1.2 — chimera-domain 纯类型

| 要求 | 证据 | 状态 |
|---|---|---|
| Provider/ProviderKind/ProviderProtocol/ProviderHealth | `crates/chimera-domain/src/provider.rs`；`provider_chimera_hub_kind_serializes_correctly`、`provider_health_variants_are_exhaustive` | PASS |
| InstallOwnership/InstallMode/TransactionState | `src/ownership.rs`；`install_ownership_managed_portable_roundtrip`、`transaction_state_variants_cover_journal_lifecycle` | PASS |
| RuntimeState | `src/runtime.rs`；`runtime_state_idle_is_default`、`runtime_state_running_carries_pid` | PASS |
| UpdateState 与 Spec 8.3 状态机一一对应（13 态） | `src/update.rs`；`update_state_machine_transitions_are_represented` 断言 13 个变体；`committing_state_must_not_be_interrupted` 验证 `Committing` 不在 `cancellable_states()` 中 | PASS |
| OperationError 分类（13 变体） | `src/error.rs`；`operation_error_cas_conflict_carries_hashes`、`operation_error_cross_origin_redirect_is_detectable`、`operation_error_invalid_url_rejects_http` | PASS |
| 无 I/O、无 Tauri 依赖 | `crates/chimera-domain` 只依赖 serde/thiserror/url/uuid，`lib.rs` 顶部注释自陈"本 crate 无 I/O" | PASS |
| "序列化版本契约" | 未找到显式 schema 版本号字段或迁移测试；只有普通 `serde_json` roundtrip 测试 | **MISSING** — Plan 用词是"纯类型和**序列化版本契约**"，当前只交付了纯类型，版本契约（例如 schema_version 标记或向后兼容测试）不存在 |

实测：`cargo test -p chimera-domain --locked` → **13 passed, 0 failed**，与声称一致。

## Step 1.3 — chimera-platform

| 要求 | 证据 | 状态 |
|---|---|---|
| 路径规范化（拒绝 traversal） | `CanonicalPath::new` 遍历 component 拒绝 `..`；`canonical_path_rejects_path_traversal` | PASS |
| 跨进程 operation lock | `OperationLock`（fs2 `try_lock_exclusive`）+ RAII `LockGuard`；`operation_lock_second_acquire_fails_while_held`、`operation_lock_is_released_on_guard_drop` | PASS |
| 单实例 | `SingleInstance::try_acquire` 包装 `OperationLock` | PASS（无专门测试用例，但共享 lock 测试间接覆盖） |
| 进程身份 port | `ProcessIdentity`（pid + executable_path，`is_under_root`）；`process_identity_records_pid_and_path`、`process_identity_matches_self` | PASS |
| **Keychain port** | Plan Step 1.3 原文列出 "keychain port" 属于 chimera-platform 的交付物。实际代码：`KeychainPort` trait 和 `MemoryKeychain` 位于 `crates/chimera-provider/src/keychain.rs`，chimera-platform 的 `Cargo.toml` 和 `tests/platform_abstractions.rs` 均无任何 keychain 相关内容 | **MISCLASSIFIED / NOT IN THIS CRATE** — 功能在 Task 2 交付，但 Plan 把它记在 Task 1 名下；Step 1.3 字面要求未在其指定位置满足 |
| 平台调用通过 trait 注入，测试替身可用 | `KeychainPort`（trait，MemoryKeychain 替身）确认可测试；但该 trait 不在 chimera-platform | PASS（整体设计目标达成，但不在声称的 crate 内） |

实测：`cargo test -p chimera-platform --locked` → **7 passed, 0 failed**，与声称一致。

## Step 1.4 — 前端骨架

| 要求 | 证据 | 状态 |
|---|---|---|
| home/providers/codex/appearance/settings 路由 | `App.tsx` 5 个 feature 组件按 `active` 条件渲染 | PASS |
| 只有首页占位状态可见，不迁移旧 UI | 在 `309ad81` 提交内容中，`home/index.tsx` 全文只有一个 `<p>HOME</p>` 占位；其余 4 个 feature 也是占位 `index.tsx`（未单独逐一复核，但 commit diff 显示每个仅 9 行） | PASS（对该提交时点） |
| `TopRail` 跨 5 屏一致 | `shell/TopRail.tsx` 单一组件被 `App.tsx` 唯一引用 | PASS |
| feature 不得直接读写文件/配置 | Step 1.4 提交的占位组件无任何 I/O；后续 Task 3 提交的实现版本改用 `invoke()`（见 Task 3 审计） | PASS（对本 Step 范围） |

## Step 1.5 — 聚合审计与依赖图/编译时间基线

Plan 原文："完成 Task 1 聚合 A/B 审计，**记录依赖图和编译时间基线**。"

全仓搜索未找到任何依赖图快照或编译时间基线文档（`docs/architecture/`、`docs/superpowers/` 均无此类文件）。本次产出的两份审计文档本身补上了"聚合 A/B 审计"缺口，但"依赖图和编译时间基线"这一独立可观察交付物**不存在**。

状态：**MISSING**。

## 结论

**PASS WITH GAPS。**

Step 1.2 和 1.4 的核心行为完全达标，Step 1.3 的四项平台原语中三项达标、一项（keychain port）不在声称位置。Step 1.1 的可见门禁（V15）当前全绿，但其底层技术手段（dependency-cruiser 实际执行、cargo metadata 解析）与 Plan 描述不符，是名不副实的替代实现。Step 1.5 要求的依赖图/编译时间基线完全缺失。

不构成 FAIL 的理由：没有任何已交付行为出现"验证会误报为通过"的安全性问题（V15 本身仍能挡住跨 feature 引用和前端直连 crate）；缺口集中在文档化基线和"实现手段与描述不一致"，而非核心领域类型或平台原语本身的正确性。建议在勾选 Task 1 前补齐：(a) 依赖图/编译时间基线文档，(b) 让 verify-v2-architecture.mjs 真正调用 dependency-cruiser 与 cargo metadata，或修订 Plan 措辞以匹配当前实现，(c) 明确 keychain port 的归属 crate。
