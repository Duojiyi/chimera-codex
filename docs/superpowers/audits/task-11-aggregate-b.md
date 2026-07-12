# Task 11 Aggregate Audit B

## Conclusion

结论：**PASS**。

本轮独立按 Task 11 / T30 的最终 diff、任务边界、Step evidence、双盲门与回归面复核。未读取或引用 Task 11 聚合审计 A，也未修改实现代码。Step 11.1-11.4 的最终 A/B 均为 PASS；其中 Step 11.4 最终 B 已在本聚合结论前完成并通过。

Task 11 已满足“品牌真相源、发行/打包名称、中英文客户 README、许可证发布门和 Windows 单桌面入口”的代码与文档门。未发现会阻断 T30 关闭的新问题。

## Scope and boundary

Task 11 聚合范围包括：

- `brand/product.toml` 及 Rust/TypeScript 生成品牌常量；
- Windows/macOS 打包名称、Tauri 产品名、窗口/托盘和发行 workflow 品牌触点；
- 中英文 README 的 ChimeraHub Key-first 客户路径、迁移说明与开源归属边界；
- AGPL-3.0-only、NOTICE、对应源归档和四类发行资产法律文件携带；
- Windows 单桌面 `Chimera++`、开始菜单管理入口、覆盖升级清理/回滚；
- 单入口启动路由、active relay/官方登录/Key-first 判定及 manager 已运行实例的目标页交付。

以下事项不属于 T30，仍保持未完成状态：

- About/GitHub/推荐等生产 UI 与注入菜单清理属于 Task 12 / T31；
- `minimum_supported_version`、普通/强制自动更新状态机属于 Task 13 / T32；
- 原创图标资产替换属于 Task 14 / T33；
- 远端同步、Release 上线与真实 Actions 结果属于 Task 15 / T34；
- Windows/macOS 实机安装、覆盖升级、Gatekeeper 与最终发行验收属于 Task 16 / T35。

因此本 PASS 不表示当前工作树已经可以对客户发布，也不把后续任务的目标行为冒充为已完成行为。

## Independent diff review

### Branding truth and generated consumers

- `brand/product.toml` 固定 `Chimera++`、`Chimera++ 管理工具`、`ChimeraPlusPlus`、公开仓库、ChimeraHub `/v1` 和 `ads_enabled = false`。
- `generate-branding.ps1 -Check` 对 Rust/TypeScript 生成文件做字节级漂移检查，并校验 Cargo/npm/Tauri 版本一致及 `X.Y.Z-chimera.N` 格式。
- 生成器同时门禁 NSIS、macOS packager、Tauri、PR/Release workflows 和 README 资产名；生产品牌测试与去推广扫描补足未直接生成的 User-Agent、provider-sync marker 和公开文档触点。
- 内建 HTTP User-Agent 统一由 `ARTIFACT_PREFIX` 派生，当前无 `ChimeraCodex`、`CodexPlusPlus/` 或旧 provider-sync writer 残留。兼容二进制名、provider id、协议 scheme、bundle id 和安装根仍按规格保留。

### README customer boundary

- README 客户区位于唯一的开发/归属分隔标题之前；该区域不包含 GitHub/About、手动检查或手动下载安装更新路径。
- 中文与英文均说明只输入 API Key、固定 ChimeraHub `/v1`、Windows 单桌面目标、macOS ad-hoc/Gatekeeper 限制及升级不覆盖现有 profile。
- 旧 `Codex++` 只出现在迁移语境；必要的本项目、上游和 cc-switch 链接只保留在开发与归属区。
- README 明确声明当前开发快照尚未完成自动更新强制策略和最终验收，不得作为客户发行版交付，避免把 Task 13-16 的目标写成当前承诺。
- README 契约包含大小写/独立 token、重复边界、手动更新同义表达和合法迁移文案的正负 fixture，未发现明显伪绿路径。

### License and corresponding source gate

- 根 `LICENSE` SHA-256 为 `8486A10C4393CEE1C25392769DDD3B2D6C242D6EC7928E1414EFFF7DFB2F07EF`；四个 workspace package 均声明 `AGPL-3.0-only` 并指向本项目公开仓库。
- `NOTICE` 分别记录上游 MIT 基线、后续 AGPL 时间线、BigPizzaV3 归属和 cc-switch/Jason Young MIT 材料，完整 MIT 文本明确独立适用于两项材料。
- Release workflow 从不可变 `TARGET_SHA` 使用 `git archive | gzip -n` 生成对应源，比较 Git tree、归档 listing、required files，并在草稿发布前核对精确 8 资产的 digest/size。
- 已发布幂等路径继续核对 tag/target、精确资产集合并重建对应源字节比较；`latest.json` 只包含 6 个安装资产，不把源码包作为 updater 资产。
- Windows installer/zip、macOS DMG/zip 均携带 LICENSE、NOTICE、SOURCE_CODE；NSIS 将三份法律文件纳入与二进制相同的 staging/backup/commit/rollback 和卸载 fail-closed 边界。
- `verify-license.ps1 -SelfTest` 覆盖许可证缺失、范围声明、对应源完整性、被注释的 draft 内容门及四类产物缺文件等负向变异。

