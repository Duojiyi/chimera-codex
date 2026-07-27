# Task 7 (T47) Audit A — Spec/行为覆盖 (v2)

**Date:** 2026-07-26
**Scope:** Step 7.1–7.4（1.x/CC Switch 只读迁移、外部管理器共存保护），`crates/chimera-migration/`（全部）、`apps/chimera-desktop/src-tauri/src/migration_cmds.rs`
**Branch:** `v2`, audited at HEAD `6d854d0` (a mid-session commit landed a frontend migration panel — see "并发编辑" note below; the Rust crate under audit was unchanged throughout)
**Auditor:** Independent A（只看 Spec/行为覆盖，不看 diff/边界/失败恢复）

## 方法

逐条对照 Plan `Task 7（T47）`、Spec §12（迁移与共存）、TODO 的 N1–N6 变更记录，检查代码是否存在、是否被真正调用、测试是否覆盖。已实际运行：

```
cargo test -p chimera-migration --locked --lib \
  --test step7_1_legacy_inventory --test step7_2_ccswitch_import \
  --test step7_3_migrate --test step7_3_url_policy --test step7_4_coexistence
# 10 + 18 + 16 + 15 + 5 + 18 = 82 passed, 0 failed

cargo test -p chimera-desktop --locked migration
# 2 passed (DTO serialization only)
```

对五处我认为是"保护"的地方做了破坏性验证（改代码 → 跑测试 → 确认变红 → 撤销 → 确认 `git diff` 对我改过的文件为空）：

1. 在 `read_legacy_inventory` 内加入一行 `fs::write(&paths.settings_path, ...)` → `read_only_source_legacy_settings_file_is_never_modified_by_a_failed_migration` 立即变红。
2. 在 `roll_back_and_report` 中注释掉 `config.restore(...)` 调用 → `a_keychain_write_failure_leaves_live_config_byte_identical_to_before` 与 `a_rollback_that_cannot_restore_config_is_reported_distinctly` 两个测试同时变红。
3. 在 `parse_profile` 中把 `contextSelection` 的原始内容拼接进 `display_name`（模拟"已弃用高级功能的载荷泄漏进已迁移字段"）→ `dropped_feature_sections_are_detected_but_their_payloads_never_reach_output` 与另一测试变红。
4. 在 `detect_coexistence` 中把"unidentifiable pid 视为冲突"的 fail-closed 逻辑改成 fail-open → 两个测试变红。
5. （旁证，非本人所为）会话期间另一个并发的 Auditor B 会话在 `crates/chimera-migration/tests/zzz_auditor_b_defeat_confirmation.rs` 中尝试 `OwnershipTakeoverConfirmation(())` 直接构造确认凭证——**编译失败**（私有字段），独立证实了 Step 7.4 的确认门在结构上无法绕过。

以上 5 处修改均已撤销；`git diff --stat -- crates/chimera-migration/ apps/chimera-desktop/src-tauri/src/migration_cmds.rs` 为空。（会话期间其他并发进程对本仓库其他文件——`crates/chimera-update/`、`crates/chimera-runtime/`、Task 8 审计稿——做了独立、与本审计无关的修改；未触碰、未验证，与本审计范围无关。）

## 并发编辑说明

本次审计过程中，仓库处于活跃并发编辑状态：审计开始时 `apps/chimera-desktop/src/features/settings/index.tsx` 尚无任何迁移相关代码（`git log` 显示该文件最后一次改动是 `19c97a4`，与迁移无关；全仓库 `grep -rl "migration" apps/chimera-desktop/src` 结果为空）。审计过程中该文件被另一进程修改并提交（`6d854d0 feat(settings): migration panel`），新增了一个 `MigrationPanel` 组件（预览列表、导入按钮、`window.confirm` 二次确认、状态提示），挂载在 Settings → Advanced 分类下。本审计以**当前 HEAD（含该提交）**为准出具结论；下文中"无迁移 UI"相关表述已按此更新状态复核。

## Step 逐条核查

