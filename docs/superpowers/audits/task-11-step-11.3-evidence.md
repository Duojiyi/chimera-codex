# Task 11 Step 11.3 Evidence

## Verified Timeline

- Upstream tag `v1.2.34` = `c1360294`, 2026-07-10 10:51 +08:00.
- Fork baseline continues through upstream `a0506ae`; Cargo metadata declared MIT, but no LICENSE/NOTICE was present.
- Upstream `7f72aec`, 2026-07-10 23:45 +08:00, added AGPLv3 and changed subsequent versions to `AGPL-3.0-only`; its README states earlier versions are not retroactively relicensed.
- This public fork tracks future formal upstream Releases, so continuing to claim MIT would become incompatible when AGPL upstream code is merged.

## Red

- `pwsh -NoProfile -File scripts/verify-license.ps1`：FAIL，`21 finding(s)`。
- 失败包括缺 LICENSE/NOTICE、Cargo 仍为 MIT、中英文 README 缺 AGPL 与许可证链接。

## License Decision

- Chimera++ 修改后的整体采用 `AGPL-3.0-only`，以兼容未来正式上游 Release 同步。
- LICENSE 从 `upstream/main:LICENSE` 机械复制；git blob 均为 `0ad25db4bd1d86c452db3f9602ccdbe172438f52`，本地 SHA-256 为 `8486A10C4393CEE1C25392769DDD3B2D6C242D6EC7928E1414EFFF7DFB2F07EF`。
- NOTICE 记录 v1.2.34/a0506ae MIT metadata 基线、7f72aec 后续 AGPL 变更、BigPizzaV3 版权与 Chimera 修改。
- cc-switch 官方 license API 返回 SPDX MIT、Copyright (c) 2025 Jason Young；NOTICE 包含完整 MIT 文本。

## Artifact-Carriage Red

- 增加 Windows installer/zip、macOS DMG/zip 必须携带 LICENSE/NOTICE 的契约。
- core installers：`18 passed / 3 failed`。
- manager PR workflow targeted：`0 passed / 1 failed`。

## Initial Green

- `verify-license.ps1`：PASS，包含 LICENSE SHA-256、NOTICE 完整性、Cargo/README 和四类发行资产携带门。
- `cargo metadata --locked --no-deps`：四个 workspace package 均为 `AGPL-3.0-only`（`4/4`）。
- Windows NSIS 嵌入并卸载 LICENSE/NOTICE；Windows portable zip staging 携带两文件。
- macOS DMG stage 与 portable zip staging 携带两文件；release workflow 验证 DMG stage 文件存在。

## Final-Audit Remediation Red

- 对应源契约：`github_release_archives_corresponding_source_from_resolved_target` 先 FAIL，缺少从可信 `TARGET_SHA` 生成的 `ChimeraPlusPlus-${VERSION}-source.tar.gz`。
- 四类精确源码说明契约：先 FAIL，Windows installer/zip 与 macOS DMG/zip 均缺 `SOURCE_CODE.txt`。
- 负向门禁：`pwsh -NoProfile -File scripts/verify-license.ps1 -SelfTest` 先 FAIL，参数不存在。
- CI 接线：`pr_build_enforces_format_and_pins_actions` 先 FAIL，PR/Release gates 未执行许可证门禁自测。
- Release 法律入口：源码归档契约再次先 FAIL，发布说明缺 AGPL、NOTICE 与同版本对应源链接。

## Final Green

- 发布作业在校验 checkout 等于 `TARGET_SHA` 后，通过 `git archive ... "$TARGET_SHA" | gzip -n` 生成确定性源码包；`git ls-tree` 与 `tar -tzf` 全量比对归档路径，并显式核验 Cargo/npm lockfile、Windows/macOS 构建安装脚本、LICENSE 与 NOTICE。
- Release 精确资产集合为 6 个安装资产、1 个源码包和 `latest.json`；updater 的 `latest.json` 明确过滤源码包，继续只列 6 个安装资产。
- Windows 与 macOS 构建均生成包含同 Release 源码资产 URL 和精确提交 SHA 的 `SOURCE_CODE.txt`；NSIS、Windows zip、macOS DMG 和 macOS zip 均携带，NSIS 卸载会清理该文件。
- Release notes 保留 `AGPL-3.0-only`、NOTICE 和同版本源码包入口；这属于发布法律信息，不进入普通用户 UI。
- `verify-license.ps1 -SelfTest` 使用内存快照执行 30 个 fail-closed 场景，不创建或递归删除 fixture：缺 LICENSE/NOTICE、上游与 cc-switch 独立 MIT 适用范围、MIT 版权/授权/条件/免责声明残缺、Cargo/README 不一致、源 URL 离开 origin、源码包未绑定 `TARGET_SHA`、源码树/listing/diff/required-file 完整性失效、draft 内容门被注释，以及四类产物缺 LICENSE/NOTICE/SOURCE_CODE。
- PR 与 Release gates 均运行普通许可证验证和负向自测。

## Final Regression

