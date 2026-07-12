# Remediation Updater Audit A - 需求与可观察行为

> Status: **PASS**
> Audit date: 2026-07-10
> Recorded: 2026-07-11
> Scope: trusted manifest、下载发布与安装器启动保护

## 独立审计 A

- manager IPC 仅接收请求版本；后端重新获取受信 `latest.json` 并绑定完整 SemVer。
- 平台、架构、文件名、size 与 SHA-256 在下载及启动前复核。
- manifest 与资产响应具有大小上限和超时。
- 独占下载锁与专属 `.part` 防止并发请求互相发布字节。
- hard-link/no-clobber 与跨文件系统 fallback 都不会覆盖竞态胜者。
- 下载失败不删除已有完整安装包，stale part 清理仅处理本产品拥有的文件。
- symlink、reparse point 与非 UTF-8 lookalike 均有负向覆盖。

## 验证证据

- updater 集成测试：35/35。
- publisher/launch guard：5/5。
- manager trusted resolver、IPC 与平台对象绑定测试通过。
- `git diff --check`：通过。

## 结论

PASS。无代码阻断项；macOS `hdiutil`、Linux发布原语与真实安装器启动仍需对应平台 CI/实机验证。
