# Task 5 (T45) Audit A — Spec/行为覆盖 (v2)

**Date:** 2026-07-26
**Scope:** Step 5.1–5.6（Windows managed runtime），commit `334b626`
**Auditor:** Independent A（只看 Spec/行为覆盖，不看 diff/边界）

## 方法

对照 Plan `Task 5（T45）` 与 Spec 第 8 节（8.1-8.4）的逐条 Step 要求，逐一检查代码是否存在、是否可观察、测试是否覆盖。已实际运行：

```
cargo test -p chimera-runtime --locked   # 25 passed, 0 failed
```

25 = Step 5.1(6) + Step 5.2(7) + Step 5.3/5.4(6) + Step 5.5(6)。

## Step 逐条核查

### Step 5.1 — ManagedPortable/ExternalMsix/ExternalPortable 探测、canonical path、junction/symlink、错误 ownership；Green 实现只读 plan

Status: **PARTIAL**

- `crates/chimera-runtime/src/detection.rs:12-22` 的 `DetectedRuntime` 枚举包含 `ManagedPortable`、`ExternalMsix`、`ExternalPortable`、`Unknown` 四个变体，与 Spec 8.1 三种安装模式对齐。
- `detect_runtime()`（`detection.rs:42-75`）：读取 `ownership.json`，若不存在则返回 `Unknown`；若存在则校验 `canonical_path` 与实际目录一致，否则返回 `CanonicalPathMismatch`。测试 `detects_managed_portable_by_ownership_file`、`no_ownership_file_returns_not_managed`、`ownership_canonical_path_mismatch_returns_error` 均通过。
- 路径穿越拒绝：`detect_runtime` 对 `..` component 显式拒绝（`detection.rs:44-48`），测试 `path_traversal_in_runtime_dir_is_rejected` 通过。
- **缺口**：`ExternalMsix` 和 `ExternalPortable` 两个变体在整个代码库中**从未被实际构造**（`grep "DetectedRuntime::ExternalMsix\|DetectedRuntime::ExternalPortable"` 只匹配到枚举定义本身和 `parse_ownership_from_value` 内部的字符串匹配分支，没有任何函数返回这两个变体的实例，也没有任何测试覆盖"检测到系统已注册的官方 MSIX"或"检测到用户自己的 portable 目录"这两个场景）。`detect_runtime` 目前只能返回 `ManagedPortable` 或 `Unknown`，无法真正探测外部安装。Spec 8.1 明确要求"检测到其他安装时，Chimera 可以继续使用健康的外部安装，或经用户明确确认迁移"——这一行为完全未实现，因为探测本身就做不到。
- **缺口**：junction/symlink 处理——`detection.rs:65` 用 `std::fs::canonicalize` 解析实际路径，理论上会跟随 symlink/junction，但**没有任何测试构造真实的 symlink/junction 并验证行为**（`chimera-platform/src/canonical_path.rs:16` 的注释自己承认"实际 symlink 解析在 platform-specific adapter 中进行"，但 `chimera-platform` 里没有对应的 Windows junction 处理代码）。Spec 明确点名的"junction/symlink"边界没有可观察的测试证据。
- "只读 plan"——Spec 8.1 要求探测后先形成只读计划、不直接修改。当前 `detect_runtime` 确实是纯读取函数，不做任何写入，这一点符合。

结论：Step 5.1 的 ManagedPortable 路径（自己创建的目录）覆盖完整，但 ExternalMsix/ExternalPortable 探测——这是 Step 5.1 标题本身列出的三分之二——完全没有实现，junction/symlink 场景无测试证据。

### Step 5.2 — MSIX manifest、架构、OpenAI Authenticode、hash、大小和下载限制；Green 实现 verify/stage，不进入 commit

Status: **PARTIAL**

