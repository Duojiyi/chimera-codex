# Remediation Updater Audit B - 平台实现与竞态边界

> Status: **PASS**
> Audit date: 2026-07-10
> Recorded: 2026-07-11
> Scope: Windows/macOS/Linux no-clobber 与启动对象绑定

## 独立审计 B

- macOS 以 Darwin `O_NOFOLLOW` 拒绝 symlink，并把已验证的同一文件描述符交给 `hdiutil`。
- Linux 使用 `renameat2(..., RENAME_NOREPLACE)`；错误路径不会退化为覆盖发布。
- Windows guard 使用 reparse-point-aware 打开方式，并阻止验证后的写、改名和删除。
- 同一 Windows 只读 guard 不阻止安装器进程正常启动。
- manager 不信任 renderer 提供的 URL、hash、size 或文件名。
- 启动前再次验证 regular file、size、hash 与平台对象，避免下载后替换。

## 验证证据

- updater：35/35。
- core publisher/Windows launch guard：5/5。
- manager trusted resolver 与平台实现静态回归通过。
- `git diff --check`：通过。

## 结论

PASS。残余风险为 macOS 短暂 FD 继承窗口、老 glibc 兼容性及同权限进程预先持有可写句柄；当前均不构成发布阻断，需由平台 CI 和实机冒烟继续覆盖。
