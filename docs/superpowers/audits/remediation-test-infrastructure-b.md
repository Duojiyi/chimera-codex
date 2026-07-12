# Remediation Test Infrastructure Audit B - Diff、边界与回归面

> Status: **PASS**
> Date: 2026-07-11
> Scope: client 注入边界、并发测试状态与 allowlist 绕过面

## TDD 证据

- 环境变量写入禁止测试先失败，再由 no-proxy client 注入转绿。
- client 注入测试先因入口缺失编译失败，再由最小内部复用实现转绿。
- allowlist 合约测试依次证明旧 SelfTest、旧宽松格式及无行号匹配会失败。
- 最终定向回归：Protocol 45/45、Ads 7/7、CDP bridge 69/69、格式检查和两个 PowerShell 门禁均通过。

## 独立审计 B

首次复核发现：

- Protocol 未发现生产代理、User-Agent 或 client 生命周期回归，结论 PASS。
- Allowlist 仍可重复使用同一豁免，未检查失效豁免；JSON 类型和多行未严格验证；部分特殊规则未按实际单行匹配，结论 FAIL。

关闭后复核：

- 生产入口仍传入 `None` 并创建正常的 proxy-aware client；`.no_proxy()` 仅存在于测试 client。
- 注入 client 使用 `reqwest::Client::clone()`，不把调用方借用泄漏到返回响应生命周期。
- 非空配置/原始 User-Agent 逐请求写入；空 User-Agent 保留生产 client 默认行为。
- allowlist 只接受严格 JSON 对象；拒绝未知字段、错误类型、CR/LF、非规范空白、绝对/盘符/反斜杠/`.`/`..`/通配路径。
- 匹配使用 Ordinal 精确比较，并同时绑定 path、pattern、line number 和完整 trimmed line。
- 命中后设置 `Used=true`；同一条不能二次消费，扫描结束后任何未使用条目都会使门禁失败。
- artifact、update、Publisher、`append_builtin_sponsors` 和通用规则均逐行传入真实行号与行文本。
- 合约自检在 `-SelfTest` 分支判断前无条件执行，因此真实扫描路径不会绕过负向测试。

## 结论

PASS。未发现剩余阻断项；新增的 `#[doc(hidden)]` client 注入入口属于非阻断的公开 API 面积风险，后续上游同步时需留意签名漂移。
