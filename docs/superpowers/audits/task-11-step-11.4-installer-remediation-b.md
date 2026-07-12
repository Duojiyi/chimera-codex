# Task 11 Step 11.4 安装器修复包独立审计 B

## 结论

**PASS**

最新修复已关闭此前阻断项：Rust runtime install/uninstall 在 capture 之前取得与 NSIS 相同的 Windows named mutex，并持锁覆盖 apply 与 rollback；NSIS uninstall 只删除本程序拥有的 URL protocol 值，再以 `/ifempty` 从子到父清理空键，因此未知值和未知子键不会被递归删除。当前 diff、生产路径和针对性测试中未发现阻止 Step 11.4 通过的行为缺口。

本审计仅依据当前工作树、生产路径、边界条件和相关测试独立完成，未读取或引用审计 A，也未修改实现代码。

## 阻断项复核

### B1. Runtime 与 NSIS 跨路径事务隔离：已关闭

- `scripts/installer/windows/CodexPlusPlus.nsi:104` 与 `:124` 的 installer/uninstaller 都以 `CreateMutexW(p 0, i 1, ...)` 创建 `Local\ChimeraPlusPlus.Setup.Transaction`。`initialOwner=1` 使首次创建者立即持有 mutex；已存在时按 `ERROR_ALREADY_EXISTS (183)` 中止。
- `crates/codex-plus-core/src/install/windows.rs:16` 使用同一 mutex 名称；`:32-60` 创建 handle 后通过 `WaitForSingleObject(..., INFINITE)` 等待所有权，并将 `WAIT_OBJECT_0` 与 `WAIT_ABANDONED` 视为已取得所有权。
- RAII guard 在 Drop 中 `ReleaseMutex` 并 `CloseHandle`；等待失败或未知状态会关闭 handle 并返回错误，不会继续进入事务。
- runtime install/uninstall 分别在 `:145`、`:172` 获取 guard，均早于 plan、shortcut/registry snapshot，因此锁覆盖 capture、apply 以及失败 rollback 的完整周期。
- 交叉路径语义闭合：NSIS 先运行时 Rust 等待；Rust 先运行时 NSIS 看到现有 mutex 后中止；Rust/Rust 由 wait 串行化；NSIS/NSIS 的后启动进程中止。
- `install/windows.rs:474-494` 的 Windows 测试验证第二个 runtime contender 在首个 guard 释放前不能进入，释放后可以继续。
- `crates/codex-plus-core/tests/installers.rs:703`、`:785` 明确约束 installer 与 uninstaller 必须使用 `initialOwner=1`，防止退回“创建但未持有”的错误互斥语义。

**判定：关闭。** named mutex 已覆盖 runtime 与 NSIS 的共享事务面，并在线性化点之前取得。

### B2. NSIS URL protocol 未知数据保护：已关闭

- `scripts/installer/windows/CodexPlusPlus.nsi:689-691` 只删除已备份的三个 owned 值：command 默认值、root 的 `URL Protocol`、root 默认值。
- `:692-695` 再按 `command -> open -> shell -> root` 调用 `UninstallRegKey`；该宏在 `:142-146` 使用 `DeleteRegKey /ifempty`，不会递归删除含未知值或未知子键的 key。
- 因此未知 `REG_DWORD`、`REG_BINARY`、`REG_EXPAND_SZ`、扩展值和未知子键始终留在注册表中；它们不依赖 rollback 重建。
- 后续步骤失败时，rollback 只恢复 owned 已知值是充分的：未知数据从未被删除，空键清理也只会在确实为空时发生。
- `installers.rs:857-895` 约束 `/ifempty` 清理、owned URL 值删除和由子到父的 key 顺序，并禁止无条件 `DeleteRegKey`。

**判定：关闭。** 正常卸载仅移除本程序拥有的数据，失败 rollback 不再丢失未知注册表内容。

## 其他事务面复核

- NSIS install 对 current、legacy、mojibake 与 compat 快捷方式均具有匹配的 backup/delete/rollback slot；`.lnk` 以原始文件复制方式恢复。
- legacy ARP install 清理位于所有可 rollback 步骤之后；current ARP 已成功建立后才提交 legacy key 删除。
- Rust snapshot 保存快捷方式原始字节及注册表 value type/raw bytes；registry 删除按子到父，恢复按父到子。
- `run_metadata_transaction` 在 apply 失败时执行 rollback，rollback 再失败时同时保留 apply 与 rollback 错误。
- NSIS uninstall 在 mutation 前备份 owned metadata；程序文件、法律文件和 metadata 删除均 fail-closed。`uninstall.exe` 最后删除，检测到前置失败时保留恢复入口。
- `LICENSE`、`NOTICE`、`SOURCE_CODE.txt` 与二进制共享 staging、backup、rollback 和卸载失败处理。
- 事务错误只包含操作、路径、subkey/value name 等定位信息，未拼接注册表 value data、快捷方式内容或凭据。

## 测试结果

- `cargo test -p codex-plus-core --lib install::windows::tests --locked`：**PASS**，4 passed、0 failed、152 filtered out。覆盖 apply failure rollback、rollback failure 双错误、快捷方式/注册表原始值恢复及 runtime mutex 并发序列化。
- `cargo test -p codex-plus-core --test installers --locked`：**PASS**，23 passed、0 failed。覆盖 NSIS mutex ownership、快捷方式事务、法律文件、URL owned-value 与 `/ifempty` 清理、ARP 和卸载器顺序。
- 本机 `Get-Command makensis` 返回 `MISSING`：未安装 NSIS 编译器，未执行 `.nsi` 语法编译或真实 installer fault-injection。
- 未运行 workspace 全套测试。

## 剩余风险

- 缺少真实 NSIS 编译和 Windows installer fault-injection；当前结论依赖 Rust 行为测试与 NSIS 静态契约测试。
- Rust 在 `WAIT_FAILED` 分支先关闭 handle，再调用 `Error::from_win32()`；理论上 `CloseHandle` 可能改写线程 last-error，影响诊断准确性。该路径仍然 fail-closed，不会在未取得锁时执行事务，因此不构成本步骤阻断。
- NSIS `RestoreRegValue` 对原本不存在的 key 只删除新写入值，不删除新建空 key；失败安装后可能留下空 current ARP/InstallDir key。空 key 不产生控制面板入口，但不是结构级完全等价。
- Rust raw snapshot 的 value name 通过 lossy UTF-16 转换存入 `String`；极端无效 UTF-16 注册表值名不能严格 roundtrip。现有测试覆盖 value type 与 data，不覆盖该名称边界。

## 最终判定

两项原阻断均已在生产实现和针对性测试中关闭，未发现新的阻断性回归。考虑上述非阻断测试缺口与边界风险后，Step 11.4 安装器修复包独立审计 B 判定为 **PASS**。
