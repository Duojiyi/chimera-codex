# Remediation Installer Audit A - 安装、升级与卸载行为

> Status: **PASS**
> Audit date: 2026-07-10
> Recorded: 2026-07-11
> Scope: Windows 事务安装/卸载与 macOS 双架构打包

## 独立审计 A

- Windows silent、manager、uninstaller 使用 staging/backup/commit/rollback，失败时恢复原文件。
- 快捷方式与卸载注册表逐项 fail-fast；删除或恢复后回读验证。
- 卸载器保留到程序文件、快捷方式和注册表清理成功之后，部分失败可重试。
- `.bak` 清理失败显式中止，避免阻断后续升级。
- `UninstallString` 保持正确引用，`InstallDir` 保持 legacy 兼容路径。
- macOS 拒绝已有输出和 symlink，归一化架构并在创建产物前校验两个二进制。
- Release 继续明确 ad-hoc sign、未 notarize。

## 验证证据

- installer tests：18/18。
- `generate-branding.ps1 -Check`：PASS。
- `git diff --check`：PASS。

## 结论

PASS。代码与静态行为无阻断项；本机缺少 `makensis`，未执行 NSIS 编译或 Windows/macOS 实机安装。
