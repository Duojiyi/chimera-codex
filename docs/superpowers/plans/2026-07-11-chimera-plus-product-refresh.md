# Chimera++ 客户发行版刷新实施计划

> Spec：`docs/superpowers/specs/2026-07-11-chimera-plus-product-refresh-design.md`
> TODO：`docs/superpowers/todos/2026-07-11-chimera-plus-product-refresh-todo.md`

## 执行门禁

每个可观察代码行为 Step 固定执行：先添加会失败的行为测试并保存 Red 命令与输出摘要；再做最小实现并保存 Green；运行针对性回归；最后由审计 A 按需求/行为独立检查、审计 B 按 diff/边界/回归面独立检查。远端配置、人工设计选择和实机冒烟不伪造单元测试 Red/Green，而是先保存失败前置证据，再执行最小变更/验证，并同样完成独立 A/B 审计。两份审计都 PASS 后才能勾选 Step。

每个 Task 末尾使用 **Task Gate**，不是最小 Step checkbox：聚合 A/B 审计复核全部 Step 证据，通过后才勾选对应 TODO 大任务。

历史审计证明旧基线可用，但不能替代本轮新行为的 Red/Green 或审计。

## 验证命令表

以下命令均从仓库根目录执行；前端命令的工作目录单独标明。Step 必须记录实际命令、exit code 和关键计数，不能只写“测试通过”。

| ID | 命令 |
|---|---|
| C1 | `cargo test -p codex-plus-core --test branding --test installers --locked` |
| C2 | `cargo test -p codex-plus-core --test cdp_bridge --test bridge_routes --locked` |
| C3 | `cargo test -p codex-plus-core --test updater --locked` |
| C4 | `cargo test -p codex-plus-launcher --locked` |
| C5 | `cargo test -p codex-plus-manager --locked` |
| C6 | 在 `apps/codex-plus-manager` 执行 `npm run check` 和 `npm run vite:build` |
| C7 | `pwsh -NoProfile -File scripts/generate-branding.ps1 -Check` |
| C8 | `pwsh -NoProfile -File scripts/verify-no-upstream-ads.ps1` 与 `pwsh -NoProfile -File scripts/test-verify-allowlist.ps1` |
| C9 | `pwsh -NoProfile -File scripts/test-sync-upstream.ps1` |
| C10 | `cargo fmt --all -- --check`、`cargo test --workspace --locked`、`git diff --check` |
| C11 | `pwsh -NoProfile -File scripts/verify-brand-icons.ps1` |
| C12 | `pwsh -NoProfile -File scripts/verify-license.ps1`、`pwsh -NoProfile -File scripts/verify-license.ps1 -SelfTest` 与 `cargo metadata --locked --no-deps` |

## Task 与 TODO 映射

| Plan Task | TODO | 聚合审计 |
|---|---|---|
| Task 11 | T30 | `audits/task-11-aggregate.md` |
| Task 12 | T31 | `audits/task-12-aggregate.md` |
| Task 13 | T32 | `audits/task-13-aggregate.md` |
| Task 14 | T33 | `audits/task-14-aggregate.md` |
| Task 15 | T34 | `audits/task-15-aggregate.md` |
| Task 16 | T35 | `audits/task-16-aggregate.md` |

每个 Step 的两份记录固定为 `audits/task-<task>-step-<step>-a.md` 和 `-b.md`。

## Task 11（T30）：品牌、客户 README 与单桌面入口

Files：`brand/product.toml`、`scripts/generate-branding.ps1`、`crates/codex-plus-core/src/branding.rs`、`apps/codex-plus-manager/src/branding.generated.ts`、`crates/codex-plus-core/tests/branding.rs`、`crates/codex-plus-core/tests/installers.rs`、`apps/codex-plus-launcher/src/main.rs`、`scripts/installer/windows/CodexPlusPlus.nsi`、`scripts/installer/macos/package-dmg.sh`、`.github/workflows/pr-build.yml`、`.github/workflows/release-assets.yml`、`apps/codex-plus-manager/src-tauri/tests/windows_subsystem.rs`、`README.md`、`README_EN.md`、`Cargo.toml`、`LICENSE`、`NOTICE`、`scripts/verify-license.ps1`。Tests：C1、C4、C5、C7、C8、C12。Rollback/停止：任一生成漂移、升级用户误进 Key-first、快捷方式回滚失败或源码发行权利不明确即停止，只撤销本 Step 的 agent-owned hunks，不勾选 T30/不发布。

