# Task 15 Step 15.2 macOS FD Context Remediation 独立审计 B

> 日期：2026-07-13
> 背景：GitHub Actions Run `29200727538` 的 macOS arm64 `E0599`
> 范围：当前未提交的 `crates/codex-plus-core/src/update.rs` diff、cfg 平台边界、类型/错误链、回归面与测试有效性
> 独立性：未读取、询问或引用审计 A。
> 结论：**PASS**

## Diff 审计

失败行原来对 `std::io::Error` 直接调用：

```rust
error.context("failed to restore installer FD flags")
```

`anyhow::Context` 为 `Result`/`Option` 提供 `.context(...)`，并不让任意 `std::io::Error` 获得该方法，因此 macOS cfg 实际编译时产生 `E0599`。补救新增：

```rust
#[cfg(any(target_os = "macos", test))]
fn installer_fd_restore_error(error: std::io::Error) -> anyhow::Error {
    anyhow::Error::new(error).context("failed to restore installer FD flags")
}
```

调用点只从直接 `.context(...)` 改为该 helper。先用 `anyhow::Error::new` 保存 `std::io::Error` 作为底层 source，再添加同一外层 context，类型关系与错误链正确。

## 平台与行为边界

- helper 的 production cfg 覆盖 `target_os = "macos"`，同时覆盖 x86_64 与 aarch64；修复不依赖具体 Apple 架构。
- `cfg(test)` 让 helper 在当前 Windows 测试构建中实际参与类型检查和执行；非 macOS 的非测试 production 构建不会增加未使用 helper。
- macOS `launch_installer` 分支仍是唯一调用者；Windows 和 unsupported-platform 分支没有行为变化。
- `F_GETFD`、清除 `FD_CLOEXEC`、`hdiutil` 调用及恢复原 flags 的顺序均未改变。
- 错误优先级未改变：spawn 失败时仍返回 spawn error，即使恢复同时失败；只有 spawn 成功且恢复失败时返回带 context 的 restore error。
- `last_os_error()` 仍在失败的第二次 `fcntl(F_SETFD)` 后立即采集；helper 转换不会覆盖 OS error。

## 测试有效性

新增 unit test 验证：

```text
outer display = failed to restore installer FD flags
alternate chain contains = restore denied
```

完整路径测试实际执行 1/1：

```text
test update::tests::installer_fd_restore_error_preserves_context_and_source ... ok
```

该测试足以防止 context 丢失或底层错误文本被吞。非阻断缺口：测试没有对 `error.chain().nth(1)` downcast 为 `std::io::Error` 并断言 `PermissionDenied`；当前实现通过明确的 `anyhow::Error::new(error)` 保留类型，但追加 downcast 断言会更直接地锁定 source 类型。

## 回归验证

| 验证 | 结果 |
|---|---|
| `cargo test -p codex-plus-core update::tests::installer_fd_restore_error_preserves_context_and_source --lib --locked -- --exact --nocapture` | PASS，1/1 |
| `cargo test -p codex-plus-core --lib --locked` | PASS，157/157 |
| `cargo test -p codex-plus-core --test updater --locked` | PASS，53/53 |
| `cargo fmt --all -- --check` | PASS |
| `git diff --check -- crates/codex-plus-core/src/update.rs` | PASS；仅现有 LF/CRLF 转换警告 |

## 未执行项

本机仅安装 `x86_64-pc-windows-msvc` Rust target，未安装 `aarch64-apple-darwin`，因此没有在本机执行 Apple target cross-check。新增 helper 的 API/类型已通过 `cfg(test)` 编译执行；真正的 macOS arm64 cfg 编译仍应由后续 Actions run 验证。

## Gate

当前 diff 最小、平台边界正确，解决了 `std::io::Error.context` 的类型错误，同时保持 FD 恢复语义、错误优先级和底层 source。未发现阻断项。**本独立审计 B 结论为 PASS。**