### Step 7.1 — Red 覆盖 1.x settings/profile/active/key/path 的历史 schema 和损坏样本；Green 实现只读 inventory 与迁移预览

**Status: MET**

- `LegacySourcePaths`/`resolve_legacy_settings_path`（`crates/chimera-migration/src/legacy_source.rs:36-53,176-178`）镜像 1.x 真实路径 `~/.codex-session-delete/settings.json`，与 `codex-plus-core` 的目录/文件名一致（doc comment 明示来源）。
- 只读性：`read_legacy_inventory`（`legacy_source.rs:188-253`）只调用 `std::fs::read_to_string`，结构上无法写入。用破坏性验证 #1（上文）证实：一旦引入写入，端到端测试 `read_only_source_legacy_settings_file_is_never_modified_by_a_failed_migration`（`tests/step7_3_migrate.rs:563-599`）立即抓到。
- 损坏样本：`truncated_json_settings_file_is_refused_not_guessed`、`settings_file_that_is_a_json_array_is_refused_as_unknown_shape`（`tests/step7_1_legacy_inventory.rs:255-280`）验证"整份文件不可解释 → fail closed"；单个 profile 损坏只跳过并记警告（`relay_profiles_field_with_wrong_type_is_skipped_with_a_warning_not_a_crash`、`a_profile_missing_any_base_url_is_skipped_with_a_warning`）。
- 历史 schema 双支持：`baseUrl`（旧）与 `upstreamBaseUrl`（新）都能被读到（`legacy_source.rs:343-360`），Key 的三路回退（`apiKey` → `configContents` 内嵌 TOML 的 `experimental_bearer_token` → `authContents` 内嵌 JSON 的 `OPENAI_API_KEY`，`legacy_source.rs:387-404`）均有专项测试。
- N1–N6 透明化：`DroppedFeature` 枚举 + `detect_top_level_dropped_features`/`detect_auxiliary_dropped_features`（`legacy_source.rs:255-299`）覆盖协议转换、插件解锁、MCP/Skills 上下文、用户脚本、会话数据库、Watcher 六项。用破坏性验证 #3 证实"载荷绝不泄漏进已迁移字段"这条防线是真实的，不是摆设。
- **缺口（PARTIAL 级别，不影响本 Step 判为 MET，但影响 Step 7.1"预览"承诺的完整性）**：`detect_auxiliary_dropped_features` 需要调用方提供真实的 `user_scripts_config_path`/`watcher_disabled_flag_path`/`session_database_paths`（`legacy_source.rs:60-64`）。但桌面壳层唯一的调用点 `collect_candidates()`（`apps/chimera-desktop/src-tauri/src/migration_cmds.rs:231-252`）只调用 `LegacySourcePaths::new(...)`，从未调用 `.with_auxiliary(...)`——`grep -n "with_auxiliary" apps/chimera-desktop/src-tauri/src/migration_cmds.rs` 零匹配。意味着 N4（用户脚本）/N5（会话数据库）/N6（Watcher）三项"检测并告知用户"的透明化承诺在真实应用里是死代码，永远不会触发；用户能看到的 dropped-features 只有从 `settings.json` 内容直接解析出的三项（协议转换、插件解锁、MCP 上下文）。这是 crate 内部完整、测试充分，但**未被壳层接上**的具体缺口。

### Step 7.2 — Red 覆盖 CC Switch Codex provider 导入、数据库锁、未知 schema 和无 Key；Green 实现显式单次导入，不修改 CC Switch

**Status: PARTIAL（结构与逻辑 MET；对真实 CC Switch 安装不可用，NOT MET）**