- `crates/chimera-runtime/src/verify.rs:16-27` 的 `verify_payload_hash` 做 SHA-256 校验，7 个测试中 4 个覆盖此函数（正确/错误 hash、错误信息携带双 hash、缺失文件），全部通过。
- `check_msix_codex_identity`（`verify.rs:44-55`）校验 package name 前缀 `OpenAI.Codex` 和 publisher 白名单，3 个测试覆盖（合法身份、错误 publisher、错误 package name），全部通过。
- **缺口**：这只是"字符串比较"，不是真实的 MSIX manifest 解析——没有代码读取 `.msix` 文件内的 `AppxManifest.xml`，`package_name`/`publisher` 两个参数在测试里都是手工传入的字面量字符串（`valid_codex_identity_passes` 直接写 `check_msix_codex_identity("OpenAI.Codex", "OpenAI, Inc.")`），生产路径上没有任何函数从真实 MSIX 包里提取这两个字段。
- **缺口**：Spec 明确列出的"架构"（arch）校验——`verify.rs` 里完全没有架构相关的函数或字段，`MsixIdentityResult` 只有 `Valid | UnknownPublisher | UnknownPackage` 三个变体，没有 `UnknownArchitecture`。架构校验缺失。
- **缺口**："OpenAI Authenticode"——`check_msix_codex_identity` 的 publisher 检查是裸字符串比较（`OPENAI_PUBLISHERS` 数组），不是真实的 Authenticode 证书链验证（没有引入任何证书解析/验证库，`Cargo.toml` 里没有 `windows` crate 的证书相关 API 或第三方签名验证库）。
- **缺口**："大小和下载限制"——`verify.rs` 没有任何 size 上限检查函数或常量。
- "不进入 commit"——代码结构上 `verify.rs` 与 `update.rs` 的 `commit_version` 确实是分离的模块，没有相互调用，这点结构上符合。

结论：Step 5.2 只实现了"给定两个字符串做名称/hash 比较"，真实的 MSIX 包解析、Authenticode 验证、架构校验、大小限制均缺失。

### Step 5.3 — 官方 MSIX 解包、文件树摘要、版本目录、current.json 和至少一版备份；Green 实现首次 managed install

Status: **PARTIAL**

- `crates/chimera-runtime/src/update.rs` 的 `RuntimeLayout` 定义了 `versions/`、`staging/`、`backup/` 三个目录和 `current.json` 指针，`initialise()` 创建前三者，6 个测试中 `layout_initialise_creates_required_dirs`、`stage_and_commit_creates_version_dir_and_current_pointer`、`current_pointer_roundtrips` 覆盖了目录布局和指针读写，全部通过。
- **缺口**："官方 MSIX 解包"——`stage_version()`（`update.rs:95-99`）只是 `fs::create_dir_all`，不做任何 MSIX 文件的解压/解包。测试里用 `fs::write(staged_dir.join("Codex.exe"), b"fake-exe")` 手工放入一个假文件模拟"已解包完成的状态"，没有一行代码或测试真正处理 `.msix` 格式。
- **缺口**："文件树摘要"（file tree digest）——`chimera-domain::InstallOwnership` 结构体里有 `file_tree_digest: String` 字段（`crates/chimera-domain/src/ownership.rs:45`），但 `chimera-runtime` 中**没有任何函数计算真实的文件树摘要**；`write_ownership_manifest` 把 `file_tree_digest` 当作外部传入的字符串参数直接写入，调用方（测试）传的是字面量 `"sha256:def"`，不是真实计算结果。
- 版本目录（`versions/<v>/`）和 `current.json`：已实现且有测试覆盖，符合。
- 备份：`backup_dir()` 方法存在（`update.rs:52`），`initialise()` 会创建该目录（测试 `layout_initialise_creates_required_dirs` 验证目录存在），但**没有任何代码把旧版本写入 `backup/` 目录**——`commit_version` 在替换旧版本目录时直接 `fs::remove_dir_all(&version_dir)`（`update.rs:112`）后 `fs::rename`，旧内容被删除而不是移到 backup。"至少保留一版备份"这一 Spec 硬性要求（8.2 原文："至少保留一个最后已知可用版本"）实际上是靠 `versions/` 目录本身没被清理（rollback 依赖的是 `versions/<旧版本>/` 还存在，而不是 `backup/`）实现的，`backup/` 目录本身形同摆设——创建了但从未写入。这是名不副实的实现：目录结构对了，语义不对。

结论：目录骨架和指针机制到位，但"官方 MSIX 解包"和"文件树摘要"计算均为占位，"备份"目录创建了但未被使用，真正的回滚保证依赖的是另一套机制（见 Step 5.4）。

### Step 5.4 — 下载到 health-check 全状态机 fault injection、取消/暂停、磁盘满、断电和重启恢复；Green 实现 atomic pointer switch 与 rollback

Status: **MISSING（fault injection 部分）/ PARTIAL（atomic switch 部分）**

