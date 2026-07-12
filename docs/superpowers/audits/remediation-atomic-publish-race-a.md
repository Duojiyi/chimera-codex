# Atomic Publish Race Remediation - Independent Audit A

> 日期：2026-07-12
> 结论：PASS（本地实现与静态 workflow 合约）

独立按需求与可观察行为复核：

- source identity 校验后、rename 前换入的攻击者文件不会留在正式路径。
- initial identity 错误会把未知对象移出正式路径；quarantine 验证后不再按可变路径删除。
- 发布后的并发安全替换保留；可信 inode hardlink 内容由开放句柄清空。
- pending lock 在 operation 前验证 inode/link、清除遗留内容并同步；Unix 权限保持 `0600`。
- Unix pending open 使用 `O_NOFOLLOW | O_NONBLOCK`，open 后继续按句柄拒绝非普通文件。
- PR macOS x64/arm64 matrix 均配置运行 core unit tests。

复核证据：core unit `152/152`、manager workflow contracts `35/35`、格式与 scoped diff 通过。

外部门：本快照尚未在 `macos-15-intel` 和 `macos-14` Actions 上运行。两路成功前不得宣称 macOS FIFO、mode `000`、Darwin `renamex_np` 和相关 Unix 文件语义已动态验收。
