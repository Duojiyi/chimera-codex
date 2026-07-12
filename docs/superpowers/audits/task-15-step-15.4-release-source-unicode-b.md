# Task 15 Step 15.4 Release Source Unicode 独立审计 B

> 日期：2026-07-13
> 分支：`codex/release-source-unicode`
> 范围：最终 diff、Git path quoting、Bash/tar 管道、两个 source-tree 校验点、mutation 有效性与回归面
> 独立性：仅按当前 diff、源码、Git/tar 行为和本审计复跑结果检查
> 结论：**FAIL**

## Findings

### B1（阻断）：`core.quotePath=false` 仍不能表示所有合法路径

位置：`.github/workflows/release-assets.yml:245`、`.github/workflows/release-assets.yml:741`

`core.quotePath=false` 的作用范围只是让高于 `0x80` 的字节不再被视为必须转义，因此当前 8 个中文路径会从八进制 C-style 表示恢复为原始 UTF-8。它不会关闭 Git 对双引号、反斜杠和控制字符的 C-style quoting。

因此，含普通空格的路径可以继续作为单行数据经过 `sort` / `diff`，但含双引号等特殊 ASCII 字符的合法路径仍可能在 `git ls-tree` 一侧成为带外围引号和反斜杠转义的表示，而 tar member listing 使用另一种表示。归档内容正确时，两个无条件 `diff -u` 仍可能错误失败。

这个问题同时存在于：

1. 已发布 Release 的 `published-source-expected.txt` / `published-source-actual.txt` 校验；
2. publish 前的 `source-tree-expected.txt` / `source-tree-actual.txt` 校验。

需要用不会发生展示层 quoting 的结构化或 NUL-safe 路径比较，并至少用中文、空格和双引号路径 fixture 验证 `git archive -> tar listing -> tree equality` 全链路。只增加全局/命令级 `core.quotePath=false` 不能关闭本 finding。

### B2（阻断）：新增测试可以被注释或无关文本伪绿

位置：`crates/codex-plus-core/tests/installers.rs:1320`

测试只断言整份 workflow 中字符串 `git -c core.quotePath=false ls-tree -r --name-only` 恰好出现两次，并断言旧字符串不存在。它没有证明这两次出现是两个目标 Bash step 中的 active `run` 命令，也没有约束 `"$TARGET_SHA"`、tar listing、排序和最终 `diff` 仍连接在同一条校验链上。

独立 mutation 将两条 active 命令都加上 `#` 后得到：

```text
commented-both-active-commands-changed=True
commented-count=2
current-test-predicate-on-commented-commands=True
```

即两个校验点均失效时，当前测试仍会通过。同理，把目标 active 命令删除、在注释或无关 job 中补两次字符串，或移除 `"$TARGET_SHA"`，也不受当前谓词约束。

测试还没有创建或分析真实路径 fixture。当前 `HEAD` 的 tree 覆盖为：

```text
tracked-unicode=8
tracked-space=0
tracked-quote=0
```

需要分别切出两个目标 job/step 的 active shell body，约束完整命令及 `ls-tree -> sort -> diff` 关系，并添加 comment、missing、wrong job、missing target SHA、断链和 decoy mutations。路径行为测试必须覆盖中文、空格和双引号，避免只验证 workflow 文本。

## 边界与回归复核

- 两个目标 step 均显式使用 `shell: bash` 和 `set -euo pipefail`；`diff -u` 保持无条件执行，缺失或额外的普通路径仍 fail-closed。
- `git -c core.quotePath=false` 是单命令作用域，不会写 repository/global Git config；这个作用域选择本身正确。
- `LC_ALL=C` 只作用于各自的 `sort`，提供确定的字节序；管道没有对普通空格做 shell word splitting。
- diff 没有改变 `git archive`、gzip、下载、发布状态或上传行为；回归范围保持很小。
- 上述正确边界不能弥补 B1 的路径表示缺口或 B2 的测试伪绿。

## 验证

| 验证 | 结果 |
|---|---|
| `cargo test -p codex-plus-core --test installers release_source_tree_checks_preserve_unicode_paths --locked -- --exact --nocapture` | PASS，1/1；但受 B2 限制 |
| 默认 `git ls-tree -r --name-only HEAD` | 8 个中文路径被 C-style 八进制转义 |
| `git -c core.quotePath=false ls-tree -r --name-only HEAD` | 8 个中文路径输出原始 UTF-8 |
| 当前 tree 路径覆盖 | Unicode 8，空格 0，双引号 0 |
| 注释两个 active 命令 mutation | 当前测试谓词仍为 true，确认伪绿 |

