# Task 11 Step 11.3 Audit A

## Final clean independent review

结论：**PASS**。

本轮从当前最终快照重新按需求、许可、TDD 证据和可观察交付行为核验，未发现阻断项。

### Windows UninstallFile fail-closed

- `UninstallFile` 宏只在目标不存在时跳过；目标存在时按 `IfFileExists → ClearErrors → Delete → IfErrors uninstall_failed` 顺序执行，删除失败会停止卸载并保留恢复入口。
- `windows_legal_files_share_the_binary_transaction_and_uninstall_mutex` 已提取 `!macro UninstallFile PATH SLOT ... !macroend` 宏体，过滤空行和 NSIS 注释，以 active exact line 查找四条命令并断言严格顺序。
- 该测试在内存中只把宏内 `IfErrors uninstall_failed` 改为注释；先断言 mutation 确实改变 fixture，再断言 fail-closed 合约返回 false。注释伪通过和 mutation 未命中均会使测试失败。
- LICENSE、NOTICE、SOURCE_CODE.txt 的 `.new` 与 `.bak` 清理均调用该宏；法律文件继续与二进制共享安装事务和 setup/uninstall mutex。

### Active-line 与发布门

- `verify-license.ps1` 的 `Assert-ActiveLine` 按 Trim 后整行精确匹配 `verify_draft_assets_content`，注释行不会命中；SelfTest 将唯一 active 调用改为注释并要求验证失败。
- Rust 发布契约测试也只在 upload 到 publish 的窗口内寻找 active exact line，不能由函数定义、普通 substring 或注释替代。
- Release gates、Windows/macOS builds 和 publish job 绑定同一 `TARGET_SHA`；build/publish 依赖许可与测试 gates。公开前校验准确资产集合及远端 digest/size，已发布幂等路径重新下载并验证对应源码。

### 许可与对应源

- `a0506ae` 基线 Cargo metadata 为 MIT 且无根 LICENSE/NOTICE，`7f72aec` 后续新增 AGPL；NOTICE 的 v1.2.34/c1360294、a0506ae、7f72aec 时间线与 Git 历史一致。
- NOTICE 分别列出 `BigPizzaV3/CodexPlusPlus through a0506ae` 与 `cc-switch/Jason Young`，并明确完整 MIT 条款 independently 适用于每个列明作品。根 AGPL LICENSE、Cargo、双语 README 和公开源 URL 一致。
- 源码资产由 `git archive ... "$TARGET_SHA"` 生成，通过 `git ls-tree`、`tar -tzf`、`diff` 比对完整树，并显式要求锁文件、两平台打包脚本、LICENSE、NOTICE。
- SelfTest 分别破坏 TARGET_SHA、tree listing、archive listing、diff、required-file、两条 MIT scope/归属、AGPL/Cargo/README 和公开源 URL，均保持 fail-closed。
- Release notes/latest body 展示 AGPL、NOTICE 及同 tag 源码资产；客户包内 SOURCE_CODE.txt 记录精确源码 URL 与 Release commit。

### 四类资产

- Windows installer/zip、macOS x64/arm64 DMG/zip 均有 LICENSE、NOTICE、SOURCE_CODE.txt 的 staging/打包路径。
- Windows 安装器将三份法律文件纳入 `.new`/`.bak` 事务、回滚、清理和互斥保护；macOS DMG 在 stage 验证三份文件，zip 从同一构建 checkout 复制三份文件。
- `verify-license -SelfTest` 逐项破坏四类资产中三份法律文件的打包 token 并要求失败。

### 本轮只读验证

- `cargo test -p codex-plus-core --test installers windows_legal_files_share_the_binary_transaction_and_uninstall_mutex --locked -- --exact`：1/1 PASS。
- `pwsh -NoProfile -File scripts/verify-license.ps1`：PASS。
- `pwsh -NoProfile -File scripts/verify-license.ps1 -SelfTest`：PASS。
- `cargo test -p codex-plus-core --test installers --locked`：22/22 PASS。
- `cargo test -p codex-plus-manager --test windows_subsystem --locked`：39/39 PASS。
- `cargo metadata --locked --no-deps --format-version 1`：PASS。
- scoped `git diff --check`：PASS（仅现有工作树行尾转换提示，无 whitespace error）。

Task 11 Step 11.3 在当前范围内可以关闭。首次公开发布仍需在全部变更提交后由真实 Windows/macOS Release Actions 验收最终产物。