- 结构/逻辑层：容器 schema（`apps.codex.{providers,current}`）解析、`providers` 既支持 map 又支持 list（`ccswitch_source.rs:149-177`）、未知 schema fail closed（`CcSwitchReadError::UnknownSchema`）、无 Key 仍导入但 `has_key=false`（`a_provider_with_no_key_anywhere_is_still_imported_with_has_key_false`）均有测试覆盖。
- "数据库锁"：`a_file_exclusively_locked_by_another_handle_is_reported_as_source_unavailable`（`tests/step7_2_ccswitch_import.rs:301-329`）用真实 Windows `share_mode(0)` 独占句柄模拟"CC Switch 正在保存"，验证了 `CcSwitchReadError::SourceUnavailable` 分支——这是一个真实的、非 mock 的操作系统级别测试，本次审计已实际运行通过。
- **严重缺口（file:line 证据，已用本仓库自身代码交叉验证）**：`ccswitch_source.rs` 的模块级 doc comment（第 11–14 行）自己承认："the outer JSON-file container is a best-effort reconstruction and should be checked against a real CC Switch install before this reader is wired into the desktop shell"。但它**已经**被接入桌面壳层：`migration_cmds.rs:218-220` 定义
  ```rust
  fn ccswitch_config_path(home: &std::path::Path) -> PathBuf {
      home.join(".cc-switch").join("config.json")
  }
  ```
  而本仓库**已经上线、已测试**的 1.x 导入器 `crates/codex-plus-core/src/ccs_import.rs:21-25` 明确写着：
  ```rust
  pub fn default_ccs_db_path() -> PathBuf {
      home_dir().join(format!(".{}-{}", "cc", "switch")).join(format!("{}-{}.db", "cc", "switch"))
  }
  ```
  即真实 CC Switch 把 Codex provider 数据存在 SQLite 文件 `~/.cc-switch/cc-switch.db` 的 `providers` 表里（`ccs_import.rs:37-41` 的 `SELECT ... FROM providers WHERE app_type = 'codex'`），**不是** JSON 文件 `~/.cc-switch/config.json`。`read_ccswitch_inventory` 在真实机器上会读不到这个文件（`ErrorKind::NotFound`），返回 `Ok(None)`——即"CC Switch 从未运行过"——即使用户机器上 CC Switch 已安装且配置了多个 Codex 供应商。这不是一个边界情况，是导入功能对任何真实 CC Switch 安装的**默认失败模式**，且失败是静默的（不报错，只是"什么都没找到"）。这正是该文件自己的 doc comment 预警、但被绕过验证直接接线的那个风险，本次审计用仓库里另一份已证实的代码印证它确实发生了。
- 结论：CC Switch 读取器的解析逻辑本身是稳固的（针对它自己假设的输入形状），但它假设的输入形状与真实 CC Switch 的存储格式不符，因此 Step 7.2 标题里"CC Switch Codex provider 导入"这一核心行为在真实用户机器上**不可用**。

### Step 7.3 — 实现迁移前 snapshot、secret 入 keychain、Provider projection、健康检查和失败全回滚；保留"恢复升级前配置"

**Status: PARTIAL**

