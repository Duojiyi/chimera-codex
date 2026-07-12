# Task 11 Aggregate Audit A

## 结论

**PASS**

按 Task 11 / T30 的需求、Step 11.1-11.4 最终 evidence、双盲审计、当前实现和本轮独立回归复核，品牌真相源、客户 README、AGPL/NOTICE/对应源义务、单桌面入口、LaunchRoute、installer 事务和 manager 已运行实例路由均已闭合。未发现阻断 Task 11 Gate 的未解决项。

本审计未读取或引用 Task 11 聚合审计 B，未修改实现代码。聚合 A 的 PASS 不代替聚合 B，也不直接勾选 T30。

## Step 门

### Step 11.1：品牌真相源与生成物

- 最终审计 A/B 均为 PASS；历史 FAIL 中的硬编码窗口标题、旧 User-Agent、capability、i18n、公开模板和 provider-sync marker 已有 Red/Green 与最终回归关闭证据。
- `brand/product.toml` 当前唯一产品值为 `Chimera++`、`Chimera++ 管理工具`、publisher `ChimeraHub`、artifact prefix `ChimeraPlusPlus`、origin `Duojiyi/chimera-codex`。
- Rust/TS 生成物、窗口、托盘、Tauri、installer、macOS、workflow 和出站标识由生成器或品牌 helper 约束；残留 `Codex++` 仅位于明确兼容、迁移或上游归属边界。
- 本轮 `generate-branding.ps1 -Check`、branding `2/2` 和真实去推广扫描通过，无生成漂移。

### Step 11.2：客户 README

- 最终审计 A/B 均为 PASS；正式 clean B 已排除曾误读审计文件的宽扫描结果。
- 中英文客户区保持 Key-only、固定 ChimeraHub `/v1` URL、`ChimeraPlusPlus-*` 三平台资产名、单桌面目标与真实性警示一致。
- GitHub、AGPL/NOTICE 和开源归属只位于开发/归属区；客户快速路径不包含 About、手动更新、旧 Chimera 名称或未经证实 MIT 声明。
- `Codex++` 仅用于覆盖升级、旧 App/快捷方式识别与清理语境；allowlist 使用精确路径、pattern、行号和完整行，未提供通配绕过。
- 本轮 README/installers 合约、去推广扫描和 allowlist/docs/assets fail-closed 自测通过。

### Step 11.3：许可证、NOTICE 与对应源

- 最终审计 A/B 均为 PASS；AGPL-3.0-only、上游 MIT 基线、cc-switch MIT 材料和 license-change timeline 的适用范围明确。
- `LICENSE`、`NOTICE`、Cargo metadata 与 README 一致；4 个 workspace package 均为 `AGPL-3.0-only`，repository 均指向公开 origin。
- Release 精确集合为 6 个安装资产、1 个绑定 `TARGET_SHA` 的源码包和 `latest.json`；Windows installer/zip、macOS DMG/zip 均携带 `LICENSE`、`NOTICE`、`SOURCE_CODE.txt`。
- draft 发布前内容门、published 幂等资产/源码复核、源码 tree/listing/diff/required-file 完整性和 NSIS 法律文件事务均 fail closed。
- 本轮普通许可证验证与 30 个负向 self-test 均 PASS。

### Step 11.4：单桌面入口与路由

- 顶层最终审计 A/B 已实际覆盖为 PASS；route、installer、manager 三组 remediation A/B 也均为 PASS。
- LaunchRoute 优先级为 mandatory update、损坏 settings、已就绪 active relay/合规 official login、已有 Key 待应用、全新 Key-first。当前不把普通更新误提升为 mandatory；真实可信 floor 属于 Task 13。
- 普通与 Aggregate relay 均通过结构化 TOML 和 live identity fail-closed 判定；错误 provider、URL、Key、认证标志或非法 TOML 不会误启动。
- Windows 桌面只创建 `Chimera++.lnk`，管理工具保留在开始菜单；current/legacy/compat 快捷方式、ARP、URL protocol 与法律文件纳入备份、mutex、commit、rollback 和保守清理边界。
- manager 文件 IPC 具备原子 publish/claim、稳定 FIFO、frontend ready/not-ready 生命周期、timeout ownership、坏条目隔离、fatal guard fail-closed、ACK 与 orphan artifact 清理；已运行 manager 不再丢失目标 route。

## 聚合交叉检查

- 品牌名称、asset prefix、installer 输出、README 下载名、release workflow 和 updater manifest 之间未发现漂移。
- README 的“尚不可作为客户正式发行版”警示仍准确：Task 11 的单入口已完成，但 Task 13 自动更新强制策略和后续 Release/实机 Gate 尚未完成。
- AGPL/NOTICE/source obligations 同时覆盖公开仓库、release notes、源码资产和四类客户交付物，没有因客户区去 GitHub 化而删除法律入口。
- 兼容 ID、旧快捷方式和旧 App 名保留均服务于升级/清理，不重新暴露为当前产品入口。
- Step 11.1-11.4 的历史 FAIL 均有对应 remediation、最终干净 A/B 和当前回归证据，未以旧中途 PASS 或仅字符串存在性替代最终门。

## 本轮验证

- `pwsh -NoProfile -File scripts/generate-branding.ps1 -Check`：PASS。
- `cargo test -p codex-plus-core --test branding --locked`：2 passed。
- `cargo test -p codex-plus-core --test installers --locked`：23 passed。
- `cargo test -p codex-plus-core --test launcher --locked`：66 passed。
- `cargo test -p codex-plus-core --test relay_config --locked`：97 passed。
- `cargo test -p codex-plus-launcher --locked`：8 passed。
- `cargo test -p codex-plus-core --lib install::windows::tests:: --locked`：4 passed。
- `cargo test -p codex-plus-manager --lib`：54 passed。
- `cargo test -p codex-plus-manager --test windows_subsystem`：40 passed。
- `npm run check`、`npm run vite:build`：PASS，Vite 1608 modules transformed。
- `pwsh -NoProfile -File scripts/verify-license.ps1`：PASS。
- `pwsh -NoProfile -File scripts/verify-license.ps1 -SelfTest`：PASS。
- `pwsh -NoProfile -File scripts/verify-no-upstream-ads.ps1`：PASS。
- `pwsh -NoProfile -File scripts/test-verify-allowlist.ps1`：PASS。
- `cargo metadata --locked --no-deps --format-version 1`：4/4 package 为 `AGPL-3.0-only` 且 repository 为 origin。
- `cargo fmt --check`、`git diff --check`：PASS；仅有工作树行尾转换提示，无 whitespace error。

## 残余风险与后续 Gate

- 本机缺少 `makensis`，未执行真实 NSIS 编译、覆盖安装、卸载和 fault-injection；由 Release CI 与 Task 16 Windows 实机 Gate 验收。
- 未执行真实双进程 Tauri/WebView 崩溃注入；manager claim 后进程崩溃采用 at-most-once 和 30 秒 orphan cleanup 的明确取舍。
- macOS x64/arm64 DMG/zip 的签名、Gatekeeper 和真实安装资产内容仍需 Release Actions 与实机验收。
- `minimum_supported_version`、可信 floor 和自动/强制更新状态机属于 Task 13；Task 11 只保证纯路由优先级和普通更新不被误判为强更。

以上风险均已明确归属后续任务或发布 Gate，不构成 Task 11 / T30 当前范围的阻断。Task 11 可在聚合 B 也 PASS 后进入 T30 勾选流程。
