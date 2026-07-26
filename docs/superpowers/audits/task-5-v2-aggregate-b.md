# Task 5 (T45) Audit B — Diff/边界/失败恢复 (v2)

**Date:** 2026-07-26
**Scope:** Step 5.1–5.6，commit `334b626`（+ 后续 `181beef` 接线相关部分）
**Auditor:** Independent B（只看 diff/边界/失败恢复，不看 Spec 覆盖清单）

## 方法

独立于 A 面，直接审查 diff 内容、边界条件和失败恢复路径。已实际运行：

```
cargo test -p chimera-runtime --locked   # 25 passed, 0 failed
```

逐文件读取 `crates/chimera-runtime/src/{detection,verify,update,health}.rs` 及对应 4 个测试文件，并核查 `apps/chimera-desktop/src-tauri/` 的接线状态、`crates/chimera-platform/src/lock.rs`。

## Diff 检查

- `crates/chimera-runtime/Cargo.toml` 依赖：`anyhow`、`serde`、`serde_json`、`thiserror`、`sha2`、`chimera-domain`、`chimera-platform`。核实 `chimera-platform` 依赖**在 `chimera-runtime/src/` 下完全未被使用**（`grep -rn "chimera_platform::" crates/chimera-runtime/src` 无匹配）——`OperationLock`（`chimera-platform/src/lock.rs`）本可以用来给 `commit_version`/`rollback_to_last_known` 加跨进程锁,防止并发更新竞争,但 `chimera-runtime` 完全没有引用它。这是一个真实的、可验证的死依赖 + 缺失防护的组合。
- `commit_version` 与 `rollback_to_last_known` 都是**无锁**函数，直接对文件系统做多步操作（`remove_dir_all` → `rename` → `write_current_pointer`,或 `read_current_pointer` → `write_current_pointer`)，中间没有任何互斥。

## 边界条件核查（按提示模板逐项探测,已实测代码路径）

### 1. 下载中途中断

不适用于当前代码——`stage_version` 不做下载,只 `create_dir_all`。如果未来接上真实下载,当前 `RuntimeLayout` 提供的 staging 目录天然隔离(下载失败时 staging 目录里是不完整文件,不会污染 `versions/` 或 `current.json`,因为 `commit_version` 是显式调用,不会自动运行)。**但这只是结构上的巧合防护,不是针对下载中断设计的测试**——没有任何测试模拟"下载了一半的文件在 staging 里"这个状态。

### 2. 进程在 stage 和 commit 之间死亡

用代码逐行核查 `commit_version`（`update.rs:102-131`）：

```rust
pub fn commit_version(...) -> Result<UpdatePointer, UpdateError> {
    let staged = layout.staging_dir().join(version);
    let version_dir = layout.version_dir(version);
    if version_dir.exists() {
        fs::remove_dir_all(&version_dir)?;     // (a)
    }
    fs::rename(&staged, &version_dir)?;         // (b)
    let previous = layout.read_current_pointer()...;
    let pointer = UpdatePointer { ... };
    layout.write_current_pointer(&pointer)?;     // (c)
    Ok(pointer)
}
```