- Snapshot-first + 原子回滚：`apply_migration`（`migrate.rs:201-393`）严格按"snapshot → keychain → provider insert → health check → 任何失败即回滚"顺序执行；`roll_back_and_report`（`migrate.rs:422-456`）逆序撤销并强制尝试 `config.restore`。破坏性验证 #2 证实：移除 `config.restore` 调用会让"失败后 live config 字节级不变"这条 stop condition 的两个测试立即变红——这是一条真实、非摆设的保护。
- Secret 入 keychain：`RedactedSecret`（`secret.rs`）保证 `Debug`/序列化都不泄漏；`KeychainSink::store`（`ports.rs:83-94`）是唯一写入路径；`secrets_never_appear_in_a_migration_error_after_a_failure`、`secrets_never_appear_in_debug_output_of_a_successful_outcome` 通过。
- URL 策略：非 HTTPS、`user:pass@`、URL fragment 均被拒绝且不写 keychain（`migrate.rs:238-286`，`tests/step7_3_url_policy.rs` 5 个测试全部通过）——doc comment 明确指出这是为了不让"迁移路径"变成"绕过 Add Provider 表单校验的后门"，逻辑合理且有测试证据。
- 幂等：`deterministic_provider_id`（`migrate.rs:136-139`）+ `ProviderSink::contains` 让重复运行不会重复插入（`re_running_a_migration_that_already_ran_does_not_duplicate_providers`）。
- **缺口 1（Spec 12.1 明确列出的四项之一被丢弃）**：Spec 原文"只迁移：供应商名称、URL、Key 引用、协议和模型。**当前活动供应商**。……"。`MigrationCandidate.is_selected`（`migrate.rs:55,74,83,107,120`）确实从 `LegacyProviderCandidate::is_active`/`CcSwitchProviderCandidate::is_current` 正确计算出来了——但 `grep -n "is_selected" crates/chimera-migration/src/migrate.rs` 显示这个字段只在赋值处出现，`apply_migration` 内部**从未读取**它：插入的 `Provider`（`migrate.rs:333-343`）不带任何"是否为当前活动供应商"的痕迹，也没有任何调用把迁移后的该 provider 切换为 live/active（chimera-provider 的"当前供应商"是由哪个 provider 被 `switch` 进 live config.toml 决定的，migrate.rs 从不调用这条路径）。且 `MigrationCandidateDto`（`migration_cmds.rs:39-47`）连"哪个是原来活动的"这个信息都没有序列化给前端。结果：迁移完成后，用户在 1.x/CC Switch 里正在用的那个供应商，在 v2 里既不会自动生效，UI 上也看不出它曾经是"当前"的——用户必须凭记忆自己再手动切换一次。这是 Spec 明文要求迁移的四项之一被完整地计算出来又完整地丢弃，不是"次要遗漏"。
- **缺口 2（"保留恢复升级前配置"不可达）**：`restore_pre_migration_configuration`（`migrate.rs:403-410`）和它依赖的 `MigrationOutcome::pre_migration_snapshot`（`migrate.rs:195`）在 crate 内部存在且有专项测试（`restore_pre_migration_configuration_is_usable_as_an_explicit_action_after_success`）。但 `migration_cmds.rs` 里 `grep -n "restore_pre_migration_configuration"` 零匹配——没有对应的 `#[tauri::command]`，`run_migration` 返回的 `MigrationResultDto`（`migration_cmds.rs:63-70`）也不包含 snapshot 或任何可用于日后调用 restore 的引用/句柄。也就是说，一次成功迁移之后，"恢复升级前配置"所需要的那份快照数据，在 `run_migration` 返回的瞬间就被丢弃了（它只是函数内部的局部变量），不是"缺一个按钮"那么简单——是数据本身已经不可获取。Spec 12.1 明确要求"成功后仍保留可识别备份"，这条在当前接线下不成立。
- 健康检查：`HealthCheckPort::check` 的真实实现 `NoNetworkHealthCheck`（`migration_cmds.rs:196-202`）永远返回 `Ok(())`，注释承认这是刻意的空实现（避免迁移时对每个 Key 发起网络请求）。事务层面"健康检查失败要触发整体回滚"这条路径是真实可用的（`a_health_check_failure_rolls_back_every_provider_and_secret_migrated_this_run` 通过），但在生产环境里这条路径永远不会被触发，因为检查本身从不失败。Spec 12.1"迁移前……健康检查"的字面要求因此只在结构上成立，行为上是空转。

### Step 7.4 — 实现已知外部 manager lock/ownership/进程检测和只读冲突 UI；迁移所有权必须显式确认

**Status: NOT MET（作为可观察的应用行为；纯逻辑层本身 MET）**

