# Task 11 Step 11.4 Installer Remediation Audit A

## Independent requirements and behavior audit

结论：**PASS**。

本轮从当前最新工作树独立核验 Windows NSIS 安装/卸载事务、Rust install/uninstall/repair、跨路径互斥、URL protocol 保守清理、legacy/compat 入口、ARP、法律文件和对应 TDD 测试。未发现需求级阻断。

### 审计结果

- 安装期 legacy/compat 快捷方式已形成完整三段事务。NSIS 在 `333-347` 对当前、legacy、乱码和兼容入口逐项 `BackupShortcut`，在 `405-414` 以 fail-closed `DeleteInstallShortcut` 删除，并在 `461-475` 使用相同 path/slot 成对恢复。测试逐项枚举 10 个历史/兼容路径，要求 backup/delete/rollback 三者同时存在。
- legacy ARP 不再伪装成值级可回滚对象。安装不再备份或恢复 `${LEGACY_UNINSTALL_KEY}`；整键删除移动到程序、法律文件、快捷方式、当前 ARP 和所有 `.bak` 清理完成之后的最终步骤（`447-455`）。删除失败明确停止并保留已成功更新的当前安装，不再丢失未知 legacy 值、类型或子键后尝试不完整恢复。
- Rust runtime 注册表快照已保留原始类型和字节。Windows integration 通过 `RegQueryInfoKeyW` 获取真实容量，逐项 `RegEnumValueW`，任何枚举错误均向上传播；快照保存 `name/value_type/data`，恢复通过 `REG_VALUE_TYPE(value_type)` 原样写回。
- Windows-only 行为测试真实创建 `REG_EXPAND_SZ` 和快捷方式字节，修改后执行 snapshot restore，并断言类型、原始 bytes 和快捷方式内容完全恢复。
- `codex-plus-core --lib` 当前可编译，mutex 并发测试和原始 registry round-trip 测试均实际执行通过。

### NSIS 安装与卸载

- `DeleteInstallShortcut`、`UninstallShortcut`、`UninstallFile` 和 `UninstallRegKey` 都在操作前清错，并在失败时跳转明确失败路径；不存在吞掉快捷方式或注册表删除错误的旧逻辑。
- 卸载在删除程序文件前完成当前、legacy、乱码、兼容及 Chimera 快捷方式和已知注册表值的备份。程序文件删除失败时尚未触碰入口和注册表，保留可重试恢复入口。
- 卸载快捷方式删除失败、URL protocol 删除失败、ARP 值删除失败或卸载器自删失败时统一进入 metadata rollback；15 个快捷方式路径均有备份和恢复，已删除的注册表值按备份恢复，恢复失败另行报告。
- URL protocol 只删除本程序拥有的 command 默认值、根 `URL Protocol` 和根默认值，随后按 `command -> open -> shell -> root` 使用 `DeleteRegKey /ifempty` 清理。NSIS 的 `/ifempty` 要求键同时没有子键和值；未知值、未知类型或未知子键存在时不会被整键删除，而是 fail closed 进入 metadata rollback，恢复三个 owned 值并保留未知状态。
- 安装器与卸载器共享 `Local\ChimeraPlusPlus.Setup.Transaction` mutex，并在 `.onInit` / `un.onInit` 使用 `CreateMutexW(..., initialOwner=1, ...)` 立即取得所有权。若 Rust 已创建同名对象，NSIS 看到 `ERROR_ALREADY_EXISTS` 后在进入固定 `.new/.bak` 事务前停止。
- `LICENSE`、`NOTICE`、`SOURCE_CODE.txt` 与三个二进制共享 staging/backup/commit/rollback。卸载对当前、`.new`、`.bak` 法律文件逐项 fail closed；卸载 mutex 在 Uninstall section 前取得。

### Rust runtime

- `install_shortcuts` 和 `uninstall_shortcuts` 在计算 plan、capture 快照之前取得同名 `Local\ChimeraPlusPlus.Setup.Transaction` mutex；guard 的 Drop 在函数退出时 release/close，因此锁覆盖 capture、apply、成功返回以及失败 rollback。repair 复用 `platform_install`，进入同一 install 锁路径。
- Rust 使用 `CreateMutexW(initialOwner=false)` 后 `WaitForSingleObject(INFINITE)` 获取所有权。NSIS 以 `initialOwner=1` 持锁时 Rust 会阻塞；Rust 持锁时 NSIS 因同名对象已存在而停止，双向串行成立。Rust 接受 `WAIT_ABANDONED` 并接管前一异常退出者释放的 mutex。
- `install_shortcuts` / `uninstall_shortcuts` 在锁内同时捕获受管快捷方式字节和六个受管注册表层级；删除顺序为子键到父键，恢复顺序为父键到子键。
- 原本不存在的键通过 restore 前的统一深到浅删除保持不存在；原本存在的空键通过 `ensure_current_user_key` 恢复；所有原始值按 type+bytes 写回。
- apply 错误不会被吞掉；rollback 失败时错误同时包含 apply 与 rollback 原因。注册表删除只忽略 `ERROR_FILE_NOT_FOUND` / `ERROR_PATH_NOT_FOUND`。

### Targeted results

- `cargo test -p codex-plus-core --test installers --locked`：23/23 PASS。
- `cargo test -p codex-plus-core --test installers windows_nsi_acquires_local_named_mutex_before_using_fixed_staging_files --locked -- --exact`：1/1 PASS，包含两处 `initialOwner=1` 契约。
- `cargo test -p codex-plus-core --lib install::windows::tests::setup_transaction_mutex_serializes_concurrent_metadata_changes --locked -- --exact`：1/1 PASS；第二线程在首个 guard 释放前无法取得同名 mutex，释放后可进入。
- `cargo test -p codex-plus-core --test installers windows_uninstall_keeps_recovery_entries_when_program_files_cannot_be_removed --locked -- --exact`：1/1 PASS。
- `cargo test -p codex-plus-core --test installers windows_install_shortcut_cleanup_is_fail_closed --locked -- --exact`：1/1 PASS。
- `cargo test -p codex-plus-core --test installers windows_legal_files_share_the_binary_transaction_and_uninstall_mutex --locked -- --exact`：1/1 PASS。
- `cargo test -p codex-plus-core --lib install::windows::tests::metadata_snapshot_restores_shortcut_bytes_and_registry_value_types --locked -- --exact`：1/1 PASS。
- scoped `git diff --check`：PASS；仅现有 LF/CRLF 转换提示，无 whitespace error。

### 静态限制

- 本机 `Get-Command makensis` 无结果，因此未执行 NSIS 编译、安装/覆盖升级、文件占用故障注入或卸载实机冒烟。
- 本结论证明当前源码契约、Windows mutex/registry 行为和针对性测试通过，不替代后续 Release CI 的 NSIS 编译、NSIS 与 Rust 跨进程实测以及 Task 16 Windows 实机验收。

安装器修复包在本独立审计 A 范围内可以关闭。
