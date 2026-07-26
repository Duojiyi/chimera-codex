# Task 1 (T41) Audit B — Diff/边界/失败恢复

**Date:** 2026-07-26
**Scope:** Step 1.1–1.5（新 workspace 与领域骨架）
**Commit:** `309ad81` feat(task-1): new v2 workspace skeleton
**Auditor:** Independent B（只看 diff/边界/失败恢复，不参考 Audit A 的结论）

方法：读实际 diff 内容，对每个新增模块问"malformed input 会怎样"、"并发访问会怎样"、"文件缺失会怎样"、"锁竞争会怎样"，并核实是否有测试覆盖对应路径；无测试覆盖的记为 gap。

## chimera-domain — 边界与失败路径

- **序列化边界**：`Provider.base_url` 用自定义 `url_serde` 模块反序列化，`Url::parse` 失败时通过 `serde::de::Error::custom` 传播，不会 panic。但**没有任何测试验证畸形 JSON（如 `base_url: "not a url"`）时的反序列化失败路径**——13 个测试全部走的是"构造合法值 → 序列化 → 反序列化 → 断言相等"的 happy path，没有一个负向测试。
- **枚举穷尽性**：`ProviderHealth`/`UpdateState`/`TransactionState` 用 `#[serde(tag = "state", rename_all = "snake_case")]`，未知 tag 值反序列化会返回 `Err`（serde 默认行为），但同样无测试覆盖"收到未知 state 字符串"这一失败路径。
- **OperationError 是纯数据，没有状态转换逻辑**，因此本 crate 本身没有可测的"失败恢复"——它只是错误信息的容器。合理，不算 gap。
- `UpdateState::is_active()` / `cancellable_states()` 是唯一的业务规则；`committing_state_must_not_be_interrupted` 测试覆盖了这条规则的核心断言（Committing 不可取消）。**但 `is_active()` 本身没有测试**——如果未来有人误改 `is_active` 使其漏掉 `Committing`，不会被测试捕获。Gap，低风险（P2）。

## chimera-platform — 边界与失败路径

- **CanonicalPath 路径穿越**：`canonical_path_rejects_path_traversal` 测试用例是 `"../../../etc/passwd"`——这是一个**相对路径**，测试断言失败的原因可能是"不是绝对路径"（`NotAbsolute`）而不是"检测到 `..`"（`Traversal`）。读源码 `new()` 的逻辑：先扫描 `..` component（会命中 `Traversal`），再检查绝对路径。因为该测试路径确实含 `..` component，会先触发 `Traversal` 分支，结论仍是"通过"，但测试断言只写了 `is_err()`，**没有断言具体是哪个错误变体**，无法区分"路径穿越检测生效"还是"碰巧因为不是绝对路径而失败"。混合了两个独立边界条件，测试意图不清晰。Gap（P2，测试精度不足，不是功能缺陷）。
- **真实 symlink/junction 穿越未测试**：源码注释自陈"实际 symlink 解析在 platform-specific adapter 中进行"，即 `CanonicalPath` 只做字符串层面的 `..` 检查，不做 `fs::canonicalize`。Spec 8.1 要求"junction/symlink"边界防护，当前 Step 1.3 交付的只是最基础的字符串检查，真正的 symlink 解析留给了后续 Task（在 `chimera-runtime/src/detection.rs` 中找到了 `std::fs::canonicalize` 调用，属于 Task 5 范围）。这是设计上的合理分层，不是 Task 1 的缺陷，但如果只读 Task 1 的交付物，"路径穿越防护"给人的印象比实际防护范围更强。
- **OperationLock 并发与失败**：
  - 同进程内二次获取：`operation_lock_second_acquire_fails_while_held` 覆盖 ✓。
  - 锁释放后重新获取：`operation_lock_is_released_on_guard_drop` 覆盖 ✓。
  - **跨进程竞争未测试**——所有测试都在同一进程内用同一个 `OperationLock` 实例竞争，这只验证了 `fs2` 文件锁 API 被正确调用，没有验证真实跨进程场景（spawn 子进程持锁、主进程尝试获取）。fs2 的 `try_lock_exclusive` 语义本身是跨进程的，但没有集成测试证明这一点在 Windows 上真的生效。Gap（P1——这是 Spec V2-R4"跨进程锁"的核心承诺，Task 1 只验证了 API 存在，没有验证跨进程行为）。
  - **锁文件损坏/权限失败**：`try_acquire` 中 `OpenOptions::new().create(true).write(true).read(true).open()` 失败会映射到 `LockError::Io`，但没有测试模拟"锁文件所在目录不可写"或"锁文件被占用为只读"的场景。Gap（P2）。
  - **holder_pid 解析的脆弱性**：`parse_pid` 是手写的字符串查找（`find("\"pid\":")` + 手动 split），不是真正的 JSON 解析。如果锁文件内容被其他进程以稍微不同的格式写入（例如字段顺序不同、多余空格），`parse_pid` 会静默返回 `None` 而不报错——这是一个 fail-soft 设计（不影响锁本身的正确性，只影响诊断信息的准确性），可接受，但值得记录。