- 纯逻辑层扎实：`detect_coexistence`（`coexistence.rs:159-204`）综合三种证据（lock holder、ownership manifest owner、running process），且对"unidentifiable pid"fail closed（`holder.pid != Some(current_pid)`，把 `None` 也当冲突处理）——破坏性验证 #4 证实这是真实生效的判定，不是永真式断言。
- 确认门是结构性的，不是运行时检查：`OwnershipTakeoverConfirmation(())`（`coexistence.rs:229-238`）私有字段 + 唯一构造函数 `user_explicitly_confirmed()`，使得"不经确认就拿到所有权凭证"在编译期就不可能——本轮审计观察到的独立旁证：会话期间另一个并发的 Auditor B 会话尝试直接构造 `OwnershipTakeoverConfirmation(())` 绕过确认，**编译失败**（"private field"），与我自己的代码阅读结论一致。
- `take_ownership`（`coexistence.rs:244-261`）只在 `verdict != Clear` 时才调用 `probe.claim_ownership`，`detect_coexistence` 路径永远不会触达 `claim_ownership`（`the_default_detection_path_never_calls_claim_ownership` 通过）。
- **但整条能力在应用里完全没有被使用**：
  - `grep -rn "coexistence\|CoexistencePort\|detect_coexistence\|take_ownership"` 在 `apps/chimera-desktop/src-tauri/src/` 与 `apps/chimera-desktop/src/` 下**零匹配**（唯一命中的四个文件是 crate 自身的源码/测试）。
  - 没有任何类型 `impl CoexistencePort for ...` 存在于仓库任何地方——`grep -rn "impl.*CoexistencePort"` 只匹配到测试文件里的 `SpyProbe`（假对象）。也就是说，读取真实操作锁文件、真实 ownership manifest、真实进程列表的适配器**从未被写出来**。
  - 没有任何 `#[tauri::command]` 暴露 `detect_coexistence`/`take_ownership` 给前端；前端 `apps/chimera-desktop/src/` 全仓库搜索 "coexistence" 零匹配。
  - 已确认存在的、Chimera 自己的跨进程操作锁 `chimera_platform::lock::OperationLock`（`crates/chimera-platform/src/lock.rs`）在桌面壳层只被 `commands.rs:251-253` 的 `restore_official` 使用；`run_migration`/`preview_migration` 两个迁移命令完全不获取任何锁，也不做任何 coexistence 检查就直接写 provider 数据库和 keychain。
  - Task 7 的 stop condition 原文"……或两个管理器能并发写同一目标即停止"和 Spec 12.3"Chimera 启动时检测已知 operation lock、ownership 和进程；发现其他管理器活动操作时进入只读状态"——这两条在当前应用里都**没有对应的可观察行为**：Chimera 启动时不做任何此类检测，`run_migration` 执行前也不做。唯一存在的是一个从未被实例化、从未被调用的 trait 和一组用假对象跑过的单元测试。

## Stop Conditions 核查总表

| Stop condition | 结论 | 证据 |
|---|---|---|
| 源数据库/1.x 文件从不被写入 | **MET** | `legacy_source.rs`/`ccswitch_source.rs` 只调用 read 系 API；破坏性验证 #1 证实端到端测试真的会抓到写入。 |
| 失败的迁移使 live config 字节级不变 | **MET**（结构层）；**未在真实文件系统上端到端验证** | `migrate.rs` 的 snapshot-first + `roll_back_and_report`；破坏性验证 #2 证实。但 `RealConfig`（`migration_cmds.rs:153-188`）用真实 `std::fs::read`/`write` 实现该 port，桌面壳层层面没有任何测试针对*这个真实实现*做故障注入（唯一测试是 crate 内用 `FakeConfigStore`）——真实文件系统 I/O 失败半途（例如 rename 失败、磁盘满）时 `RealConfig::restore` 本身不是原子的（`migration_cmds.rs:171-187` 直接 `fs::write`，没有 temp-file+rename），理论上真实环境里"回滚写入"这一步本身可能半途失败，这一层未被测试覆盖。 |
| 1.x 已下线的高级功能（N1-N6）不被迁移 | **MET**（内容不泄漏这一点）；**PARTIAL**（辅助标记类三项在生产环境从不触发，见 Step 7.1 缺口） | 破坏性验证 #3；`with_auxiliary` 从未被调用。 |
| 两个管理器不能并发写同一目标 | **NOT MET（应用层）** | 见 Step 7.4：`CoexistencePort` 无真实实现、无命令、无 UI、无启动时检测；`run_migration` 不获取任何锁。 |
| 拿到所有权需要显式确认 | **MET（若被调用）**；**不可达（未被任何调用点使用）** | `OwnershipTakeoverConfirmation` 结构性防绕过（编译期），但整个 Step 7.4 模块未接入应用，所以这条保护目前保护的是一个永远不会被执行的代码路径。 |

