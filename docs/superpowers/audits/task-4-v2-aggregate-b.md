# Task 4 (T44) Audit B — Diff/边界/失败恢复 (v2)

**Date:** 2026-07-26
**Scope:** Step 4.1–4.7，commit `323d9da`
**Auditor:** Independent B（只看 diff/边界/失败恢复，不看 Spec 覆盖清单）

## 方法

独立于 A 面，直接审查 diff 内容、边界条件和失败恢复路径。已实际运行：

```
cargo test -p mirror-contract --locked   # 11 passed, 0 failed
node scripts/test-mirror-contract.mjs    # PASS
```

逐文件读取 `services/mirror-contract/src/{manifest,cas,capability}.rs`、`tests/mirror_contract.rs`、`scripts/test-mirror-contract.mjs`。

## Diff 检查

- `services/mirror-contract/Cargo.toml`：依赖 `serde`、`serde_json`、`thiserror`、`url`、`sha2`、`semver`、`chimera-domain`。核实后发现 **`url`、`semver`、`chimera-domain` 三个依赖在 `src/` 下完全未被引用**（`grep -rn "url::\|semver::\|chimera_domain::" services/mirror-contract/src` 无匹配）。这是死依赖，不是 bug，但说明当前实现比 Cargo.toml 暗示的范围小——真实的 URL 校验和 domain 类型复用都没有发生，manifest 里的 `asset_url` 字段是裸 `String`，没有用 `url::Url` 做结构化校验。
- `mirror_contract.rs` 测试文件里所有测试都是纯内存对象操作，没有一个测试涉及文件 I/O、网络、进程间通信或时间依赖，说明这套测试对"真实失败模式"（网络中断、磁盘满、并发）零覆盖，只覆盖"给定内存中已构造好的值，函数逻辑对不对"。

## 边界条件核查

### 1. Manifest schema 边界

- `MirrorManifest::is_stable_compatible()`（`manifest.rs:53-55`）：`channel == "stable" && compatibility_status == Compatible`。`channel` 是裸 `String`，不是枚举——`CompatibilityStatus` 有类型但 `channel` 没有。测试和 V9 脚本都是手工传入 `"stable"`/`"raw"`/`"candidate"` 字符串常量，代码层面完全没有防止拼写错误（如 `"Stable"` 大小写不一致）导致的静默不匹配。V9 脚本在 `channel is raw|stable|candidate` 检查里用了大小写敏感的 `includes()`，Rust 侧 `is_stable_compatible()` 同样大小写敏感——两者一致，但都是脆弱的字符串比较而非类型安全枚举，是潜在的一致性风险点（未发现实际 bug，但架构脆弱）。
- V9 脚本第 41-52 行的 raw/stable compatibility_status 互斥检查，只在 JSON fixture 路径执行；对 Rust 端传入的 `CompatibilityStatus::Incompatible { reason }` 变体，脚本侧的 JS 校验逻辑（`cs === "compatible" || cs?.status === "compatible"`）需要处理 Rust `#[serde(rename_all="snake_case")]` 序列化后的 tagged enum 形状。我验证了实际序列化结果：

```
cargo test 输出确认 CompatibilityStatus::Incompatible{reason} 通过 serde_json 序列化，
且脚本的 cs?.status 分支专门为此形状设计
```

这一处理是对的，脚本正确覆盖了 tagged-enum 的 JSON 形状,不是缺陷。

### 2. CAS 反回滚边界

- `validate_stable_promotion`（`cas.rs:29-41`）：`proposed.sequence <= current.sequence` 触发拒绝。测试覆盖了三种边界：`current+1`（accept）、`current==proposed`（reject as stale）、`current > proposed`（reject，防回滚攻击）。**边界值本身覆盖完整**。
- 但 `StablePointer.sequence` 类型是裸 `u64`，没有测试覆盖 `u64::MAX` 溢出场景（下一次推广 `sequence = u64::MAX + 1` 会 panic 而不是优雅拒绝）。这是理论边界，实际不会在可预见时间内触发，标记为低优先级 gap 而非阻断项。
- `verify_manifest_digest`：只做字符串相等比较，不做 digest 格式校验（例如 `pointer.manifest_digest` 是空字符串时，`""== ""` 会通过——测试没有覆盖"两边都是空/占位字符串"这种误报通过的场景）。这是一个真实的边界缺口：如果调用方忘记填充 digest 字段，函数会静默"验证通过"。

### 3. Capability 绑定边界

- `matches_digest`（`capability.rs:32-35`）：同样是字符串相等比较，同样没有测试覆盖"两边都是空字符串"的误报通过场景。
- 更重要的是：**没有任何测试或代码把 CAS 推广（`cas.rs`）和 capability 绑定（`capability.rs`）串联起来验证**。Spec 9.3 要求"在同一 CAS 事务中把 capability digest 绑定到 stable manifest"，但代码里 `validate_stable_promotion` 完全不知道 `CapabilityManifest` 的存在，两个模块之间没有任何调用关系或共享状态检查。如果未来有人推广一个 stable 版本但忘记同时绑定/更新 capability manifest，当前代码**不会报错**——没有任何守卫检测这种不一致。这是本次审计发现的最重要的架构缺口。

### 4. V9 脚本边界