- **LockGuard::path() 未被任何测试使用**——`pub fn path(&self)` 是为诊断准备的公开 API，当前无测试验证其返回值正确。Gap（P3，低风险，纯 getter）。

## 前端骨架 — 边界与失败路径

- Step 1.4 的占位组件（`309ad81` 时点）没有任何状态、没有任何输入处理，因此没有边界条件可言——这是合理的，占位阶段不应该有失败路径需要覆盖。
- `.dependency-cruiser.cjs` 的三条规则本身有一个**未被验证的假设**：`no-cross-feature-imports` 用了 `pathNot: "^src/features/$1/"` 这种反向引用语法，这依赖 dependency-cruiser 自己支持 `$1` 捕获组替换到 `pathNot`。因为这个配置文件从未被实际执行（见 Audit A 的发现），**这条规则語法是否真的按预期工作完全未经验证**。如果语法有误，规则会静默变成"总是不匹配"或在解析阶段报错，两种情况现在都不会被发现。这是一个"看似有防护、实际未验证"的边界风险。Gap（P1，因为这是 Spec V2-R18 明确要求的自动拒绝机制，而当前完全没有执行证据）。

## verify-v2-architecture.mjs 本身的失败路径

- 脚本用 `try { statSync } catch { /* skip */ }` 吞掉所有文件系统错误后继续——对目录快速变化的场景（不太可能发生在 CI）是合理的防御性写法，不是问题。
- **Check 3（legacy crate 增长检测）是一个空检查**：只要 `codex-plus-core/src` 目录存在就直接 `pass()`，不做任何实际的文件数量比较或 git diff。这意味着**这条门禁永远不会失败**，无论 1.x crate 里加了多少新文件。对 Plan Step 1.1 明确写的"cargo metadata fixture 拒绝... 非法 crate 边"而言，这是一个静默失效的边界检查——不是运行时错误，而是逻辑上从未生效的防护。这是本次审计发现的最严重问题：**门禁脚本本身给出绿色但没有真正检测目标条件**。

## Journal/事务/回滚

Step 1.1–1.5 范围内没有引入任何持久化事务或 journal（那是 Task 2/5 的范围），因此本轮没有"崩溃恢复"类失败路径需要验证。合理。

## 结论

**PASS WITH GAPS。**

domain/platform 两个 crate 的核心类型和锁原语本身工作正确（20 个测试全绿，已重新执行确认），也没有发现会导致数据损坏或安全绕过的缺陷。但存在两类值得记录的问题：

1. **架构门禁的验证深度不足**：`verify-v2-architecture.mjs` 的 Check 2/Check 3 是字符串匹配和永远通过的占位检查，不是 Plan 承诺的 `cargo metadata` 解析；`.dependency-cruiser.cjs` 从未被任何脚本或 CI 实际调用执行。当前"V15 PASS"的绿色结果不能证明 Plan 描述的防护手段真的生效。
2. **跨进程锁缺少真正的跨进程测试**：7 个 platform 测试都是同进程竞争，Spec V2-R4 承诺的"跨进程锁"这一属性依赖 fs2/OS 语义，没有集成测试直接验证。

两者都是"验证方法弱于承诺"的问题，不是已发现的功能性 bug。建议：(a) 让 verify-v2-architecture.mjs 真正调用 `cargo metadata --locked --format-version 1` 并解析依赖图，或用 `npx depcruise` 真正执行 `.dependency-cruiser.cjs`；(b) 补一个 spawn 子进程的跨进程锁集成测试；(c) CanonicalPath 的路径穿越测试应拆分为"相对路径"和"绝对路径含 `..`"两个独立断言。
