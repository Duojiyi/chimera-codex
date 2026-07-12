# Atomic Publish Race Remediation Evidence

> 日期：2026-07-11
> 范围：`settings::atomic_write_inner` 校验后、rename 前的 source-path 替换竞态；pending provider lock 持久化契约
> 状态：本地实现与静态 workflow 合约已通过独立 A/B；真实 macOS x64/arm64 Actions 待远端验收

## Red

新增测试钩子精确运行在 source identity 已验证、`fs::rename` 尚未执行的位置。钩子把可信临时文件移走，再在原路径写入攻击者字节。

命令：

`cargo test -p codex-plus-core atomic_write_never_leaves_swapped_bytes_published_after_source_verification --locked -- --nocapture`

结果：exit 1，`0 passed; 1 failed`。断言确认 `attacker-after-source-verification` 被 rename 到正式目标并残留。

首次运行曾因测试使用当前 Rust 工具链不支持的 `Result::is_err_or` 而编译失败；改为等价的 `map_or` 后重新运行，才把上述行为失败记为有效 Red。

## Green 与回归反馈

第一版修复让所有 publish identity mismatch 都进入隔离清理。新测试通过，但原有 `atomic_write_does_not_delete_a_concurrent_safe_replacement` 失败，证明该实现会误删发布后换入的并发安全文件，因此未接受。

最终实现分两阶段验证：

1. `rename` 返回后立即、在 after-publish hook 前验证目标仍是可信 inode；不匹配时按观察到的 identity 隔离清理。
2. hook 后再次验证；此时不同 inode 视为发布后的并发替换，只返回错误，不删除替换文件。可信 inode 新增 hardlink 等不安全状态仍隔离清理。

针对性命令：

`cargo test -p codex-plus-core atomic_write_ --locked -- --nocapture`

首轮结果：exit 0，`9 passed; 0 failed`。新竞态、hardlink、symlink、秘密清理、并发安全替换和 cleanup 后再次替换均通过。

独立审计随后新增两条 Red：

- 初次发布 identity 返回 `PermissionDenied` 时，攻击者字节仍留在正式路径：exit 1，`0 passed; 1 failed`。
- quarantine identity 验证后、旧删除动作前换入安全文件时，安全文件被误删：exit 1，`0 passed; 1 failed`。

第二轮 Green：初次 identity 无法证明时直接把对象移出正式路径；quarantine 不再按已验证后可能变化的路径删除，而是保留在随机隔离名。Unix path identity 改用 `symlink_metadata`，不读取内容，新增 mode `000` 与 FIFO 非阻塞测试（在 Unix runner 执行）；Windows 仅请求 `FILE_READ_ATTRIBUTES`。当前 Windows 原子相关过滤回归为 `13 passed; 0 failed`。

完整 core unit：

`cargo test -p codex-plus-core --lib --locked`

最终结果：exit 0，`152 passed; 0 failed`。

## Persistent Lock Contract

固定 `.lock` 文件不在 unlock 后删除。删除并重建会让等待者继续锁旧 inode、后来者锁新 inode，形成两个锁域并破坏互斥。

`pending_provider_import_round_trips_and_clears` 先创建包含 17 字节遗留内容、Unix `0644` 的 lock。有效 Red 为 `left: 17 / right: 0`；Green 在取得排他锁并验证路径 identity 后 truncate + sync，pending JSON 清除后 lock 仍为普通零字节文件，Unix 权限为 `0600`。

`pending_lock_rejects_a_path_replaced_after_open` 在 lock open 后把路径移走并重建。Red 中 operation 仍执行；Green 在排他锁后重新 no-follow 打开当前路径，比较两个句柄的卷/文件 ID 与 link count，不一致时 operation 不运行。

命令：

`cargo test -p codex-plus-core pending_provider_import_round_trips_and_clears --locked`

结果：exit 0，`1 passed; 0 failed`。

split-lock 针对性命令：

`cargo test -p codex-plus-core pending_lock_rejects_a_path_replaced_after_open --locked`

结果：exit 0，`1 passed; 0 failed`。

文件锁用于协调正常 Chimera++ 进程，并检测 open→lock 窗口的路径替换。拥有同一 OS 用户任意文件写权限的恶意进程不在该互斥威胁模型内，因为它无需绕过 lock 即可直接修改 pending、settings 或 auth 文件。

第二轮审计发现 pending JSON 的 Unix no-follow read 会在 FIFO 上先阻塞、后检查文件类型。Red 在 source contract 中要求 `O_NONBLOCK`，当前实现缺失时 exit 1。Green 为 Unix open 同时设置 `O_NOFOLLOW | O_NONBLOCK`，仍由 open handle metadata 拒绝非普通文件；新增 Unix `mkfifo` 用例要求一秒内返回错误。Windows source contract targeted test exit 0；真实 FIFO 用例由 macOS runner 执行。

## Cross-Platform PR Gate

Red 在 `windows_subsystem.rs` 要求 PR 的 `macos-dmg` matrix 包含：

`cargo test -p codex-plus-core --lib --locked`

旧 workflow targeted test exit 1。Green 在 matrix Rust setup 后增加该 step，Intel 与 Apple Silicon runner 都会执行。当前 workflow contract targeted test exit 0；完整 `windows_subsystem` 为 `35 passed; 0 failed`。

## Static Gates

- `cargo fmt --all -- --check`：exit 0
- `git diff --check -- crates/codex-plus-core/src/settings.rs crates/codex-plus-core/src/provider_import.rs`：exit 0（仅换行策略 warning）
- `cargo test -p codex-plus-manager --test windows_subsystem --locked`：exit 0，`35 passed; 0 failed`

## Remaining External Verification

钩子回归已在当前 Windows 工具链确定性执行，PR workflow 已要求 macOS x64/arm64 运行 core unit。真实 macOS Actions 尚未发生；在两个目标均通过前，不把跨平台竞态验收标为完成。Linux/Android/BSD 不属于当前发行目标。
