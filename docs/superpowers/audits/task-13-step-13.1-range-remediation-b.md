# Task 13 Step 13.1 Numeric Range Remediation Audit B

> Date: 2026-07-12
> Result: **PASS**
> Scope: four-component u64 boundaries, bypass resistance, generator/validator call paths and workflow wiring

## Independence

本审计独立从最终实现、边界和绕过角度复核；未读取、引用或等待任何 range 审计 A 记录，也未与审计 A 沟通。

## Findings

未发现阻断问题。

- `scripts/release-manifest.mjs:8-9` 定义四分量 Chimera 版本格式及精确的 `u64::MAX`。
- `normalizeChimeraVersion` 在 `:19-23` 先把正则捕获的 major、minor、patch、chimera revision 全部转换为 `BigInt`，再统一执行 `part > U64_MAX` 拒绝；等于上限不会被误拒绝。
- 四个分量分别使用 `18446744073709551615` 均被接受，分别使用 `18446744073709551616` 均以 `outside u64 range` 被拒绝。负数、小数、指数、空白、前导零、外来通道和附加 build metadata 仍被既有严格正则拒绝，未发现可绕过到 `BigInt` 比较后的表示法。
- `generateManifest` 经 `resolveMinimumSupportedVersion` 调用同一个 `normalizeChimeraVersion`；`validateManifestFloor` 对 manifest latest/floor 直接调用该函数，在配置 expected floor 时还会再次经 `resolveMinimumSupportedVersion` 校验。生成、普通 validate 和 expected-floor validate 没有分叉规则。
- `.github/workflows/release-assets.yml:771` 的发布生成调用 `--generate`；已有 Release 与新 Release smoke 在 `:250`、`:824` 调用 `--validate-floor`。两条 CLI 都落到上述共享函数。
- Node 端对 chimera revision 也施加 `u64` 上限，即使 Rust `semver` 的 prerelease 内部表示可能更宽，这属于发布端 fail-closed 收紧，并符合本次“四个分量”边界要求。

## Commands And Results

- 四分量边界探针：major/minor/patch/revision 的 `u64::MAX` 接受、`MAX+1` 拒绝 -> **PASS**。
- `generateManifest` 使用 `1.2.34-chimera.18446744073709551615` floor，并由带 expected floor 的 `validateManifestFloor` 复核 -> **PASS**。
- `node scripts/release-manifest.mjs --self-test` -> **PASS**。
- `node --check scripts/release-manifest.mjs` -> **PASS**。
- `cargo test -p codex-plus-core --test updater --locked release_workflow_emits_minimum_supported_version -- --exact` -> **PASS, 1/1**。
- `cargo test -p codex-plus-core --test updater --locked release_manifest_self_test_runs_in_pr_checks -- --exact` -> **PASS, 1/1**。
- `git diff --check` -> **PASS**。

## Residual Risk

- 内置 self-test 的持久化越界样例目前集中在一个超范围 minor；四分量完整矩阵由本次独立探针验证。实现使用统一的 `parts.some(...)`，不存在按分量分支，因此该测试覆盖差异不构成当前阻断，但未来若拆分解析逻辑应把四分量矩阵固化进 self-test。
- 本地没有执行真实 GitHub Release；工作流结论来自 CLI 调用位置、共享调用链和可执行合约测试。

## Decision

**PASS**。原 numeric range 阻断已关闭，可进入 Step 13.1 的后续收口审计。
