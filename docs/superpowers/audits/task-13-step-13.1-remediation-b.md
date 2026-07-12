# Task 13 Step 13.1 Remediation Audit B

> Date: 2026-07-12
> Result: **FAIL**
> Scope: final diff, Node/Rust version-rule parity, workflow wiring, Manager payload propagation, idempotent Release paths and regression surface

## Independence

本审计独立按最终 diff、边界和回归面执行；未读取、引用或等待补救审计 A，也未与审计 A 沟通。只读核对了规格、Plan、原始/补救 evidence 和原始 Step 审计历史。

## Blocking Finding

### B1. Node 生成器接受 Rust 客户端无法解析的超范围版本分量

- `scripts/release-manifest.mjs:8-21` 用正则接受任意长度十进制分量，并转换为无上限的 JavaScript `BigInt`；没有把 major/minor/patch 限制到 Rust `semver::Version` 使用的 `u64` 范围。
- `crates/codex-plus-core/src/update.rs:68` 则通过 `semver::Version::parse` 解析 latest 与 floor。超出 `u64` 的 major/minor/patch 会解析失败，`release_from_latest_json_payload` 因而拒绝整个清单。
- 可复现输入：latest 为 `1.2.35-chimera.1`、`MINIMUM_SUPPORTED_VERSION=0.18446744073709551616.0-chimera.1`。Node 的 `resolveMinimumSupportedVersion` 接受该 floor，因为 major `0 < 1`，并会将它写入 `minimum_supported_version`；Rust 客户端不能解析其中超过 `u64::MAX` 的 minor。
- `.github/workflows/release-assets.yml:771` 的实际发布路径直接调用该生成器，两个 smoke 路径在 `:250`、`:824` 也复用同一 Node 校验器，因此均无法发现这个跨语言不一致。配置该值可发布一个所有 Rust 客户端都无法消费的 `latest.json`，违反 Step 13.1 的“非法值拒绝”以及发布/运行时规则一致性要求。

修复门槛：先增加会失败的 Node 合约测试，覆盖至少一个“数值顺序低于 latest、但任一 core 分量大于 `u64::MAX`”的 floor；再让 Node 规则与 Rust 对齐并跑完整 Step 回归。也可以建立单一共享规则来源，但发布端必须在上传前 fail closed。

## Verified Passing Areas

- 缺省 floor、同上游边界、跨上游 floor、外来通道、普通非法值和 floor 高于 latest 已有 Rust/Node 覆盖。
- workflow 在 PR/Release gate 执行 Node self-test，在生成路径调用 `--generate`，并在新发布和已有发布 smoke 路径调用 `--validate-floor`。
- 已发布 Release 路径只做结构/顺序校验、不强行套用后来变化的仓库 floor，符合不可变 Release 的幂等复核语义；draft/tag-only/missing 路径仍绑定已解析的目标 SHA。
- Manager check payload 在成功时传播 floor、失败时返回 `null`；安装重新获取失败返回 `null`，下载/启动成功或失败均传播可信 Release 的 floor。前端 `UpdateResult` 声明 nullable 可选字段，未发现本次补救引入的字段丢失。

## Commands And Results

- `node scripts/release-manifest.mjs --self-test` -> **PASS**。
- `node --check scripts/release-manifest.mjs` -> **PASS**。
- `cargo test -p codex-plus-core --test updater --locked` -> **PASS, 42/42**。
- `cargo test -p codex-plus-manager --test windows_subsystem --locked manager_update_install_keeps_visible_progress_bar -- --exact` -> **PASS, 1/1**（41 filtered out）。
- `cargo fmt --all -- --check` -> **PASS**。
- `git diff --check` -> **PASS**；仅报告工作树既有的 LF/CRLF 转换警告。
- 只读 Node 边界探针：`resolveMinimumSupportedVersion('1.2.35-chimera.1', '0.18446744073709551616.0-chimera.1')` -> **被接受**，确认 B1。

## Residual Risk

- GitHub Actions 的真实 draft/resume/published 流程未在本机执行；当前判断基于 workflow 控制流、可执行 helper 和静态/单元回归。
- Manager 契约测试仍以源码静态断言为主，未直接调用 Tauri command 后反序列化每一种 payload；最终代码路径已人工逐支核对，但后续状态机接线仍应补行为级测试。

## Decision

**FAIL**。B1 未关闭前不得把 Step 13.1 标记完成，也不得依赖该清单进入最低支持版本缓存或强制更新逻辑。
