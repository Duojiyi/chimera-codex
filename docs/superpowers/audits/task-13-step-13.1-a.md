# Task 13 Step 13.1 Audit A

## Independent requirements and behavior audit

结论：**FAIL**。

本轮只按产品规格、Plan Step 13.1、TDD evidence、当前测试和可观察清单/更新检查行为独立复核；未读取、引用或等待审计 B，也未与审计 B 沟通。Rust core 的版本解析和字段传递已满足大部分边界，但 Manager 边界仍丢失 floor，release workflow 的生成契约也没有被可执行测试或发布 smoke 证明，因此 Step 13.1 尚不能勾选。

## Blocking findings

### A1. `minimum_supported_version` 未传播到 Manager 的可观察 `UpdateResult`

- `crates/codex-plus-core/src/update.rs:31-50` 已在 `Release` 和 `UpdateCheck` 增加 nullable 字段，`update_check_from_release` 也在 `:382-385` 从 `Release` 传入 `UpdateCheck`。
- 但 `apps/codex-plus-manager/src-tauri/src/commands.rs:1702-1711` 手工构造对外 JSON 时未输出 `minimumSupportedVersion`，失败 payload `:1717-1727` 同样没有该字段。
- `apps/codex-plus-manager/src/App.tsx:538-547` 的 `UpdateResult` 类型也没有 `minimumSupportedVersion`。因此 core 中的字段在实际 Manager command 边界被静默丢弃，前端无法观察或消费可信 floor。
- 当前 updater 测试只把 core `UpdateCheck` 直接序列化，无法发现这条生产传播链断裂。应先增加失败的 Manager command/type 合约测试，再补 success/failure payload 和 TypeScript 字段；若产品明确把 Manager 接线全部留到 Step 13.3，则 Plan/evidence 需要把 Step 13.1 的“传播”边界限定为 core，而不能宣称当前生产 UpdateResult 已传播。

### A2. release workflow 生成契约未被行为测试，且发布 smoke 接受缺失/错误 floor

- `crates/codex-plus-core/tests/updater.rs:226-234` 的 `release_workflow_emits_minimum_supported_version` 只对 workflow 源码做五次 `contains`。它不执行 Node 生成逻辑，不验证缺省值、同上游/跨上游 floor、外来/非法值或 floor 高于 latest 的 workflow 行为；相关字符串即使位于死代码、注释或彼此不相干的表达式中，测试仍会通过。
- `.github/workflows/release-assets.yml:774-795` 的当前实现表面上有缺省和顺序校验，`:817-819` 也写出字段，但没有可执行的 workflow/helper 合约测试证明这些行为。Rust parser 测试不能替代另一份 JavaScript 实现的测试。
- workflow 的“已有 Release”匿名验证 `:245-289` 和“新 Release”匿名验证 `:885-943` 都只检查 `version` 与 assets；两处 `jq` 均不检查 `minimum_supported_version` 的存在、通道合法性或 `minimum <= version`。所以发布后的 `latest.json` 即使缺失 floor、使用外来通道或高于 latest，发行 smoke 仍会通过。
- 应先用 Red 测试覆盖实际 manifest 生成器的默认/配置/非法/越界输入，再把生成逻辑抽成可直接执行的 helper 或等价可测试单元；新发布 smoke 至少应按本次期望 floor 做精确断言，已有 Release 的幂等验证也应 fail closed 校验字段存在、格式合法且不高于 latest。

## Verified Rust behavior

- 缺失 `minimum_supported_version` 向后兼容为 `None`，并进入 `Release`/core `UpdateCheck`。
- 同上游相等边界 `1.2.34-chimera.3 / 1.2.34-chimera.3` 通过。
- 跨上游 `latest 1.2.35-chimera.1 / floor 1.2.34-chimera.3` 通过。
- 无 Chimera 后缀、beta/外来通道、畸形版本和非字符串值均被拒绝；floor 高于 latest 被拒绝。
- 合法 floor 经 semver 解析后规范化为不带 `v` 的字符串，并由 core `Release` 传到 core `UpdateCheck`。

## TDD evidence review

- `task-13-step-13.1-evidence.md` 记录了单一 Red 命令：41 个 updater 测试中原有 36 个通过，新加 5 个全部按预期失败；随后同一命令 Green 为 41/41。该记录能说明 core 测试先行。
- 现有五个新测试确实覆盖 core 缺省、同/跨上游、外来/非法值、floor 高于 latest，以及源码中存在 workflow 字段的最小断言。
- evidence 将 `release_workflow_emits_minimum_supported_version` 称为 workflow contract，但它只有源码字符串断言，未覆盖 workflow 的可观察生成行为；也没有 Manager 对外传播的 Red 证据。严格 TDD 门禁因此尚未闭合。

## Commands and evidence

- `cargo test -p codex-plus-core --test updater --locked`: **PASS，41/41**。
- `cargo fmt --all -- --check`: **PASS**。
- `git diff --check -- crates/codex-plus-core/src/update.rs crates/codex-plus-core/tests/updater.rs .github/workflows/release-assets.yml`: **PASS**；仅出现现有 LF/CRLF 转换警告。
- `rg -n -C 8 "UpdateCheck|check_for_update|latest_version|update_available" ...`: 确认 core 字段存在，但 Manager success/failure payload 与前端类型均无 `minimumSupportedVersion`。
- `rg -n -C 10 "validate_latest_manifest|minimum_supported_version|latest-smoke" .github/workflows/release-assets.yml`: 确认生成段写出字段，但两份 `validate_latest_manifest` 均未验证该字段。

## Residual risks

- 本轮未运行真实 GitHub Actions；即使补齐本地可执行合约测试，workflow YAML、repository variable 和真实 Release 仍需后续远端 Gate 验证。
- Rust 测试使用不带 `v` 的 fixture，而 workflow 实际输出 `v...`；当前 parser 可接受并规范化 floor，但应在修复合约测试时加入真实 workflow 形状，避免两端格式漂移。
- 可信 floor 缓存、单调升高/回滚/断网策略属于 Step 13.2；普通/强制更新决策及 launcher/manager 阻断 UI 属于 Step 13.3。本次 FAIL 不要求提前实现这些后续行为。

## Gate decision

Step 13.1 **不得勾选**。关闭 A1、A2 后需要重新保留 Red/Green/针对性回归证据，并再次执行相互独立的 A/B 审计。
