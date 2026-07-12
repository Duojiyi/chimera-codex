# Task 15 Step 15.5 Sync Issue Repository 独立审计 B

> 日期：2026-07-13
> 分支：`codex/sync-issue-repo`
> 范围：`sync-upstream.yml` 的 Issue 命令、PowerShell/GitHub Actions 边界及 `windows_subsystem.rs` 回归测试
> 独立性：仅按当前 diff、源码、命令语义和独立 mutations 检查
> 结论：**FAIL**

## Finding

### B1（阻断）：测试未约束 verb、job/step 与无-checkout边界

位置：`apps/codex-plus-manager/src-tauri/tests/windows_subsystem.rs:862`

新增测试只收集整份 workflow 中非整行注释且包含 `gh issue ` 的行，然后断言：

1. 总数为 5；
2. 每行都包含 `--repo "$env:GITHUB_REPOSITORY"`。

它没有约束五条命令必须是成功路径的 `list + close` 和阻断路径的 `list + edit + create`，没有限定所属 job/step，也没有证明 `report-blocked` 保持无 checkout。

独立 mutation 结果：

```text
baseline=True
removed-close-changed=True predicate=True
blocked-checkout-changed=True predicate=True
```

第一项 mutation 将成功路径的 `gh issue close` 替换为另一条带正确 `--repo` 的 `gh issue list`。命令面仍为 5 条，当前测试继续通过，但成功路径不再关闭旧 blocked Issue。

第二项 mutation 在 `report-blocked` 中加入 checkout。当前测试同样继续通过，未锁住本次修复所依赖的“无仓库工作树也可调用 `gh issue`”边界。

同理，删除目标命令并在无关 job 增加带 repo 的 active decoy，也可以保持 count=5 并伪绿。

测试需要至少：

- 切分 `publish-sync-pr` / `Push sync branch and open PR`，精确要求一条 list 和一条 close；
- 切分 `report-blocked` / `Upsert blocked Issue (conflict or gate failure)`，精确要求一条 list、一条 edit 和一条 create；
- 对五条命令分别要求 quoted `--repo "$env:GITHUB_REPOSITORY"`；
- 断言 `report-blocked` job 不含 active `actions/checkout`；
- 拒绝 comment、missing、wrong verb、wrong job/step、active decoy 和 checkout mutations。

## 实现复核

当前 workflow 实现本身未发现阻断：

- 五条 active Issue 命令的 verb 分布为 `list, close, list, edit, create`。
- 每条都使用 `--repo "$env:GITHUB_REPOSITORY"`；`--repo` 在子命令参数中的当前位置均被 GitHub CLI 接受。
- PowerShell 双引号会把默认 runner 环境变量展开成 `OWNER/REPO` 单个参数；仓库名中的 `/`、连字符等不会发生 word splitting。
- `GITHUB_REPOSITORY` 是 GitHub Actions 每个 step 默认提供的环境变量，不依赖显式 `env:` 或 checkout。
- 两个调用 step 都设置 `GH_TOKEN: ${{ github.token }}`，对应 job 都有 `issues: write`。
- 成功路径在 PR 已建立、required-check workflow 已 dispatch 且 auto-merge 已启用后 list；只对精确 title 命中的 open Issues 执行 close。
- `report-blocked` job 没有 checkout，只下载 prepare artifact；显式 repo 使 list/edit/create 不依赖 Git remote discovery。
- 阻断路径先 list 并按精确 title 过滤；命中时 edit，未命中时 create，保持幂等 upsert。
- `$title`、`$body`、Issue number 和 repo 都作为 PowerShell 参数传递；双引号/变量展开不会把 body 拆成多个 native arguments。
- `$PSNativeCommandUseErrorActionPreference = $true` 与 `$ErrorActionPreference = 'Stop'` 使任一 `gh` 失败终止对应 step。

## 回归验证

| 验证 | 结果 |
|---|---|
| `cargo test -p codex-plus-manager --test windows_subsystem upstream_sync_issue_commands_bind_repository_explicitly --locked -- --exact --nocapture` | PASS，1/1；但受 B1 限制 |
| `pwsh -NoProfile -File scripts/test-sync-upstream.ps1` | PASS |
| scoped `git diff --check` | PASS；仅现有 LF/CRLF checkout 提示，无 whitespace error |
| removed-close mutation | 当前谓词错误返回 true |
| report-blocked checkout mutation | 当前谓词错误返回 true |

## 结论

workflow 的五条命令已正确绑定目标仓库，能够修复无 checkout job 中 GitHub CLI 无法推断 repository 的问题；但新增测试不能证明成功 close、阻断 upsert 和无-checkout边界持续成立。B1 关闭前，本步骤不能通过。

---

## Remediation 复审（2026-07-13）

> 最终结论：**PASS AFTER REMEDIATION**

### B1 已关闭

最终测试先将 CRLF 规范为 LF，再按以下边界切分：

- `publish-sync-pr` job / `Push sync branch and open PR` step；
- `report-blocked` job / `Upsert blocked Issue (conflict or gate failure)` step。

成功 step 必须恰有 2 条 active Issue 命令，分别包含 `gh issue list` 和 `gh issue close`；阻断 step 必须恰有 3 条，分别包含 `gh issue list`、`gh issue edit` 和 `gh issue create`。五条命令均逐条要求 quoted `--repo "$env:GITHUB_REPOSITORY"`。

`report-blocked` job 还独立拒绝 `uses: actions/checkout`。这锁住了本次修复的关键运行条件：该 job 只有 artifact download，不依赖 Git worktree 或 remote discovery。

负向 mutations 覆盖：

- 将 success `close` 改成额外 `list`；
- 在 report job 增加 checkout；
- 注释 blocked `create`；
- 在同 job 的错误 step 添加 active create decoy。

四者均被 step-scoped helper 拒绝。此前 `removed-close=True` 与 `blocked-checkout=True` 的伪绿入口已关闭；wrong-step decoy 也不能补足目标 step 的命令数。

### 最终验证

| 验证 | 结果 |
|---|---|
| focused `upstream_sync_issue_commands_bind_repository_explicitly` | PASS，1/1 |
| full `windows_subsystem` | PASS，44/44（复用任务证据） |
| `scripts/test-sync-upstream.ps1` | PASS |
| `cargo fmt --all -- --check` | PASS（复用任务证据） |
| scoped `git diff --check` | PASS；仅现有 LF/CRLF checkout 提示，无 whitespace error |

实现的 PowerShell 参数、`GITHUB_REPOSITORY` 默认环境变量、成功 close 和阻断 upsert 行为保持正确；测试已能拒绝要求中的 missing、comment、wrong-step decoy 与 checkout regressions。未发现剩余阻断项，最终状态更新为 **PASS AFTER REMEDIATION**。
