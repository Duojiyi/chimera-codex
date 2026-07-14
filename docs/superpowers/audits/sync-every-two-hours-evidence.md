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

## Cloud Gate Remediation

- Cloud Red: PR #12 run `29344521235` 的 `Branding generate check` 在当前版本
  `1.2.35-chimera.4`、当前 build `6` 与最新正式发行完全相同时仍要求 build 大于
  `6`，导致普通发布后 PR 被错误拦截，后续平台 job 全部 skipped。
- Policy: 当前版本与最新 Chimera Release tag 相同时，build 必须与已发布值相等；
  当前版本严格高于最新 tag 时，build 必须严格递增；版本倒退一律拒绝；首发只要求正整数。
- Audit Red 3: `1.2.34-chimera.99` / build `7` 相对已发布
  `1.2.35-chimera.4` / build `6` 被旧逻辑错误接受。
- Audit Green 3: 严格比较 `X.Y.Z-chimera.N` 四段数值；SelfTest 覆盖同版本保持、
  同版本变化/回退、新版本递增/复用、上游版本倒退、Chimera revision 倒退、非法版本和首发边界。
- Audit Red 4: 最新 tag 发现链路没有可注入 Git runner，无法保留 `git tag` / `git show`
  失败及禁止回退旧 tag 的确定性测试，SelfTest 以未知 `GitRunner` 参数退出 1。
- Audit Green 4: Git runner 注入后，SelfTest 验证只读取排序后的第一个 tag，并对 tag 枚举失败、
  最新 tag 内容读取失败、tag/build 缺失或重复全部 fail-closed。
- Audit Red 5: 元数据同时包含合法 `macos_build_number = 6` 与非法重复
  `macos_build_number = "invalid"` 时，旧解析器只统计合法行并错误接受 build `6`。
- Audit Green 5: 解析器先统计所有 plain/单双引号 `macos_build_number` key 且要求恰好一次，
  再严格解析唯一值；mixed valid/invalid duplicate SelfTest 通过。
- Gate wiring Red: 旧的全 workflow `contains` 会接受 `if: false` decoy job；下一 job key
  带行尾注释、`gates` 自身被 `if: false` 跳过或 `continue-on-error` 非阻断也可绕过；
  quoted/spaced 等价 key 也曾逃过精确前缀扫描。
- Gate wiring Green: PR/Release 均执行 `generate-branding.ps1 -SelfTest` 与
  `test-sync-upstream.ps1`；合约限定 `jobs.gates`，识别带注释的 job 边界，禁止非阻断 gate，
  识别 plain/quoted/spaced job-level controls，且 Release gate 只允许既定 `should_publish` 条件。

## Regression

- `pwsh -NoProfile -File scripts/generate-branding.ps1 -SelfTest`: PASS.
- `pwsh -NoProfile -File scripts/generate-branding.ps1 -Check`: PASS.
- `pwsh -NoProfile -File scripts/test-sync-upstream.ps1`: PASS.
- `pwsh -NoProfile -File scripts/verify-no-upstream-ads.ps1`: PASS.
- `pwsh -NoProfile -File scripts/verify-license.ps1`: PASS.
- `cargo fmt --all -- --check`: PASS.
- `git diff --check`: PASS.

## Gate

本地轻量回归已通过；Rust/前端完整构建按用户要求交由 PR 云端 required checks。
GitHub schedule 为 best-effort，可能延迟，不能视为准点 SLA；手工 `workflow_dispatch` 保持可用。