## 结论

当前两行实现能修复现有中文路径造成的 hosted failure，并且没有扩大 Git config 作用域；但它不是对合法特殊路径的完整修复，测试也不能证明两个实际校验点仍然有效。B1、B2 关闭前，本步骤不能通过。

---

## Remediation 复审（2026-07-13）

> 最终结论：**PASS AFTER REMEDIATION**

### B1 已关闭：路径比较改为端到端 NUL-safe

两个 source-tree 校验点均不再比较 `git ls-tree` 与 `tar -t` 的展示文本，而是：

1. 将可信的 `git archive` 解包到独立 `/tmp` root；
2. 使用 `git ls-tree -rz --name-only "$TARGET_SHA" | LC_ALL=C sort -z` 生成 expected；
3. 在 quoted archive root 中使用 `find . -mindepth 1 ! -type d -print0`；
4. 使用 `sed -z` 去掉每条记录的 `./`，再用 `sort -z` 排序；
5. 使用 `cmp` 对两个 NUL-delimited byte streams 做精确比较。

该链不经过 Git 或 tar 的可读展示 quoting，因而中文、普通空格、双引号、反斜杠和换行均作为路径原始字节保留；Git 路径本身不能含 NUL，因此 NUL 是完整记录分隔符。

`find` 默认不跟随 symlink，`! -type d` 会包含普通文件、指向文件/目录的 symlink 和 broken symlink，同时排除目录；这与递归 `git ls-tree --name-only` 的文件/链接清单语义一致。Git 不记录空目录，解包产生的目录项不会造成额外记录。

解包边界也保持可信：

- 已发布资产在解包前先与同一 `TARGET_SHA` 生成的 expected `git archive | gzip -n` 做字节级 `cmp`；
- publish 前的 source asset 在同一 step 中直接由 `git archive` 生成；
- 两者均带受控单一 prefix，目标目录和 `cd` 路径均加引号；
- `set -euo pipefail` 保持不变，解包、进入 prefix、任一 NUL pipeline 或最终 `cmp` 失败都会终止 step；
- publish job 的校验仍位于 Release create/upload/publish 外部副作用之前。

当前 tree 没有 symlink 或 gitlink，因此 symlink 结论来自 `find`/Git/tar 语义复核；未发现实现缺口。

### B2 已关闭：测试限定到具体 active step

最终 helper 依次切分：

- `verify-published-release` job / `Verify existing corresponding source` step；
- `publish-release` job / `Create draft, upload assets, publish` step。

它先移除整行注释，再要求每个目标 step 含唯一 Git NUL listing、`find -print0`、`sed -z`、至少两个 `sort -z`，并分别要求正确的 `mkdir`、`tar -xzf`、quoted `cd` 和最终 `.z` 文件 `cmp`。同 job 其他 step、其他 job 或文件末尾注释不能再满足目标 step。

独立 mutation 结果：

```text
baseline=True
commented-target-step=False
missing-extractions=False
missing-cmp=False
wrong-step-active-decoy=False
```

仓库测试自身还覆盖首个 listing 缺少 `-z`、首个真实 `find` 被注释，以及同 job 其他 active step 添加完整 decoy；三者均被拒绝。此前 B2 的 comment/decoy/missing 伪绿已关闭。

### 最终验证

| 验证 | 结果 |
|---|---|
| focused `release_source_tree_checks_preserve_unicode_paths` | PASS，1/1 |
| `cargo test -p codex-plus-core --test installers --locked` | PASS，30/30 |
| 独立 step-boundary mutations | baseline true；commented/missing extraction/missing cmp/wrong-step decoy 全部 false |
| scoped `git diff --check` | PASS；仅现有 LF/CRLF checkout 提示，无 whitespace error |

最终 remediation 同时关闭 B1 和 B2，未发现剩余阻断项。本审计 B 的最终状态由 **FAIL** 更新为 **PASS AFTER REMEDIATION**。

---

## License Contract 同步复审（2026-07-13）

> 范围：`scripts/verify-license.ps1`、`apps/codex-plus-manager/src-tauri/tests/windows_subsystem.rs` 及其对应 workflow token
> 最终结论：**PASS AFTER REMEDIATION**

### 中途 finding（已关闭）：sorting token 与实际 shell 行不一致

本轮初始 diff 曾要求不存在的 token：

```text
| LC_ALL=C sort -z > /tmp/source-tree-actual.z
```

当时 workflow 将重定向放在 subshell 结束行 `) > /tmp/source-tree-actual.z`，生产守卫实际复跑失败。最终 remediation 将两个 actual-list 输出的重定向移动到各自 `sort -z` 行：

