# Task 15 Step 15.2 CI License Hash Remediation 独立审计 B

> 日期：2026-07-12
> 范围：`scripts/verify-license.ps1` canonical hash diff、换行/编码边界、内容篡改与 SelfTest fail-closed 能力
> 独立性：未读取或引用审计 A，只按当前 diff、生产读取路径和独立 mutation 复核。
> 结论：**PASS AFTER REMEDIATION**

## Findings

### B1（已关闭）：BOM 读取语义与 SelfTest 策略不一致

生产快照通过 `Get-Content -Raw -Encoding UTF8` 把 `LICENSE` 读成字符串，再对字符串做 canonical hash。初次补救的 SelfTest 直接在已解码字符串前拼接 `[char]0xFEFF` 并要求 mismatch，因此没有覆盖物理 UTF-8 BOM 会在读取时被剥离的真实边界。

独立文件级诊断写入 `EF-BB-BF + 原 LICENSE UTF-8 bytes` 后走与生产相同的 `Get-Content`：

```text
physical-bom-byte-prefix=EF-BB-BF
get-content-preserves-u-feff=False
decoded-equals-original=True
```

物理 BOM 在进入 `Test-LicenseSnapshot` 前已被解码器剥离，因此 canonical hash 与无 BOM 文件相同。补救后策略明确为“单个 leading UTF-8 BOM 是编码等价”，与生产读取行为一致；canonicalization 也使用 `StartsWith(..., StringComparison.Ordinal)` 显式移除已解码字符串开头的一个 U+FEFF，避免文化比较把不可见字符视为 ignorable 并误删首个真实字符。

最终独立边界结果：

```text
physical-efbbbf-findings=0
physical-efbbbf-decoded-equals-original=True
leading-u-feff-findings=0
embedded-u-feff-hash-mismatch=True
double-leading-u-feff-hash-mismatch=True
```

因此只有一个 leading BOM 被视为编码元数据；嵌入式或第二个 U+FEFF 仍属于内容并触发 hash mismatch。`Ordinal -> CurrentCulture` mutation 会使 SelfTest 失败，文化比较回归已被覆盖。

## 已确认边界

- canonicalization 顺序为 CRLF → LF，再 CR → LF；不会删除或折叠普通字符、空白、尾随换行或 Unicode 内容。
- 当前 LICENSE 的 LF、CRLF 和 CR-only 三种语义等价换行均得到期望 hash `8486A10C...F07EF`。
- 普通内容篡改和嵌入式 U+FEFF 都得到不同 hash；生产 `Test-LicenseSnapshot -CheckLicenseHash` 对内容篡改明确报告 `LICENSE SHA-256 mismatch`。
- 单个 leading U+FEFF/物理 `EF-BB-BF` 按明确策略等价；嵌入式和双 leading U+FEFF 均被拒绝。
- UTF-8 重新编码使用无 BOM 的 `Encoding.UTF8.GetBytes(string)` 内容 bytes；未发现换行规范化之外的同义化会放松内容检测。
- 正常 gate 和 SelfTest 当前均通过，CI workflow 同时运行普通 gate 与 `-SelfTest`。

## SelfTest Mutation 复核

初次复核发现 SelfTest 只有 baseline、LF 和 CRLF 正向样例，因此以下两项破坏仍误报 PASS：

```text
hash-disabled-selftest-passes=True
cr-normalization-removed-selftest-passes=True
```

补救加入 CR-only 正向例、独立 LICENSE tamper 负例和 U+FEFF 字符负例后，原 mutation 已关闭：

```text
hash-disabled-selftest-passes=False
cr-normalization-removed-selftest-passes=False
ordinal-removed-selftest-passes=False
```

禁用 hash 时 SelfTest 明确报告内容 mutation 与 U+FEFF mutation 未失败；移除 CR normalization 时明确报告 CR-only canonical hash mismatch。这两项现在是真实 fail-closed，而非结构字符串断言。

## 验证结果

| 验证 | 结果 |
|---|---|
| `pwsh -NoProfile -File scripts/verify-license.ps1` | PASS |
| `pwsh -NoProfile -File scripts/verify-license.ps1 -SelfTest` | PASS |
| canonical LF / CRLF / CR-only | PASS / PASS / PASS |
| LICENSE 内容篡改 | PASS：生产函数报告 hash mismatch |
| hash-disabled SelfTest mutation | PASS：自测返回 false |
| CR-only normalization removal mutation | PASS：自测返回 false |
| 物理 UTF-8 BOM 经过生产 `Get-Content` | PASS：按明确编码等价策略接受 |
| embedded / double-leading U+FEFF | PASS：均命中 hash mismatch |
| Ordinal → CurrentCulture mutation | PASS：自测返回 false |
| `cargo test -p codex-plus-core --test installers --locked` | PASS，28 tests |
| `git diff --check -- scripts/verify-license.ps1 ...` | PASS；仅现有 LF/CRLF 转换警告 |

## Gate

canonical hash 只规范化明确允许的三种换行与单个 leading BOM；普通内容、嵌入式/重复 BOM 和尾部篡改仍改变 hash。禁用 hash、移除 CR-only normalization 和恢复文化比较均会让 SelfTest 失败，CI 继续同时运行正常 gate 与 SelfTest。**本独立审计 B 最终结论为 PASS AFTER REMEDIATION。**