- [x] Step 11.1：Red 修改 branding/installer/workflow 契约期望为 `Chimera++`、`Chimera++ 管理工具`、`ChimeraPlusPlus`；Green 修改 `brand/product.toml`、运行生成器并最小更新窗口、托盘、安装器和产物名；跑 branding/installer/workflow 回归并完成 A/B 审计。
- [x] Step 11.2：Red 扩展文档门禁，拒绝客户快速路径中的旧品牌、手动 About 更新和未经证实 MIT 声明；Green 重写中英文 README，保留必要开源归属但不把 GitHub 当客户操作入口；跑扫描回归并完成 A/B 审计。
- [x] Step 11.3（许可证验证）：保存“Cargo/README 声称 MIT 但仓库无 LICENSE”的失败前置证据；核验本 fork 源码基线、上游当前许可证和第三方归属，确定可发行许可证；Red 创建 `scripts/verify-license.ps1` 要求 LICENSE/NOTICE/Cargo/README 一致，Green 添加并统一文件与元数据；跑 C12 并完成 A/B 审计。权利不明确时本 Step 保持 FAIL 并阻断发布。
- [x] Step 11.4：Red 为纯 `LaunchRoute` 判定和 NSIS 快捷方式矩阵覆盖强更优先、损坏 settings、官方登录、任意 active relay、未应用 Key、全新状态、覆盖清理与回滚；Green 实现单桌面 `Chimera++`、开始菜单管理入口和分流；跑 launcher/installer 回归并完成 A/B 审计。
**Task 11 Gate：** 复核 Step 证据和许可证发布门，完成 `task-11-aggregate` A/B 审计后才勾选 T30。

## Task 12（T31）：移除 About、GitHub 与残余推荐链路

Files：`apps/codex-plus-manager/src/App.tsx`、`apps/codex-plus-manager/src/i18n.ts`、`apps/codex-plus-manager/src/i18n-en.ts`、`tools/i18n-keys.json`、`assets/inject/renderer-inject.js`、`crates/codex-plus-core/src/assets.rs`、`crates/codex-plus-core/tests/cdp_bridge.rs`、`scripts/verify-no-upstream-ads.ps1`、`scripts/test-verify-allowlist.ps1`。Tests：C2、C5、C6、C8。Rollback/停止：日志/诊断不可达、后台 updater 被误删或生产扫描出现非 allowlist GitHub/推广项即停止，只撤销本 Step 的 agent-owned hunks。

- [x] Step 12.1：Red 添加前端/静态契约，拒绝 `about` route/nav/screen、项目主页、Issues、GitHub 和手动更新按钮；Green 删除 About 并把日志/诊断移到维护页；跑 typecheck/build/静态回归并完成 A/B 审计。
- [x] Step 12.2：Red 添加注入资产契约，拒绝 About/提出问题/仓库全局变量；Green 删除注入菜单入口和 `assets.rs` 仓库注入，后台 updater URL 作为精确 allowlist；跑 bridge/assets 回归并完成 A/B 审计。
- [x] Step 12.3：Red 扩展生产扫描器覆盖推荐、赞助、交流群、GitHub UI 和第三方图标引用；Green 清理发现项与 i18n 死键；跑扫描器自测和真实扫描并完成 A/B 审计。
**Task 12 Gate：** 完成 `task-12-aggregate` A/B 审计后才勾选 T31。

## Task 13（T32）：启动自动更新与最低支持版本

Files：`crates/codex-plus-core/src/update.rs`、`crates/codex-plus-core/tests/updater.rs`、`apps/codex-plus-launcher/src/main.rs`、`apps/codex-plus-manager/src-tauri/src/commands.rs`、`apps/codex-plus-manager/src/App.tsx`、`.github/workflows/release-assets.yml`、`scripts/installer/windows/CodexPlusPlus.nsi`、`scripts/installer/macos/package-dmg.sh`。Tests：C1、C3、C4、C5、C6 和 release workflow 合约测试。Rollback/停止：错误资产可能启动、旧版本被破坏、可信 floor 回退或受支持版本断网锁死即停止，只撤销本 Step 的 agent-owned hunks。

- [x] Step 13.1：Red 覆盖 `minimum_supported_version` 缺省、同上游版本边界、跨上游版本（latest `1.2.35-chimera.1` / floor `1.2.34-chimera.3`）、外来通道、非法值、高于 latest；Green 扩展 Release/UpdateCheck 和 release workflow 清单；跑 updater/workflow 回归并完成 A/B 审计。
- [x] Step 13.2：覆盖可信 floor 原子缓存、跨进程单调升高、清单回滚、断网、无缓存和损坏缓存；更新状态独立于用户配置，并纳入 Task 13 聚合 A/B 审计。
- [x] Step 13.3：覆盖无更新、普通自动更新、强制更新、失败继续和重试；纯决策状态机已接入 launcher/manager 阻断 UI，并纳入 Task 13 聚合 A/B 审计。
- [x] Step 13.4：覆盖 Windows `/S` 静默参数、当前进程退出和安装失败保留旧版本；纳入 installer/updater 回归与 Task 13 聚合 A/B 审计。
- [x] Step 13.5：覆盖 macOS 自动下载、DMG 打开和人工确认提示；不声称静默安装，并纳入 Task 13 聚合 A/B 审计。

**Task 13 Gate：** 完成 `task-13-aggregate` A/B 审计后才勾选 T32。

## Task 14（T33）：原创 Chimera++ 图标