- `validateManifest`（`test-mirror-contract.mjs:29-53`）对 `compatibility_status` 的判断用了 `cs === "compatible" || cs?.status === "compatible"`，兼容字符串和 tagged-object 两种形状。但如果 `compatibility_status` 字段整体缺失（`undefined`），`cs?.status` 会是 `undefined`，`isCompat` 为 `false`——对于 `channel==="raw"` 场景这会误判"未设置"等同于"合规的未 compatible"，通过检查；对 `channel==="stable"` 场景则会正确拒绝。这个不对称是合理的（stable 缺字段应该拒绝，raw 缺字段不应该拒绝），但脚本里没有测试用例明确验证"字段完全缺失"这一路径，只验证了"字段存在但值错误"的路径（`REQUIRED_MANIFEST_FIELDS` 循环会先单独报"缺字段"错误，所以实践中不会被漏判掉——核实后确认二者叠加时行为正确，非缺陷）。
- `validateCasSequence`（56-67 行）：`pointers.length < 2` 时直接 `return`，即"只有 0 或 1 个指针时不做任何单调性检查"。这是合理的默认（没有历史无法比较），但也意味着 V9 脚本本身**不能单独检测"第一次发布的 sequence 是否从 1 或某个约定值开始"**——如果第一次发布错误地用了 `sequence: 999999`，脚本不会报错。这是脚本层面的边界盲点，纯 Rust 单元测试同样没有覆盖"首次发布 sequence 起始值"这个场景。

## 失败恢复路径核查（按提示要求逐项探测）

以下按用户提示里列出的 Task 4/5 通用失败场景模板，逐项确认 Task 4 范围内是否适用及是否覆盖：

1. **下载中途中断** —— 不适用于 Task 4（mirror-contract 不做下载，是 schema/CAS 契约层）；无代码，无测试，标记为 N/A。
2. **进程在 stage 和 commit 之间死亡** —— 不适用；mirror-contract 没有 stage/commit 状态机（那是 Task 5 的职责）。
3. **current.json 损坏或缺失** —— 不适用于 mirror-contract；该文件属于 Task 5 `chimera-runtime`。
4. **无前一版本时尝试回滚** —— CAS 层面对应的是"没有 current pointer 时的首次推广"，前面第 4 点已指出该场景缺乏专门测试（sequence 起始值未强制）。
5. **两次更新竞争（race）** —— **完全未覆盖**。`validate_stable_promotion` 是纯函数，不做任何锁或原子性保证；如果两个 CI workflow 同时读到 `current.sequence=5` 并各自算出 `proposed.sequence=6`，两者都会通过校验——真正的 compare-and-swap 需要底层存储的原子写（如对象存储的条件 PUT），但 mirror-contract 只提供了"事后校验函数"，没有提供任何原子性保证的调用约定或文档说明谁负责加锁。这是一个明确的 gap：Spec 9.4 写"stable 推广使用 compare-and-swap，防止较旧 workflow 覆盖较新 stable"，但当前实现的 CAS 只是"两个数字比较"，真正的原子交换（read-modify-write 的原子性）完全依赖尚未存在的对象存储层，没有测试能证明并发场景下不会产生 TOCTOU 竞争。
6. **ownership manifest 指向的路径不再匹配** —— 不适用于 Task 4（这是 Task 5 `detect_runtime` 的职责，已在该 crate 验证）。

## 安全/密钥边界

- 复核用户提示中的已知差距：mirror-contract 中确认**没有任何密码学签名验证代码**（未搜到 `ed25519`、`rsa`、`Signature`、`verify_signature` 等关键字任何匹配）。只有 `sha2` 用于内容 hash，`Cargo.toml` 里也没有引入任何签名库。这与 Spec 9.2"manifest 必须经过 Chimera 离线公钥可验证的签名"要求存在实质差距——当前 CAS + hash 机制可以防止"内容被篡改后 hash 仍匹配"这种情况被检测出来（因为会重新计算 hash 发现不一致），但**不能防止"攻击者伪造一个新 manifest + 新 hash + 递增的 sequence 号"这种主动攻击**，因为没有任何东西证明 manifest 来自受信的 Chimera 发布流程。这是一个真实的安全边界缺口，不是护栏，是信任假设的缺失。
- `.github/workflows/v2-build.yml` 的 `supply-chain` job 已核实只运行 `verify-license.mjs`、`verify-no-secrets.mjs`、`test-mirror-contract.mjs`、`verify-v2-architecture.mjs`、`verify-design-tokens.mjs`，均为无 secret 的静态检查，没有触碰任何真实凭据。这一点符合 Step 4.6 对 PR 侧的要求。

## 结论

**FAIL —— 存在未被任何测试覆盖的并发/信任边界缺口，且真实的密码学信任链完全缺失。**

关键发现（供 A 面参考，独立得出）：

1. CAS 推广与 capability 绑定之间没有任何一致性守卫——可以推广 stable 而不更新 capability，代码不会报错。
2. `verify_manifest_digest` 与 `matches_digest` 都是裸字符串相等比较，"两边都为空"会静默通过，无测试覆盖这一误报路径。
3. Compare-and-swap 的原子性完全没有实现或测试——当前只有"事后数值比较"，真正的并发安全依赖不存在的存储层，Spec 明确要求的"旧 workflow 竞争"场景未被验证。
4. 没有任何签名验证代码，`url`/`semver`/`chimera-domain` 三个声明依赖在 `src/` 下未被使用，说明契约层比 Cargo.toml 暗示的更薄。

这些发现和 A 面独立得出的"Step 4.1-4.7 大部分验收标准未满足"结论相互印证但角度不同：A 面看的是"Spec 条款是否被满足"，B 面看的是"现有代码在边界和并发下是否安全"，两者都指向同一个事实——当前提交只是一层契约骨架，真正的镜像基础设施、签名信任链、并发安全保证都还不存在，这与 Release Gate R4 未授权的状态一致，不应视为 Task 4 完成。