## 未能验证的部分

- **真实 CC Switch 安装**：本次审计通过交叉引用本仓库已上线的 `ccs_import.rs`（读 SQLite）证明了 `ccswitch_source.rs` 假设的 JSON 容器与真实格式不符，但未能在装有真实 CC Switch 的机器上跑一次端到端的"预览迁移"来直接观察"检测到 0 个供应商"这一现象——没有这样的机器可用。这属于文档要求的"真实 Codex build / 真实机器"类不可验证项，但间接证据（同仓库两套导入器对同一软件的存储格式给出互相矛盾的结论）已经足够支撑 NOT MET 的判断，而不只是"未验证"。
- **Windows 实机共存矩阵**（旧 Chimera/官方 MSIX/外部 portable/Codex App Manager 同时安装）：这是 Step 7.5 的范围，本次审计范围明确限定在 7.1–7.4，未做。
- **`RealKeychain`/`RealProviders` 对真实 OS keychain / 真实 SQLite 的故障注入**（例如 keychain 服务不可用、SQLite 文件被其他进程锁定时 `run_migration` 的真实行为）：`migration_cmds.rs` 里这三个 port 实现只有 2 个 DTO 序列化测试覆盖，没有任何测试用真实 OS keychain 或真实数据库文件驱动 `run_migration` 走一遍。真实环境下的行为只能通过代码审查+crate 内假对象测试的组合来推断，不是直接验证。

## 结论

**PASS WITH GAPS — 不建议在当前状态下勾选 Step 7.1–7.4 全部通过。**

已验证真实存在、测试充分、且经受住破坏性验证的部分：
- 1.x 只读 inventory 读取、损坏样本 fail-closed、Key 三路回退、内容类 dropped-feature 检测且不泄漏载荷（Step 7.1 主体）。
- 迁移事务的 snapshot-first、字节级回滚、URL 策略、幂等去重、secret 全程不落地明文（Step 7.3 主体）。
- Coexistence 判定逻辑本身（fail-closed、确认门结构性防绕过）（Step 7.4 的库内实现）。

三个不应被淡化为"部分完成"的具体缺口：

1. **CC Switch 导入对真实安装不可用**（Step 7.2）：`migration_cmds.rs:218-220` 指向 `~/.cc-switch/config.json`，而本仓库自己的 `ccs_import.rs:21-25` 证明真实数据在 `~/.cc-switch/cc-switch.db`（SQLite）。这不是"未验证"，是"已用仓库内证据证伪"。
2. **Coexistence 保护完全未接入应用**（Step 7.4 + 对应 stop condition）：`CoexistencePort` 没有真实实现、没有 Tauri 命令、没有 UI，`run_migration` 执行前不做任何检测也不取任何锁。Task 7 明确写的"两个管理器能并发写同一目标即停止"这条 stop condition，在当前 shipped 应用里没有任何代码路径去阻止它。
3. **"当前活动供应商"被计算后丢弃，"恢复升级前配置"数据在返回时即丢失**（Step 7.3）：均为 Spec 12.1 逐条列出的迁移标的，前者字段存在但从不读取，后者数据存在但从不持久化/暴露。

TODO 自己标注 T47 为"代码完成，缺 UI 与共存实机矩阵"——本次审计确认这一自评是准确的，但也发现了三项该自评未提及的、更具体的功能性缺口（CC Switch 路径不匹配、活动供应商丢失、回滚快照不可达），且确认"缺共存"不是"缺打磨"，而是这条 stop condition 目前完全没有代码执行它。