Files：`brand/icon/logo.svg`、`brand/icon/PROVENANCE.md`、`scripts/verify-brand-icons.ps1`、`apps/codex-plus-manager/src-tauri/icons/icon.png`、`apps/codex-plus-manager/src-tauri/icons/icon.ico`、`assets/images/codex-plus-plus.png`、`assets/images/codex-plus-plus.ico`、`docs/images/codex-plus-plus.png`、`docs/images/codex-plus-plus.ico` 及 macOS 打包生成的 `.icns` 输入。Tests：C1、C7、C11、Tauri/NSIS 引用检查和多尺寸截图目视。Rollback/停止：任一第三方路径/像素进入工作区、16/32px 不可辨或平台引用缺失即停止，只撤销本 Task 新生成资产。

- [x] Step 14.1（设计验证）：先保存概念尚未确认的失败前置证据和设计验收清单；生成 3 个不导入第三方资产的原创 SVG 概念，记录工具、模型、brief、输入边界和视觉相似性 A/B 审计；由用户确认一个方向。
- [x] Step 14.2：Red 增加主 SVG、尺寸、透明度、资源引用、非旧哈希和全路径清单门禁；Green 从确认 SVG 导出既有 PNG/ICO/macOS 输入并替换全部发行图标位；跑门禁并完成 A/B 审计。
- [x] Step 14.3：在浅/深背景及 16/32/48/512px 目视验证，补 `brand/icon/PROVENANCE.md` 与发行许可记录；完成 A/B 审计。
**Task 14 Gate：** 完成 `task-14-aggregate` A/B 审计后才勾选 T33。

## Task 15（T34）：同步、Release 与远端上线

Files：`.github/workflows/pr-build.yml`、`.github/workflows/release-assets.yml`、`.github/workflows/sync-upstream.yml`、`scripts/sync-upstream.ps1`、`scripts/test-sync-upstream.ps1` 及 workflow 合约测试。Tests：C8、C9、GitHub API branch/rules/workflow/Release 回验、Windows x64 与 macOS x64/arm64 runs。Rollback/停止：remote/branch 不符、checks 不全绿、首发未获确认或匿名资产校验失败即停止，禁止合并/发布；远端写入后只使用新修复提交/PR，不改写历史。

- [x] Step 15.1：Red 补 workflow 合约，要求 required checks、build-first、最低版本字段、首发 `public-release` environment 和不降低 main 保护；Green 修复 workflows/脚本并完成 A/B 审计。
- [ ] Step 15.2（远端验证）：保存“远端 SHA 落后且 checks 失败”的前置证据；取得用户对远端写入与首发的确认后推送已审计提交，以当前 SHA 跑 Windows x64 与 macOS x64/arm64，关闭失败并完成 A/B 审计。
- [ ] Step 15.3（治理验证）：保存 token 缺失/审批策略不可满足的前置证据；配置最小权限 GitHub App/token 和可满足的审批/auto-merge 策略，完成策略读取回验与 A/B 审计。
- [ ] Step 15.4（首发验证）：保存 Release/latest 404 前置证据；首发经受保护环境人工确认，验证 tag、三个构建目标、匿名 latest/asset 和发布回滚；通过后才启用后续正式版本全自动发布，并完成 A/B 审计。
- [ ] Step 15.5（同步验证）：保存远端 sync workflow 未部署的前置证据；手动触发 sync，验证已处理 tag 幂等和冲突 Issue；不发行 upstream main 未发布提交，完成 A/B 审计。

**Task 15 Gate：** 完成 `task-15-aggregate` A/B 审计后才勾选 T34。

## Task 16（T35）：实机与最终验收

Files：已构建安装资产、安装/更新状态、`docs/superpowers/audits/task-11-*` 至 `task-16-*` 与本轮全部 diff。Tests：Windows 安装升级矩阵、macOS x64/arm64 冒烟、C6、C7、C8、C9、C10。Rollback/停止：任何平台安装不可恢复、全量回归失败或宣传/第三方图标残留即不宣布发行就绪；已发布资产有问题时停止 latest 指向并发布递增修复版本，不复用或覆盖既有 tag。

- [ ] Step 16.1（实机验证）：先保存未安装/未升级的失败前置证据；Windows 实机验证全新安装、Codex++ 覆盖升级、单桌面入口、开始菜单管理入口、强制/普通更新、失败回滚和卸载；完成 A/B 审计。
- [ ] Step 16.2（实机验证）：先保存未完成 Gatekeeper 冒烟的失败前置证据；macOS x64/arm64 验证 DMG、旧 App 迁移、Gatekeeper、自动下载与人工确认；完成 A/B 审计。
- [ ] Step 16.3：运行 workspace 全量、前端、branding、i18n、去推广、workflow、格式与差异门禁；独立扫描宣传、交流群、GitHub UI、旧品牌和第三方图标，完成 A/B 审计。
**Task 16 Gate：** 完成 `task-16-aggregate` 最终 A/B 审计后才勾选 T35 并宣布发行就绪。