三个可中断点:
- **在 (a) 和 (b) 之间死亡**（旧版本目录已删除,新版本还没 rename 过来）：下次启动后,`versions/<version>/` 既没有旧内容也没有新内容——**该版本目录彻底消失**,而 `current.json` 仍指向这个已经不存在的版本。`check_runtime_health` 会读到 `current.json` 里的 `active_version`,尝试 `find_codex_exe(&version_dir)`,目录不存在,返回 `exe_present: false`。行为上不会 panic,但也没有任何自动恢复——没有代码检测"current.json 指向的版本目录不存在"并自动触发 `rollback_to_last_known`。这是一个真实的**未覆盖的失败态**：中断点 (a)-(b) 之间的死亡会导致系统卡在"health check 失败但没有自动修复"的状态,需要外部人工介入调用 rollback。没有任何测试构造这个场景（没有测试在 `remove_dir_all` 和 `rename` 之间人为中断）。
- **在 (b) 和 (c) 之间死亡**（新版本目录已经 rename 完成,但 `current.json` 还没写)：`versions/<version>/` 存在且完整,但 `current.json` 仍指向旧版本（如果这是首次安装,`current.json` 干脆不存在)。下次启动读到的是旧指针,新版本目录变成一个孤儿目录,不会被使用也不会被清理。这比场景 (a)-(b) 安全（旧版本仍可用）,但同样没有测试覆盖,也没有清理孤儿目录的逻辑。
- **在 (c) 内部 `write_current_pointer` 的 tmp→rename 之间死亡**：`update.rs:80-90` 显示 `write_current_pointer` 用 tmp 文件写入后 `fs::rename`,这一步本身是原子的（同分区 rename 在 Windows NTFS 上是原子操作）,所以这个子步骤是安全的——**这是本次审计验证的、真正做对的边界**。但外层调用者（`commit_version`）如果在调用 `write_current_pointer` 之前死亡,如上一条分析。

结论：单个"tmp→rename"写入是原子的,但 `commit_version` 函数整体是**三个独立步骤的非原子序列**,中间没有 journal/日志记录进行中的操作,这与 Spec 8.2 明确要求的"更新期间...断电、磁盘满、杀进程或健康失败后,下次启动必须依据 transaction 恢复"直接冲突——当前没有 `transaction.json`（尽管 `update.rs:2` 的模块注释写着 "transaction.json" 作为目标布局的一部分,但源码里**没有任何代码读写这个文件**,`grep -rn "transaction.json\|transaction_state" crates/chimera-runtime/src` 只匹配到注释,没有实现)。

### 3. current.json 损坏或缺失

- **缺失**：`read_current_pointer()`（`update.rs:69-78`）在文件不存在时返回 `Ok(None)`,不报错——这是正确的优雅处理,测试 `staging_does_not_modify_current_pointer` 间接验证了这一路径（stage 后 `current.json` 确实不存在,断言通过）。
- **损坏**（存在但内容不是合法 JSON,或 JSON 合法但字段类型不对)：`read_current_pointer` 对 `serde_json::from_slice` 失败会返回 `Err(UpdateError::PointerCorrupt(...))`（`update.rs:75-77`)。**但没有任何测试构造一个真正损坏的 `current.json`**（比如写入 `"not valid json{"` 或写入合法 JSON 但缺 `active_version` 字段)后调用 `read_current_pointer` 验证返回的确实是 `PointerCorrupt` 而不是 panic。这是一个已声明但未经测试验证的错误路径——代码"看起来"处理了,但没有回归保护;如果未来有人重构 `UpdatePointer` 的字段而破坏了向后兼容的反序列化容错,不会有测试失败提醒。
- 更严重的问题：`check_runtime_health`（`health.rs:26-30`）对 `layout.read_current_pointer()?` 用 `?` 直接向上传播 `PointerCorrupt` 错误。**这意味着 `current.json` 损坏会让健康检查本身失败并返回 Err,而不是触发任何恢复流程**。在 `commands.rs:79-94` 里 `get_runtime_info` 对 `check_runtime_health` 的 `Err` 分支统一处理为"未安装"（`install_mode: "not_installed"`)——这是一个危险的静默降级：**"current.json 损坏"和"从未安装过"这两种完全不同的情况,在当前实现下对用户呈现的是同一个 UI 状态**,用户无法区分"我需要首次安装"还是"我的安装被损坏了,需要修复/回滚"。这是一个真实的可用性 + 诊断缺陷,不是纯粹的代码 bug,但直接影响 Spec 8.2 "失败始终保留当前可运行版本、诊断和一键修复入口"这一要求——诊断信息在这里被吞掉了。

### 4. 无前一版本时尝试回滚

