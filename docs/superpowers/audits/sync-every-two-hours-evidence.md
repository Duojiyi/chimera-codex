# Sync Every Two Hours Evidence

## Scope

- 将上游正式 Release 监控从每日两次改为每两小时一次。
- 使用 UTC `23 */2 * * *`，避开整点调度高峰。
- 保留 `workflow_dispatch`、`sync-upstream` concurrency 和现有安全边界。

## TDD

- Initial Red: 新调度契约在旧 `0 6` / `0 18` cron 上退出 1。
- Initial Green: workflow 改为唯一 `23 */2 * * *`，完整同步契约通过。
- Audit Red 1: 单引号和无引号额外 cron 被旧正则错误接受。
- Audit Green 1: 统计任意引号形式的 block cron 后测试通过。
- Audit Red 2: quoted-key 和 flow-map 合法 YAML 仍可绕过。
- Audit Green 2: schedule 块去掉空行/纯注释后严格只允许唯一 canonical 行。

## Regression

- `pwsh -NoProfile -File scripts/test-sync-upstream.ps1`: PASS.
- `pwsh -NoProfile -File scripts/verify-no-upstream-ads.ps1`: PASS.
- `cargo fmt --all -- --check`: PASS.
- `git diff --check`: PASS.

## Gate

最终独立审计 A、B 均为 PASS。GitHub schedule 为 best-effort，可能延迟，不能视为准点 SLA；手工 `workflow_dispatch` 保持可用。