- `commit_version()` 的 `current.json` 写入使用 tmp 文件 + `fs::rename` 的原子替换模式（`update.rs:80-90`），这是合理的原子指针切换实现。
- `rollback_to_last_known()`（`update.rs:134-156`）读取 `previous_version` 字段并切回，若无 `previous_version` 或目标目录不存在则返回 `NoPreviousVersion` 错误。测试 `rollback_to_last_known_good_restores_previous`、`rollback_with_no_previous_version_returns_error` 均通过。
- Spec 8.3 定义的完整状态机（`Idle → Checking → Available → Downloading ↔ Paused → Verifying → Staged → WaitingForSafeRestart → Committing → HealthChecking → Succeeded/RolledBack/FailedRecoverable`）在 `chimera-domain/src/update.rs` 中作为纯类型 `UpdateState` 定义齐全（13 个变体，`domain_types.rs` 里 `update_state_machine_transitions_are_represented` 测试验证了变体存在且数量为 13），`cancellable_states()` 正确排除了 `Committing`（测试 `committing_state_must_not_be_interrupted` 验证）。
- **但这个状态机类型在 `chimera-runtime` 中完全未被使用**——`grep "UpdateState" crates/chimera-runtime/src` 无匹配。`stage_version`/`commit_version`/`rollback_to_last_known` 都是无状态的纯函数调用，不维护、不持久化、不检查任何 `UpdateState` 值。也就是说 Spec 8.3 的状态机"定义"存在，但"实现"（真正驱动状态转换、在每个阶段做正确的事）不存在——没有代码在 `Downloading` 阶段检查暂停请求，没有代码把系统置于 `Committing` 后拒绝退出，`chimera-desktop` 前端虽然调用了 `apply_codex_update`/`repair_runtime`/`rollback_runtime` 等 IPC 命令（`apps/chimera-desktop/src/features/codex/index.tsx:101-106,311`），但这些命令**在 Tauri 后端完全没有被注册**（`apps/chimera-desktop/src-tauri/src/lib.rs:27-37` 的 `generate_handler!` 列表里只有 8 个命令：`get_system_status`、`list_providers`、`launch_codex`、`switch_provider`、`test_provider`、`list_skins`、`apply_skin`、`try_skin`、`restore_default_skin`，没有 `get_runtime_status`、`repair_runtime`、`run_diagnostics`、`rollback_runtime`、`apply_codex_update`）。前端调用这些命令会在运行时得到 Tauri "command not found" 错误。这是一个真实的、可复现的功能缺口，不是文档遗漏。
- **Fault injection 完全缺失**：Plan/Spec 明确要求的"下载到 health-check 全状态机、取消/暂停、磁盘满、断电和重启恢复"测试——搜索全仓库确认不存在任何模拟磁盘满（如写入受限文件系统、mock `ENOSPC`）、模拟断电（如在 `commit_version` 执行中途 kill 进程并重启后验证状态）、模拟下载中断的测试。25 个 `chimera-runtime` 测试全部是"正常路径调用 + 少量输入校验"，没有一个是故意在操作中途注入失败后验证恢复行为的 fault injection 测试。
- "取消/暂停"——没有任何 `pause`/`cancel`/`resume` 相关的函数或测试。

结论：这是 Task 5 里差距最大的 Step。原子指针切换和"有前一版本目录就能回滚"这两个具体机制是真实可用的，但 Spec 明确要求的完整状态机驱动、fault injection、取消暂停均不存在；更严重的是前端已经写了调用这些能力的 UI 代码，但后端命令层完全没有接上，形成了"看起来能用、实际会报错"的断层。

### Step 5.5 — 受管根进程、外部同名进程、独立 ChatGPT、MSIX Codex 和 PID/path 复核；Green 只关闭 owned runtime 进程

Status: **PARTIAL**

- `crates/chimera-runtime/src/health.rs:59-67` 的 `is_process_owned_by_runtime` 用路径前缀匹配（规范化大小写和分隔符）判断一个可执行文件路径是否位于受管根目录下。3 个测试覆盖：目录内的进程判定为 owned，目录外的判定为 not owned，`ChatGPT.exe` 位于完全不同路径时判定为 not owned（`chatgpt_exe_outside_runtime_root_is_not_owned`，直接对应 Spec 8.2"不得按名称广泛结束独立 ChatGPT"这一要求）。
- **缺口**：这个函数只接受 `exe_path: &Path` 作为输入——**没有任何代码从真实运行中的进程列表里查询可执行文件路径**。也就是说"PID/path 复核"里的"PID → path"这一步完全不存在：仓库里没有引入任何进程枚举库（`Cargo.toml` 没有 `sysinfo`、`windows` crate 的进程 API，或类似依赖），无法在真实场景下拿到"当前有哪些进程在运行、它们的 exe 路径是什么"这组信息。`is_process_owned_by_runtime` 是一个正确的纯函数，但它上游缺一个"谁来调用它、传入什么真实数据"的实现,因此无法真正做到"只关闭 owned runtime 进程"这个 Spec 行为要求——因为目前没有任何代码会真正尝试关闭任何进程。
- `check_runtime_health`（`health.rs:26-43`）只检查 exe 文件是否存在于磁盘（`exe.exists()`），不检查进程是否真的在运行、是否响应。"启动存活检查"（liveness）在 commit 信息里被作者自己标注为"open item"（见下方 diff 记录），代码注释 `health.rs:25` 也写"A deeper liveness check (launching the process) is done in integration tests"，但整个仓库搜不到这样的 integration test。