- `rollback_to_last_known`（`update.rs:134-156`)对两种"无前一版本"情况都做了处理：`current.json` 不存在（`ok_or(NoPreviousVersion)?`,`update.rs:137`)和 `previous_version` 字段为 `None`（`update.rs:141`)。测试 `rollback_with_no_previous_version_returns_error` 覆盖了完整流程（先 `initialise()`,不 commit 任何版本,直接调用 rollback),验证返回 `UpdateError::NoPreviousVersion`,**这个边界测试是完整且正确的**。
- 但还有第三种情况未被覆盖：`previous_version` 字段有值,但**该版本对应的目录已经被外部删除**（比如用户手动清理了磁盘)。逐行看代码：`update.rs:144-147` 确实检查了 `prev_dir.exists()`,不存在则返回 `NoPreviousVersion`——**这个检查存在**,但完全没有测试覆盖这条路径（构造一个 `previous_version` 指向不存在目录的场景)。这是"代码写对了,但测试没跟上"的典型缺口,和 A 面从 Spec 覆盖角度发现的问题是同一类但视角不同（B 面看到的是:这个具体分支在 diff 里改对了,回归保护没跟上,一旦有人重构这段代码,不会有测试失败提醒）。

### 5. 两次更新竞争（race）

**完全未覆盖,且有明确证据表明当前实现在真实并发下会产生数据损坏。**

逐行分析：`commit_version` 的三个步骤（`remove_dir_all` → `rename` → `write_current_pointer`)如果被两个线程/进程同时对同一个 `version` 调用,`remove_dir_all` 和 `rename` 都不是互斥的。更危险的是**两个不同版本同时提交**的场景：线程 A 提交版本 26.721,线程 B 提交版本 26.732,两者并发执行到 `read_current_pointer()...map(|p| p.active_version)` 这一步时,都可能读到同一个"提交前"的 `current.json`,导致两者算出的 `previous_version` 相同,最后哪个线程的 `write_current_pointer` 后写入,`current.json` 就以哪个为准——这不是"两个更新谁赢",而是**两个更新都会声称自己是"上一个版本"的直接后继**,如果之后有人从其中一个版本回滚,回滚目标是错的（丢失了另一个并发提交的更新记录）。

`crates/chimera-platform/src/lock.rs` 里的 `OperationLock`（用 `fs2::FileExt::try_lock_exclusive` 实现文件锁,已核实这是真实的进程间排他锁,不是玄虚设计)本可以直接包住 `commit_version`/`rollback_to_last_known` 调用,`apps/chimera-desktop/src-tauri/src/state.rs:70-71` 甚至已经定义了 `operation_lock()` 路径方法——**但没有任何 `chimera-runtime` 函数使用它**。这不是"锁机制不存在",是"锁机制存在但没有被接到需要保护的临界区上"。这是本次审计发现的最重要的并发缺口。

### 6. ownership manifest 指向的路径不再匹配

- `detect_runtime`（`detection.rs:62-72`)在 `ownership.json` 存在但 `canonical_path` 与实际路径不一致时返回 `CanonicalPathMismatch` 错误,测试 `ownership_canonical_path_mismatch_returns_error` 验证。**这个边界测试是完整的**,与 A 面结论一致。
- 但这只覆盖"ownership 文件本身记录的路径不对"这一种情况,没有覆盖"ownership 文件路径记录正确,但文件树摘要（`file_tree_digest`)与磁盘实际内容不一致"这一更常见的完整性问题——因为如 A 面已指出,`file_tree_digest` 从未被真实计算,这个字段目前只是一个不透明字符串,无法拿来做完整性校验,`detect_runtime` 也没有尝试用它做任何比对。

## 前后端接线断层复核（独立验证,非引用 A 面）

直接读取 `apps/chimera-desktop/src-tauri/src/lib.rs:27-37` 的 `generate_handler!` 宏参数列表：

```
get_system_status, list_providers, launch_codex, switch_provider,
test_provider, list_skins, apply_skin, try_skin, restore_default_skin
```

