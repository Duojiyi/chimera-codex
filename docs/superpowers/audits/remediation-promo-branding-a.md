# Remediation Promo/Branding Audit A - 需求与行为

> Status: **PASS**
> Date: 2026-07-10
> Scope: 去推广、交流群清理、注入 UI 品牌与公开仓库入口

## TDD 证据

- Red: 强化扫描器首次报告 21 项；`cdp_bridge` 2 项失败；赞助图片 normalize 测试失败。
- Green: `cargo test -p codex-plus-core --test cdp_bridge --test ads --no-fail-fast`，77 passed。
- 回归: `verify-no-upstream-ads.ps1` OK；两个注入 JS 的 `node --check` 均通过。

## 独立审计 A

首次结论为 FAIL：发现 `docs/images/discussion-group-qr.jpg` 和 Bug 表单三处 `Codex++` 未被门禁覆盖。

关闭后独立复核：

- 交流群二维码和 24 个 `sponsor-*` 素材均不存在，且没有代码或文档引用。
- Bug 表单三处已改为 `Chimera Codex`。
- 门禁显式检查 sponsor 文件名、交流群二维码和公开 Bug 表单旧品牌。
- GitHub API 确认仓库公开且 Discussions 已启用。

## 结论

PASS。仅保留安装包 UI 尚未目视验证这一发布阶段风险。
