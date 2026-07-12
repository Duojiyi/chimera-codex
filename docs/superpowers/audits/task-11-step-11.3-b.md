# Task 11 Step 11.3 Audit B

## 结论

**PASS**。按当前最终快照独立复核 diff、边界、回归面和门禁伪通过风险，未发现阻断项。发布前远端内容校验、已发布幂等验证、`latest.json` 资产边界、NSIS 法律文件事务、setup/uninstall 互斥及卸载 fail-closed 均形成了实现与针对性回归证据。

## 独立复核证据

### 1. 注释发布前内容门禁会 fail closed

- `scripts/verify-license.ps1:63-68` 的 `Assert-ActiveLine` 按 trim 后精确活动行匹配，不接受 `# verify_draft_assets_content`。
- 普通验证在 `scripts/verify-license.ps1:184` 强制要求活动 `verify_draft_assets_content` 行。
- SelfTest 在 `scripts/verify-license.ps1:228-240,260` 将该活动调用替换为 YAML 注释并要求验证失败；本轮 `-SelfTest` PASS，证明该负向 case 确实被捕获。
- `apps/codex-plus-manager/src-tauri/tests/windows_subsystem.rs:414-423` 将检查范围限定在最后一次 `gh release upload` 与 `draft=false` publish 之间，并要求存在 `line.trim() == "verify_draft_assets_content"` 的活动命令。
- 只读内存结构验证：当前 upload→publish 窗口活动调用数为 1；注释变异后活动调用数为 0。

### 2. 草稿发布前远端 8 资产内容与集合均精确

- `.github/workflows/release-assets.yml:624-648` 要求草稿状态、资产总数恰为 8、每个期望名称恰好出现一次。
- `.github/workflows/release-assets.yml:650-682` 对 6 个安装资产、源码包和 `latest.json` 逐一计算本地 SHA-256/size，并与 GitHub Release API 的 `digest`/`size` 比较。
- `.github/workflows/release-assets.yml:839-846` 的顺序为 upload → target 校验 → exact set → digest/size → publish；内容校验失败时 Release 保持 draft。

### 3. 已发布幂等路径绑定精确资产、tag/target 与 TARGET_SHA 源码

- `.github/workflows/release-assets.yml:136-166` 要求正式 Release 资产总数恰为 8、每项唯一、`targetCommitish` 为 40 位不可变 SHA，且与 tag commit 一致。
- `.github/workflows/release-assets.yml:184-196` 继续要求目标是 `origin/main` 祖先、目标 `Cargo.toml` 版本与当前发行版本一致。
- `.github/workflows/release-assets.yml:198-234` checkout 已解析的 `TARGET_SHA`，用相同 prefix 与 `gzip -n` 重建对应源，下载正式源码资产并执行逐字节 `cmp`，随后再次比较 archive tree 与 Git tree。

### 4. latest.json 仅含 6 个安装资产

- `.github/workflows/release-assets.yml:743-763` 先限定本地只有 7 个 `ChimeraPlusPlus-*` 发布文件（6 个安装资产 + 1 个源码包）。
- `.github/workflows/release-assets.yml:776-779` 明确从 manifest 输入排除精确源码包名。
- 新发布和已发布 smoke 均要求 `.assets` 数组长度恰为 6，并逐项核对 URL、SHA-256 与 size（`.github/workflows/release-assets.yml:245-283,861-898`）。未发现源码误入 updater manifest 的路径。

### 5. NSIS 法律文件事务、mutex 与卸载顺序闭合

- `scripts/installer/windows/CodexPlusPlus.nsi:168-282` 将程序、卸载器、`LICENSE`、`NOTICE`、`SOURCE_CODE.txt` 全部纳入 `.new` staging、`.bak` backup 和 commit。
- `scripts/installer/windows/CodexPlusPlus.nsi:382-410` 成功后清理全部备份；`:456-535` 失败时按相反顺序删除新文件、恢复旧备份并清理 staging。
- setup `.onInit`（`:102-120`）与 uninstall `un.onInit`（`:122-140`）共用 `Local\ChimeraPlusPlus.Setup.Transaction`，创建失败或发现已存在 mutex 均 abort，卸载不能并发删除 setup 的恢复文件。
- `UninstallFile` 宏（`:94-100`）对存在的每个文件执行 `ClearErrors` → `Delete` → `IfErrors uninstall_failed`。
- Uninstall 段 `:573-584` 对程序/法律文件的全部 12 个 `.new/.bak` 路径各调用一次 `UninstallFile`；这些调用全部位于快捷方式和注册表 metadata 删除（`:585-606`）之前。
- 只读结构验证结果：`UninstallFileCallCount=12`、`EachExpectedCallExactlyOnce=True`、`FilesBeforeShortcuts=True`、`FilesBeforeRegistry=True`、宏内 `IfErrors uninstall_failed=True`。

### 6. 四类交付与许可证元数据

- Windows setup、Windows zip、macOS DMG、macOS zip 均包含 `LICENSE`、`NOTICE`、`SOURCE_CODE.txt`；release workflow 还对 macOS stage 三份文件做存在性检查。
- macOS packager 拒绝已有输出及符号链接输出父路径，在 DMG 创建前复制法律文件；未发现 staging 覆盖或递归删除风险。
- `LICENSE` SHA-256 为 `8486A10C4393CEE1C25392769DDD3B2D6C242D6EC7928E1414EFFF7DFB2F07EF`。
- Cargo metadata 中 4 个 workspace package 均为 `1.2.34-chimera.1`、`AGPL-3.0-only`，repository 均为 `https://github.com/Duojiyi/chimera-codex`。

## 执行结果

- `pwsh -NoProfile -File scripts/verify-license.ps1`：PASS。
- `pwsh -NoProfile -File scripts/verify-license.ps1 -SelfTest`：PASS，包含“注释 draft 内容门禁必须失败”的负向 case。
- `cargo metadata --no-deps --format-version 1`：PASS。
- `cargo test -p codex-plus-core --test installers --locked`：PASS，22 passed。
- `cargo test -p codex-plus-manager --test windows_subsystem --locked`：PASS，39 passed。
- 指定实现与测试文件 `git diff --check`：PASS；仅有 Git LF/CRLF 提示，无 whitespace error。
- 本机未提供 `actionlint`、`makensis` 或 Ruby YAML parser，因此未执行独立 Actions parser 与本地 NSIS 编译；PR/release workflow 会安装固定版本 NSIS 并实际构建安装器。基于当前脚本控制流、内存结构断言和定向测试，未形成残余阻断。
