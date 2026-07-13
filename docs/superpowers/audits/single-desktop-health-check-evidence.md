# Single Desktop Health Check Evidence

## Scope

- Windows 桌面继续只保留一个 `Chimera++.lnk`，其产品语义为管理工具主入口。
- 健康检查不再要求已废弃的第二个桌面 `Chimera++ 管理工具.lnk`。
- macOS 继续分别检查 `Chimera++.app` 和 `Chimera++ 管理工具.app`。
- 补丁版本为 `1.2.35-chimera.4`，macOS build number 为 `6`。

## TDD Evidence

### Initial Red / Green

- Red: `node --test apps/codex-plus-manager/src/entrypoint-health.test.ts` 因缺少 `entrypoint-health.ts` 失败。
- Green: 后端在 Windows 将桌面主入口映射为管理入口；前端相同入口只显示一行，macOS 独立入口仍显示两行。

### Audit Remediation Red / Green

- Red: 同一路径但状态不一致的测试只返回一行；持久化 workflow 契约同时证明 `npm test` 未进入 PR/Release gate。
- Green: 去重增加状态边界；`package.json` 提供确定性 `npm test`；PR 和 Release 综合 gate 均在 typecheck 后、production build 前执行该测试。
- Red: Windows 单入口在候选路径为 `null` 时仍显示两行。
- Green: `OverviewPayload.single_entrypoint` 在成功路径和后台失败 fallback 中显式设置；前端不再从 path 推断平台。

## Local Regression

- `npm test --prefix apps/codex-plus-manager`: 15 passed.
- `npm run check --prefix apps/codex-plus-manager`: PASS.
- `node tools/i18n-verify.mjs`: plain 580/580, template 43/43, manifest exact PASS.
- `scripts/generate-branding.ps1 -Check`: PASS.
- `scripts/verify-no-upstream-ads.ps1`: PASS.
- `cargo metadata --locked --no-deps --format-version 1`: PASS.
- `cargo fmt --all -- --check`: PASS.
- `git diff --check`: PASS.

按用户要求未在本地运行完整 Cargo tests、NSIS、Tauri 或平台安装包构建；这些由 GitHub required checks 验证。

## Gate

独立审计 A、B 在所有整改完成后均为 PASS。Windows/macOS 实机安装、快捷方式目标、状态刷新和双 App 展示仍属于未完成的真实平台验收，不在此记录中宣称完成。
