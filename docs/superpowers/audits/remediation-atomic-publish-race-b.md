# Atomic Publish Race Remediation - Independent Audit B

> 日期：2026-07-12
> 结论：PASS（本地代码与静态 workflow 合约）；跨平台动态验收待远端

独立按 diff、竞态和边界复核：

- rename 前后、PermissionDenied/未知 identity、hardlink、quarantine verify 后替换和 no-replace restore 未发现剩余误发布或误删。
- Unix FIFO 读取不会在 fstat 前阻塞；pending reader 与 lock identity 复核复用同一 nonblocking/no-follow helper。
- split-lock 的 open→lock 窗口由双句柄 identity/link 校验阻断；同 OS 用户任意文件写权限明确不属于协作锁威胁模型。
- `O_NONBLOCK` 不会把普通 lock 文件的 `lock_exclusive` 改为 try-lock。
- macOS matrix 的两种架构都包含 `cargo test -p codex-plus-core --lib --locked`。

独立 targeted：provider source contract `1/1`、macOS workflow contract `1/1`、scoped diff-check 通过。

外部门：真实 macOS x64/arm64 Actions 均通过后才能关闭跨平台动态验收。FIFO 测试若未来回归为真实阻塞，会依赖 CI job timeout；source contract 可提前捕获删除 `O_NONBLOCK`，后续可再升级为子进程级硬超时测试。
