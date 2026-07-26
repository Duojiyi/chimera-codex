# Task 4 (T44) Audit A — Spec/行为覆盖 (v2)

**Date:** 2026-07-26
**Scope:** Step 4.1–4.7（Chimera Codex Mirror 与 stable 兼容门），commit `323d9da`
**Auditor:** Independent A（只看 Spec/行为覆盖，不看 diff/边界）

## 方法

对照 Plan `Task 4（T44）` 与 Spec 第 9 节的逐条 Step 要求，逐一检查代码是否存在、是否可观察、测试是否覆盖。已实际运行：

```
cargo test -p mirror-contract --locked   # 11 passed
node scripts/test-mirror-contract.mjs    # PASS（无 fixture，仅内联结构检查）
```

## Step 逐条核查

### Step 4.1 — 冻结 raw manifest schema；Windows Store/FE3 与 macOS appcast 探测 fixture；原始对象按 digest 不可变

Status: **PARTIAL**

- `services/mirror-contract/src/manifest.rs:6-28` 定义了 `MirrorManifest`，字段基本对齐 Spec 9.2，但缺失 Spec 9.2 明确列出的 `capability_manifest_url / size / sha256` 三个字段——`MirrorManifest` 结构体里完全没有这三个字段，`scripts/test-mirror-contract.mjs:22-27` 的 `REQUIRED_MANIFEST_FIELDS` 也没有校验它们。这意味着 stable manifest 本身不携带指向 capability manifest 的可验证指针，Step 4.5 要求的"绑定到同一 CAS 事务"缺少落点。
- Spec 9.2 还要求"manifest 必须经过 Chimera 离线公钥可验证的签名"。`MirrorManifest` 没有任何 `signature` 字段，仓库里也搜不到任何签名生成/验证代码（`grep -r "ed25519\|Signature\|verify_signature"` 均无结果）。当前只有内容 hash（`sha256`）和 schema 校验，没有对 manifest 本体的签名验证——这是 Spec 明确写出的硬性要求，未实现。
- "Windows Store/FE3 与 macOS official appcast 探测的 fixture tests" 完全不存在：仓库里没有任何探测官方发布源（winget/FE3/Sparkle appcast）的代码或 fixture，`tests/mirror_contract.rs` 里的 11 个测试全部是对内存中手工构造的 `MirrorManifest`/`StablePointer`/`CapabilityManifest` 做 schema/roundtrip/CAS 校验，不涉及任何探测逻辑。
- "原始对象按 digest 不可变保存"——没有对象存储/CAS 存储层代码，只有 in-memory 类型定义，无法验证不可变性。

结论：Step 4.1 只完成了"manifest 字段 schema + 内容 hash 校验"这一小部分，探测 fixture、签名验证、不可变对象存储均缺失。

### Step 4.2 — SHA256SUMS、来源证明、Windows Authenticode / macOS Sparkle 与 Team ID 元数据验证；不重签官方 payload

Status: **MISSING**

- `OfficialIdentity { signer, subject, team_id }`（`manifest.rs:30-35`）只是数据结构，没有任何验证函数读取真实 Authenticode 证书链或 Sparkle 签名。`source_provenance`（`manifest.rs:45-50`）同样只是数据结构，没有代码去比对 SHA256SUMS 或校验来源 URL 的真实性。
- 没有任何测试或代码路径实际下载/校验一个官方 payload。

### Step 4.3 — raw 发布幂等、双对象存储同步、地区路由、匿名下载校验

Status: **MISSING**

- 仓库内无对象存储客户端代码、无幂等发布逻辑、无地区路由逻辑。`services/mirror-contract` 只包含类型和纯函数，没有任何 I/O 或网络代码（`Cargo.toml` 依赖里没有 HTTP client）。

### Step 4.4 — TUF 风格 offline root/online targets、threshold/expiry/version、连续轮换和吊销；signed stable manifest、CAS 推广、回撤、保留策略、candidate 灰度

Status: **PARTIAL（仅 CAS 反回滚一项）**

- `services/mirror-contract/src/cas.rs:29-41` 的 `validate_stable_promotion` 确实实现了"较低或相同 sequence 被拒绝"（防回滚/防旧 workflow 覆盖新 stable），并有 3 个针对性测试（`higher_sequence_is_accepted`、`same_sequence_is_rejected_as_stale`、`lower_sequence_is_rejected_prevents_rollback_attack`，均通过）。`verify_manifest_digest`（`cas.rs:44-55`）校验指针记录的 digest 与实际 manifest digest 一致。
- 但 TUF 风格的 root/targets 职责拆分、threshold 签名、metadata 有效期、根证书连续轮换、吊销流程——ADR-003、ADR-006 里描述的整套信任链——在代码里完全不存在。CAS 只是一个整数比较，不是 TUF 意义上的签名验证系统。
- "保留策略"（至少保留当前 stable、上一 stable、客户包内置版本）没有任何代码体现；`retain`/`retention` 关键字在仓库中搜不到对应实现。
- "candidate 灰度"——`channel` 字段允许 `"candidate"` 值（脚本里有校验），但没有任何灰度推送/订阅逻辑。

