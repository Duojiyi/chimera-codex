# Sync v1.2.36 Merge Execution Evidence

## Scope

- 修复 Sync upstream run `29349243534` 将 merge 执行错误误报为空冲突的问题。
- 为 merge 与后续 candidate commit 提供命令级 Git bot identity。
- 只有真实 unmerged paths 才返回 code 2 / action `conflict`；其他 merge/probe 错误返回 code 4 / action `error`。

## TDD

- Cloud Red: run `29349243534` 返回 exit 2、`conflict_files=[]`，随后 `git merge --abort`
  报 `MERGE_HEAD missing`；Issue #13 因此没有冲突文件。
- Red 1: 新 identity/disposition 测试在旧脚本上因缺少 `Get-IdentifiedGitArgs` 退出 1。
- Green 1: merge 与 commit 使用命令级 `github-actions[bot]` identity；disposition 覆盖真实冲突、
  空路径执行错误和 conflict probe 失败。
- Audit Red 2: 合并 stdout/stderr 会把 warning-only stderr 当作冲突文件；测试未锁定 production callsites
  及 code/action/abort 映射。
- Green 2: `Invoke-Git` 分离 stdout/stderr，`Lines`/`StdoutLines` 仅含 stdout；disposition 返回
  `ExitCode`、`Action`、`ShouldAbort`，source-contract mutation 锁定全部接线。
- Audit Red 3: warning-only 测试仅使用手工对象，没有执行共享 Git wrapper。
- Green 3: 只读 `alias.stream-probe` 固化 Code=0、stdout=`out`、stderr=`warning:advisory`、
  `Text` 同时保留两流的真实行为。

## Regression

- `pwsh -NoProfile -File scripts/test-sync-upstream.ps1`: PASS.
- PowerShell parser (`sync-upstream.ps1`, `test-sync-upstream.ps1`): PASS.
- `cargo fmt --all -- --check`: PASS.
- `git diff --check`: PASS.

## Gate

独立审计 A、B 均 PASS。残余风险：临时 stderr 文件删除或既有 best-effort worktree 清理在极端
文件系统错误下可能掩盖/追加诊断；GitHub runner 为临时环境，半成品分支仍不会推送。