结论：owned/not-owned 的路径判定逻辑正确且有针对性测试（特别是 ChatGPT.exe 场景直接命中 Spec 明确点名的风险），但"从真实进程列表中安全地找出并关闭"这一完整行为链路缺失上游数据源，无法在真实系统上运行。

### Step 5.6 — 启动存活与最小应用健康检查、修复计划和一键回滚；完成 Task 5 聚合 A/B 安全审计

Status: **MISSING（除本文档外）**

- "启动存活"（launch and verify the process actually starts）——不存在，见 Step 5.5 分析。
- "最小应用健康检查"——`check_runtime_health` 只做文件存在性检查，不是"应用健康"检查（没有发起任何 HTTP/IPC 探测确认 Codex 进程真的可用）。
- "修复计划"——前端 `index.tsx` 有 `handleRepair` 调用 `invoke("repair_runtime")`，但如上所述该命令未在后端注册，修复计划本身在后端不存在任何实现（无 `repair` 相关函数）。
- "一键回滚"——`rollback_to_last_known` 函数本身存在且测试通过，但通过 UI 触发的路径（`rollback_runtime` IPC 命令）未接通,见 Step 5.4 分析。
- 本文档即为 Task 5 聚合审计 A 面。

## 结论

**FAIL —— 不满足 Task 5（T45）勾选标准。**

已验证真实存在并测试通过的部分：
- ManagedPortable 探测 + ownership 校验 + 路径穿越拒绝（Step 5.1 的三分之一）
- SHA-256 hash 校验 + MSIX 身份字符串比较（Step 5.2 的一部分，非真实 MSIX/Authenticode 解析）
- 目录布局 + 原子 `current.json` 切换 + 基本回滚（Step 5.3/5.4 的机制层）
- 进程 ownership 路径判定纯函数，包含 ChatGPT.exe 场景（Step 5.5 的判定逻辑层）

25/25 `cargo test -p chimera-runtime --locked` 全部通过，这一点属实,与提供的摘要一致。

但 Plan 中 Step 5.1–5.6 的大量验收标准未被满足：

| Step | 状态 | 缺失核心项 |
|---|---|---|
| 5.1 | PARTIAL | ExternalMsix/ExternalPortable 探测完全未实现；junction/symlink 无测试 |
| 5.2 | PARTIAL | 真实 MSIX manifest 解析、架构校验、真实 Authenticode、下载大小限制均缺失 |
| 5.3 | PARTIAL | MSIX 解包、文件树摘要计算缺失；backup/ 目录创建但未使用 |
| 5.4 | MISSING/PARTIAL | fault injection 全缺；UpdateState 状态机定义存在但未接入 runtime；前端调用的 5 个 IPC 命令未在后端注册 |
| 5.5 | PARTIAL | 路径判定纯函数正确，但无真实进程枚举数据源，无法真正执行"只关闭 owned 进程" |
| 5.6 | MISSING | 启动存活、真实健康检查、修复计划均未实现 |

与提示中给出的已知真实差距一致：无真实 MSIX 解包/下载实现（`stage_version` 只创建目录），无 power-loss/fault-injection 测试。本次审计还发现原提示未明确提及的两项具体、可复现的缺口：(1) 前端 `codex/index.tsx` 调用的 `get_runtime_status`、`repair_runtime`、`run_diagnostics`、`rollback_runtime`、`apply_codex_update` 五个命令在 Tauri 后端 `lib.rs` 的 `generate_handler!` 中完全未注册，运行时会直接报错；(2) `backup/` 目录被创建但从未被写入,回滚实际依赖的是"旧版本目录未被删除"这一副作用,不是 Spec 描述的"备份策略"。

不建议勾选 T45；建议记录为"目录/指针/ownership 判定这层地基契约已完成并通过单元测试，真实安装/更新/进程管理能力（依赖 Release Gate R1 的真实 Codex MSIX 工件）以及前后端 IPC 接线均未完成"。