再读取 `apps/chimera-desktop/src/features/codex/index.tsx` 里实际调用的 `invoke()` 目标：`get_runtime_status`（第 82、92 行）、`repair_runtime`（101）、`run_diagnostics`（102）、`rollback_runtime`（103、311）、`apply_codex_update`（106）。**五个命令名在后端注册列表里一个都不存在**。这不是"功能未完成"的软性缺口,是会在运行时抛出 Tauri `command not found` 异常的确定性缺陷——只要用户打开 Codex 页面,`get_runtime_status` 调用就会失败,整个页面拿不到真实状态。

补充核查提交历史：`181beef`（"wire the Tauri command layer so the app runs end to end"）commit message 自己写道:"The frontend called invoke() for 8 commands but the Tauri shell registered zero"——说明作者已经意识到过这个断层并做过一次修复尝试,但修复后仍然只注册了 9 个命令,其中不包含 Codex 运行时管理相关的 5 个命令。也就是说这个断层在 Task 5 交付之后依然存在,未被后续 commit 关闭。

## 安全/权限边界

- `write_ownership_manifest`（`detection.rs:176-178`)使用 tmp 文件 + `fs::rename` 原子写,这一步是对的。
- `is_process_owned_by_runtime` 路径前缀比较（`health.rs:59-67`)：核实了它用 `.to_lowercase()` 和替换分隔符做规范化,但**没有处理路径遍历技巧**——如果 `runtime_root` 是 `C:/rt` 而 `exe_path` 是 `C:/rt-evil/x.exe`,字符串前缀匹配 `"c:/rt-evil/x.exe".starts_with("c:/rt")` 会返回 `true`（因为 `"c:/rt-evil"` 确实以 `"c:/rt"` 开头,中间没有路径分隔符边界检查)。这是一个真实的、可用测试复现的边界 bug——没有测试覆盖这种"前缀相似但不是子目录"的情况,现有 3 个测试用的路径要么明确在目录内、要么在完全不相关的目录,没有测试这种"名字前缀重叠"的对抗性场景。

## 结论

**FAIL —— 存在未被任何测试覆盖的并发竞争、多步操作非原子性和已确认的运行时断裂。**

关键发现（供 A 面参考,独立得出）：

1. `commit_version` 是三步非原子序列,中间任一步失败/中断都会导致 `current.json` 与磁盘实际版本目录不一致,且**没有自动检测或恢复机制**——`transaction.json`（模块注释提到但源码未实现）本应用于记录进行中操作以便重启恢复,当前完全缺失。
2. `chimera-platform::OperationLock` 是真实可用的跨进程锁,`chimera-desktop` 状态层也已经定义了锁文件路径,但 `chimera-runtime` 的更新/回滚函数完全没有使用它——两次并发提交会导致 `current.json` 的 `previous_version` 链断裂,这是可推理证明的真实并发 bug,不是理论假设。
3. `current.json` 损坏时,`check_runtime_health` 的错误会被上层 `commands.rs` 静默折叠为"未安装"状态,用户无法区分"从未安装"和"安装损坏"——直接违反 Spec 对诊断可见性的要求。
4. `is_process_owned_by_runtime` 的路径前缀比较存在路径遍历风格的误判漏洞（`C:/rt-evil` 被误判为 `C:/rt` 的子目录）。
5. 前端 `codex/index.tsx` 调用的 5 个运行时管理 IPC 命令在 Tauri 后端完全未注册,这是确定性运行时错误,即便忽略所有未实现的功能深度,当前代码在集成层面已知不可用。

这些发现和 A 面独立得出的"Step 5.1-5.6 大量验收标准未满足"结论相互印证但角度不同：A 面看的是"Spec 条款是否被满足",B 面看的是"现有代码在边界、并发和失败恢复下是否安全"。两者共同指向同一个结论——25/25 单元测试通过只证明了"孤立、无并发、无中断场景下,已实现的那部分函数逻辑正确",不能作为 Task 5 完成的证据。fault injection（提示中已知差距)和跨进程锁接线（本次审计新发现)都是阻断项。