- `cargo test -p codex-plus-manager --test windows_subsystem --locked`：`39 passed`。
- `cargo test -p codex-plus-core --test installers --locked`：`22 passed`。
- `pwsh -NoProfile -File scripts/verify-license.ps1`：PASS。
- `pwsh -NoProfile -File scripts/verify-license.ps1 -SelfTest`：PASS。
- `cargo metadata --locked --no-deps --format-version 1`：workspace `4/4` 为 `AGPL-3.0-only`。
- `pwsh -NoProfile -File scripts/generate-branding.ps1 -Check`：PASS。
- `pwsh -NoProfile -File scripts/verify-no-upstream-ads.ps1`：PASS。
- `pwsh -NoProfile -File scripts/test-verify-allowlist.ps1`：PASS。
- PyYAML 解析 PR/Release workflow：PASS。
- `cargo fmt --all -- --check` 与 `git diff --check`：PASS。

## First Final-Audit Findings And Remediation

- 审计 A 首轮 FAIL：完整 MIT 正文只明确归入 cc-switch，未无歧义覆盖上游 MIT 基线；SelfTest 未分别破坏 `git ls-tree`、`tar`、`diff` 和 required-file 门。
- Red：普通许可证验证因缺上游独立 MIT 适用声明而 `2 finding(s)`；`license_self_test_breaks_each_source_archive_integrity_gate` 因缺四个负向场景而 FAIL。
- Green：NOTICE 现在分别列出 BigPizzaV3/CodexPlusPlus through `a0506ae` 与 cc-switch/Jason Young，并明确同一完整 MIT 正文独立适用于两项材料；验证器与 SelfTest 分别门禁其范围和四个源码完整性步骤。
- 审计 B 首轮 FAIL：draft 只验资产名、published 幂等路径不拒绝额外资产/不复核源码、三份法律文件不在 NSIS 事务内、卸载器未共享 setup mutex。
- Red：`release_verifies_remote_asset_content_before_publish_and_on_idempotent_exit` 与 `windows_legal_files_share_the_binary_transaction_and_uninstall_mutex` 均先 FAIL。
- Green：draft 在 publish 前对 8 个远端资产逐一比较 SHA-256 digest 与 size；published 路径要求精确 8 资产、Release target 与 tag 一致，并从 `TARGET_SHA` 重建源码包后与远端源码包字节级 `cmp`。
- Green：NSIS 将 LICENSE/NOTICE/SOURCE_CODE 纳入与二进制相同的 `.new` staging、`.bak` backup、commit、rollback 和 cleanup；`un.onInit` 与 setup 共享 `Local\\ChimeraPlusPlus.Setup.Transaction` mutex。
- 修复后 targeted 与完整回归均 Green；进入第二轮干净 A/B 复审，旧 FAIL 不作为放行依据。

## Second B Review Findings And Remediation

- 审计 B 第二轮仍 FAIL：`verify_draft_assets_content` 调用被注释后，普通许可证门、自测和 Rust 字符串断言会伪通过；卸载器连续删除法律 `.new` 文件后 `ClearErrors`，可能吞掉删除失败并继续移除元数据。
- Red：许可证 SelfTest 契约因缺 `commented draft asset content gate` 而 FAIL；安装器事务契约因缺 fail-closed `UninstallFile` 调用而 FAIL。
- Green：许可证验证新增精确活动行门，SelfTest 用正则只注释 publish 前的调用并确认验证失败；Rust 契约将检查窗口限定在 upload 与 publish 之间并要求存在精确活动命令行。
- Green：NSIS 新增 `UninstallFile` 宏，所有程序/法律 `.new`、`.bak` 恢复文件均先 `IfFileExists`，删除失败立即跳转 `uninstall_failed`，成功前不会继续删除快捷方式、ARP 或卸载器。
- 修复后 `windows_subsystem 39/39`、`installers 22/22`、license SelfTest、去推广扫描、allowlist 自测、Rust format 与 YAML parse 全部 PASS；进入最终干净 A/B 复审。

## Final A Test-Gap Remediation

- 最终 A 复审仅剩测试充分性 FAIL：`UninstallFile` 实现正确，但删除宏体中的 `IfErrors uninstall_failed` 时旧测试仍可能通过。
- 增加宏体语义解析：只读取 active 行，按序要求 `IfFileExists`、`ClearErrors`、`Delete`、`IfErrors uninstall_failed`；随后在内存中仅注释错误跳转，断言同一契约必须失败。
- Targeted 与完整 installers 回归均 `22/22` PASS；该修复只增强测试门，不改变已审计的卸载行为。

## Final Double-Blind Result

- 审计 A：PASS，许可链、源码完整性 fail-closed、宏体错误跳转 mutation 和四类交付法律入口均无阻断。
- 审计 B：PASS，draft/published 资产内容、active publish gate、NSIS 法律事务、setup/uninstall mutex 与卸载 fail-closed 均无阻断。
- Step 11.3 已满足勾选门；真实 Windows/macOS Release Actions 仍保留为发布前验收门，不以本机静态测试伪装实机结果。
- core installer/document contracts：`21/21`。
- manager workflow contracts：`35/35`。
- 去推广扫描、allowlist fail-closed、rustfmt、full `git diff --check`：PASS。

真实 NSIS、macOS x64/arm64 构建与安装资产内容仍由后续远端 CI/实机 Gate 验收，不在 Windows 本机伪造通过。
