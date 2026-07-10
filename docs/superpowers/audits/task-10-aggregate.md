# Task 10 Aggregate Audit — Full Verification Gate

> Status: passed for automated gates; first public Release deferred to user push decision
> Date: 2026-07-10
> Scope: Task 10 Steps 1–6（自动化全量验证；手工冒烟与首次公开 Release 待推送后）

## Evidence Ledger

| Area | Evidence | Result |
|------|----------|--------|
| Branding check | `generate-branding.ps1 -Check` PASS（含 CRLF 归一化） | pass |
| Ads scan | `verify-no-upstream-ads.ps1` OK | pass |
| Format | `cargo fmt --check` | pass |
| Rust tests | `cargo test -p codex-plus-core -p codex-plus-manager` | pass |
| Frontend | `npm run check` (tsc) | pass |
| Sync dry-run | `sync-upstream.ps1 -DryRun` exit 0，幂等 noop | pass |
| Watcher fixtures | 对齐 OpenAI.Codex 包名（非 ChatGPT-Desktop） | pass |
| Settings test isolation | `set_settings_path_for_tests` → thread-local | pass |
| Workflow/packager tests | ChimeraCodex + build-first latest.json 断言 | pass |

## Independent Audit A — Requirements

自动化门禁覆盖 Plan Task 10 Step 1。手工 Windows/macOS 安装冒烟与首次公开 Release（Step 2–5）依赖推送与真实资产，按用户要求保留给推送决策后执行。T29 / 部分 V* 保持未勾选。

## Independent Audit B — Implementation / Safety

无密钥写入；未 push；未创建公开 Release；upstream push 仍阻断。生成文件 EOL 门禁已加固。

## Deferred Gates (user push / token)

- T29 首次公开 Release 匿名下载
- D11 `CHIMERA_AUTOMATION_TOKEN` / Actions 写权限最小集
- T28 真实冲突 Issue 演练
- V1–V10 / V14 需安装包与远端 Release 的手工/CI 冒烟

## Decision

Task 1–9 产品与 CI 代码已完成本地提交；Task 10 自动化验证通过。**请用户决定是否推送**；推送并配置 token 后再做首次公开 Release。
