# Remediation Promo/Branding Audit B - Diff 与回归边界

> Status: **PASS**
> Date: 2026-07-10
> Scope: 去推广与品牌修复 diff、边界条件和回归面

## TDD 证据

- Red: 推广门禁、注入脚本文本断言和本地赞助图片注入测试均先失败。
- Green: ads 8/8、cdp_bridge 69/69、推广门禁 OK。
- 边界: 已有通用 ads normalize 接口保留；生产 `ADS_ENABLED=false`、URL 列表为空、前端没有消费入口。

## 独立审计 B

首次结论为 FAIL：发现 Bug 表单旧品牌、Discussions 未启用，以及二进制推广素材缺少防回归门禁。

关闭后独立复核：

- Bug 表单、About、Issues、Discord/Telegram 和 sponsor 运行时路径无旧推广残留。
- 自有 Discussions 与 Issues 均已启用，contact link 可用。
- 删除素材后增加文件存在性门禁，防止 `docs/images/sponsor-*` 与交流群二维码回流。
- `git diff --check`、两个 JS 语法检查和 77 个定向测试通过。

## 结论

PASS。最终安装包的真实 UI 目视验证仍属于 Release 产物验收。
