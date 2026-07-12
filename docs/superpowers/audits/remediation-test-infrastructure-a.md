# Remediation Test Infrastructure Audit A - 需求与可观察行为

> Status: **PASS**
> Date: 2026-07-11
> Scope: protocol proxy 测试隔离与去推广 allowlist 精确匹配

## TDD 证据

- Protocol Red 1：新增约束测试后，因仍存在 `std::env::set_var` 而稳定失败。
- Protocol Red 2：测试改用 client 注入后，四个注入入口尚未实现，编译按预期失败。
- Protocol Green：实现显式 client 注入并移除进程环境变量写入后，`protocol_proxy` 45/45 通过。
- Allowlist Red 1：合约测试调用尚不存在的 `-SelfTest`，按预期失败。
- Allowlist Red 2：严格 JSON 解析启用后，旧 `path:pattern:reason` 格式按预期 fail closed。
- Allowlist Red 3：审计补测要求 `LineNumber`、严格解析与一次性消费，旧匹配器按预期失败。
- Allowlist Green：合约自检与真实 `verify-no-upstream-ads.ps1` 均通过。

## 独立审计 A

首次复核结论：

- Protocol：PASS。
- Allowlist：FAIL。解析失败路径没有持续回归，打包检查仍把整文件内容当作允许行。

关闭后复核：

- loopback 测试全部使用显式 `.no_proxy()` 客户端；测试文件不再写进程环境变量。
- 设置路径测试仍由可恢复互斥锁串行化；User-Agent、超时、failover 与 SSE 行为断言保留。
- allowlist 严格绑定仓库根相对路径、scanner pattern、源码行号与完整 trimmed 行。
- 旧格式、非字符串字段、CR/LF、多行与 wildcard 路径均由正常 scanner 无条件执行的合约自检覆盖。
- 所有特殊规则均逐命中行匹配，不再把整文件内容传给允许行参数。
- 18 条真实 allowlist 与当前 18 个命中逐项对应；未使用或重复消费均失败。

## 回归证据

- `cargo fmt --all -- --check`
- `cargo test -p codex-plus-core --test protocol_proxy --test ads --test cdp_bridge`
  - Protocol 45/45
  - Ads 7/7
  - CDP bridge 69/69
- `pwsh -NoProfile -File scripts/test-verify-allowlist.ps1`
- `pwsh -NoProfile -File scripts/verify-no-upstream-ads.ps1`

## 结论

PASS。需求行为与负向边界均已覆盖；无阻断发现。