### Single desktop, route and transaction boundary

- NSIS 桌面 `CreateShortcut` 精确为一个 `Chimera++.lnk`；开始菜单保留主入口、管理工具和卸载入口。current、legacy、乱码和 compat 快捷方式均先备份，再删除，并具有对称回滚。
- Windows Rust install/repair 路径同样只创建主桌面入口；快捷方式原始 bytes 与 registry raw name/type/data 在事务失败时恢复。
- NSIS 与 Rust 共用 `Local\ChimeraPlusPlus.Setup.Transaction`；NSIS 以 `initialOwner=1` 获取 mutex，legacy ARP 整键删除延后到不可回滚提交点，URL protocol 仅删除 owned values 并保守清理空键。
- `select_launch_route` 顺序为 mandatory update、settings recovery、ready relay/official login、Key 待应用、Key-first。当前未把普通 `update_available` 提升为 mandatory；可信 floor 的生产接线明确留给 Task 13。
- 普通 relay 与 Aggregate 均严格匹配 live identity；普通配置完整 TOML 非法、provider/URL/Key/认证标志不一致均 fail closed。官方登录与中转模式门控、Key 来源边界均有负向测试。
- manager 已运行实例的 route 使用原子文件发布/claim、稳定 FIFO、frontend ready/not-ready 和 ACK；坏条目、超时、迟到 ACK、orphan 辅助文件和 guard fatal error 均有收束行为。

## Step and evidence gates

- Step 11.1：最终 A/B PASS；品牌真相源、窗口/UI、User-Agent、provider-sync marker、installer/workflow 触点与扫描门闭合。
- Step 11.2：最终 A/B PASS；README 唯一边界、客户禁项、迁移语境、真实性警示和负例 fixture 闭合。
- Step 11.3：最终 A/B PASS；许可证范围、对应源、draft/published 内容门、法律文件事务与卸载 fail-closed 闭合。
- Step 11.4：最终 A/B PASS；单桌面、路由 identity、manager IPC、installer 并发/回滚及非法 TOML fail-closed 闭合。
- 各 evidence 保留了初始 Red、审计发现的补充 Red、Green 和针对性回归；历史 FAIL 被保留但最终轮结论与当前实现一致。

## Verification

本聚合审计在当前工作树实际执行或在紧邻的 Step 11.4 最终复核中执行并确认：

- `cargo test -p codex-plus-core --test branding --locked`：`2 passed`。
- `cargo test -p codex-plus-core --test installers --locked`：`23 passed`。
- `cargo test -p codex-plus-core --test relay_config --locked`：`97 passed`。
- `cargo test -p codex-plus-core --test launcher --locked`：`66 passed`。
- `cargo test -p codex-plus-launcher --locked`：`8 passed`。
- `cargo test -p codex-plus-manager --locked`：lib `54 passed`、`windows_subsystem 40 passed`，bin/doc `0 failed`。
- Windows install transaction unit tests：`4 passed`。
- `npm run check`、`npm run vite:build`：PASS，Vite `1608 modules transformed`。
- branding generation、license、license SelfTest、去推广扫描、allowlist fail-closed：PASS。
- `cargo metadata --locked --no-deps`：workspace `4/4` 为 `AGPL-3.0-only`。
- `cargo fmt --all -- --check`、`git diff --check`：PASS；仅有 Git 行尾转换提示，无 whitespace error。

## Residual risks

1. 本机没有 `makensis`，未把 NSIS 静态契约冒充为真实编译或安装测试；Release CI 与 Task 16 必须完成实际构建、覆盖安装、失败回滚和卸载冒烟。
2. 本轮没有执行真实双进程 Tauri/WebView 崩溃注入；manager IPC 对 claim 后崩溃采用 at-most-once 和过期清理的明确取舍。
3. Task 11 的 workflow 契约和许可证自测通过，不等于远端 Release 已上线或资产已匿名可用；远端发布门仍由 Task 15 控制。
4. 当前 README 的发行准备警示必须保留，直到 Task 13-16 的更新、图标、远端和实机门全部完成。

从聚合审计 B 侧，Task 11 / T30 可以进入完成状态；仍须由独立聚合审计 A 同样 PASS 后，才可按项目流程勾选 T30 并继续依赖任务。
