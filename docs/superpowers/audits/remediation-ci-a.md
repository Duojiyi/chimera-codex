# Remediation CI Audit A - 需求与发布行为

> Status: **PASS**
> Audit date: 2026-07-10
> Recorded: 2026-07-11
> Scope: upstream sync、PR gates 与 build-first Release

## 独立审计 A

- resume 分支在重新门禁前必须同时包含受信 `origin/main` 和正式 upstream Release tag。
- resume 与新建候选均绑定门禁前后 HEAD，并要求工作树保持 clean。
- draft Release 会删除非预期资产，上传后严格验证六个平台资产与 `latest.json` 共七项。
- tag 使用 no-clobber 语义；发布前后复核 target SHA 与 tag 指向。
- `latest.json` 的六个资产逐项绑定 GitHub Release API 的 digest 与 size。
- 执行 `gh` 的 PowerShell 路径启用 native-command fail-fast。
- 写权限与自动化 token 延迟到所有只读门禁之后。

## 验证证据

- `cargo test -p codex-plus-manager --test windows_subsystem`：29/29。
- workflow YAML 与 PowerShell AST 解析通过。
- `pwsh -NoProfile -File scripts/sync-upstream.ps1 -DryRun`：退出 0，HEAD/refs 不变。
- `git diff --check`：通过。

## 结论

PASS。代码层 CI 闭环无阻断项；真实 Actions 运行仍以 `CHIMERA_AUTOMATION_TOKEN` 和产品代码推送为外部前置条件。