结论：Step 4.4 的验收标准里只有"防回滚 CAS"这一项被证明落地，其余（TUF 信任链、保留策略、候选灰度推送）均缺失。

### Step 4.5 — capability 发布 schema、canonical serialization、兼容规则；镜像 gate 唯一生产者、digest 绑定

Status: **PARTIAL**

- `services/mirror-contract/src/capability.rs:6-14` 定义了 `CapabilityManifest`，`matches_digest`（32-35 行）验证 `bound_raw_digest` 与给定 raw digest 是否一致，两个测试通过（`capability_matches_bound_digest`、`capability_manifest_roundtrips_json`）。
- 但"canonical serialization"（跨实现确定性字节序列，用于签名）没有实现——测试里用的是 `serde_json::to_string_pretty`，这不是规范化序列化（字段顺序、空格、转义规则没有被固定为契约的一部分，只是 Rust 结构体字段声明顺序的副产品）。
- "镜像 gate 是唯一生成/签名/发布者，只接受精确 raw digest 在其声明支持平台/架构上的探针证据，并在同一 CAS 事务中把 capability digest 绑定到 stable manifest"——这一整套流程性约束没有代码体现：`MirrorManifest` 缺少 `capability_manifest_url/size/sha256` 字段（见 Step 4.1），`validate_stable_promotion` 也不检查 capability 绑定，所以"同一 CAS 事务绑定"目前无法验证是否成立。

### Step 4.6 — PR/fork 只跑无 secret mock；真实 ChimeraHub smoke 仅在审批保护 Environment 运行

Status: **MISSING（就 T44 范围而言）**

- `.github/workflows/v2-build.yml` 里的 `supply-chain` job 确实只运行 `node scripts/test-mirror-contract.mjs`，该脚本不发起任何网络请求、不读取任何 secret（已核实脚本源码，只有内联对象校验和可选 fixture 目录扫描）。就"PR 不接触 secret"这一半而言是满足的。
- 但"真实 ChimeraHub smoke 仅在审批保护 Environment 或手动 release job 中运行"——没有任何这样的 workflow 存在（`.github/workflows/` 目录下搜不到对应 job），跳过条件、脱敏、轮换、吊销演练均未见落地。这部分要求实质上尚未开始。

### Step 4.7 — 完成镜像仓库和客户端消费者的独立 A/B 审计；未授权远端资源时以本地 fixture/CI dry-run 保持未完成

Status: **本文档本身即该审计的 A 面**

- 依据 Plan 执行规则 8："计划中的远端仓库创建、secret、域名、CDN、签名和公开 Release 都需要单独授权"。`Duojiyi/chimera-codex-mirror` 仓库尚未被授权创建（Spec 目标仓库标注"待授权"），这与 Task 0 hard gate 一致（`docs/architecture/decisions/ADR-003-raw-stable-dual-channel-tuf.md:36`、`ADR-006-independent-tuf-roots.md:42` 都把密钥轮换/吊销演练标记为"Release Gate R4"前置条件）。
- `services/mirror-contract/src/lib.rs:2` 的注释也明确写着："The actual mirror deployment requires Release Gate R4 authorization. This crate provides the contract layer"——代码作者自己承认这只是"contract layer"，不是完整 Task 4 交付。

## 结论

**FAIL —— 不满足 Task 4（T44）勾选标准。**

已验证真实存在并测试通过的部分：manifest/CAS/capability 三个模块的类型定义、schema 校验（V9 脚本）、CAS 反回滚逻辑（11/11 Rust 测试 + V9 脚本 PASS）。

但 Plan 中 Step 4.1–4.7 的绝大多数验收标准未被满足：

| Step | 状态 | 缺失核心项 |
|---|---|---|
| 4.1 | PARTIAL | 官方源探测 fixture、manifest 签名、不可变对象存储 |
| 4.2 | MISSING | 真实 Authenticode/Sparkle/SHA256SUMS 验证 |
| 4.3 | MISSING | 幂等发布、双存储同步、地区路由 |
| 4.4 | PARTIAL | 仅 CAS 反回滚；TUF 信任链、保留策略、灰度推送均缺失 |
| 4.5 | PARTIAL | canonical serialization、CAS 内绑定校验缺失 |
| 4.6 | MISSING | 审批保护 Environment 的真实 smoke workflow |
| 4.7 | OPEN | 远端镜像仓库未授权，客户端消费者审计无法完成 |

这与"提示中给出的已知真实差距"一致：镜像基础设施（域名/对象存储/签名密钥）未建立且被 Release Gate R4 阻断；mirror-contract 没有真正的密码学签名验证，只有 hash + schema。此外本次审计还发现原提示未提及的两项具体缺口：`capability_manifest_url/size/sha256` 字段在 manifest schema 和 V9 脚本中均缺失，以及 manifest 本体缺少 Spec 9.2 明确要求的签名字段。

不建议勾选 T44；建议将当前交付记录为"Step 4.1/4.4/4.5 的地基契约层已完成"，其余 Step 待后续提交补齐后再次审计。