```text
| LC_ALL=C sort -z > /tmp/published-source-actual.z
| LC_ALL=C sort -z > /tmp/source-tree-actual.z
```

这与 verifier、manager assertion 和 core helper 的 token 完全一致。重定向仍位于 subshell 内最后一个 pipeline；`set -euo pipefail` 及 subshell 的 pipeline exit status 保持有效，行为与原 subshell 整体重定向等价。

### Token 与 mutation 复核

- publish source 环的 archive、extract、actual sort output 和 final compare token 均使用 `$source_asset` / `/tmp/source-tree-*.z`，不会由已发布资产的 `/tmp/published-source-*` 环替代满足。
- Git `-z` listing、`find -print0` 和 `sed -z` 在两个环各出现一次；`Assert-ReplacementFails` 使用全局 `.Replace`，对应 self-test 会同时真实改动两个 occurrence，而不是只改不存在的 fixture。
- actual sorting token 精确且只命中 publish 环；sorting mutation 将 `sort -z` 改为 `sort` 后，生产 required token 消失并产生 finding。
- `Assert-ReplacementFails` 在 replacement 未改变 fixture 时会记录 `negative case did not mutate fixture`；改变后若 verifier 无 finding，则记录 `negative case passed unexpectedly`。因此 self-test PASS 同时证明每个 case 确实发生 mutation 且被 gate 拒绝。
- 新 case names 已同步：NUL listing、archive extraction、NUL traversal、NUL normalization、NUL sorting、comparison、required file 和 commented content gate。
- 旧 case names，以及旧 `.txt` tree-list / `diff -u` contract token 已从 verifier 与 manager test 移除。workflow 中仍存在的 `tar -tzf "$source_asset"` 用于后续 required-file 检查，不是旧 whole-tree comparison 的残留。

`verify-license.ps1` 与 manager test 的部分检查仍是全文 token 检查；comment/decoy/job 边界由同一回归面的 core `release_source_tree_checks_preserve_unicode_paths` 测试补强。该测试限定两个具体 active step，绑定各自 unique actual output，并包含 `missing_sort`、commented find 和 wrong-step active decoy mutations，因此没有剩余可观察伪绿路径。

### 最终验证

| 验证 | 结果 |
|---|---|
| `pwsh -NoProfile -File scripts/verify-license.ps1` | PASS |
| `pwsh -NoProfile -File scripts/verify-license.ps1 -SelfTest` | PASS |
| manager `github_release_archives_corresponding_source_from_resolved_target` | PASS，1/1 |
| manager `license_self_test_breaks_each_source_archive_integrity_gate` | PASS，1/1 |
| core `release_source_tree_checks_preserve_unicode_paths` | PASS，1/1，含 `missing_sort` |
| scoped `git diff --check` | PASS；仅现有 LF/CRLF checkout 提示，无 whitespace error |

本轮 token mismatch、sorting 环覆盖和 case-name 同步均已关闭；未发现剩余阻断项。

### Expected Sort 最终收紧复核

最终 verifier 不再只要求 `git ls-tree -rz` 前缀，而是精确要求 publish source 环的完整 expected-list 命令：

```text
git ls-tree -rz --name-only "$TARGET_SHA" | LC_ALL=C sort -z > /tmp/source-tree-expected.z
```

该 token 同时绑定目标 commit、NUL listing、NUL sort 和 publish 环唯一 expected 输出，不能由 `/tmp/published-source-expected.z` 环或只保留 `git -z` 的 decoy 满足。对应 `Assert-ReplacementFails` 只把该完整行的 `sort -z` 改为 `sort`；fixture 必然实际变化，且 required token 随即消失。actual-list sorting 仍由独立 `/tmp/source-tree-actual.z` token 和 mutation 保护，因此 expected 与 actual 两侧的 NUL sort 各有独立守卫。

manager assertion 已同步完整 expected token，case list同时包含 `source tree expected NUL sorting integrity` 和 `source tree NUL sorting integrity`。core helper继续限定两个具体 active step，并分别绑定 published/publish 的 unique actual outputs。

最终复跑：

- `verify-license.ps1`：PASS；
- `verify-license.ps1 -SelfTest`：PASS；
- manager `license_self_test_breaks_each_source_archive_integrity_gate`：PASS，1/1；
- scoped `git diff --check`：PASS，仅现有 LF/CRLF checkout 提示。

Expected sort 的独立覆盖已关闭，最终结论保持 **PASS AFTER REMEDIATION**。
