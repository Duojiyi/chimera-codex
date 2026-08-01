# Chimera++ v2.1.0 参考项目兼容矩阵

- 更新日期：2026-08-01
- Chimera++ 基线：`v2.1.0` 候选提交
- CC Switch 功能基线：`v3.19.1`（`28529620f438b2ed25c812f6364825d846a4a9d6`）
- 规则：只按行为和最小补丁适配；不以整文件覆盖或同名函数替代验证。

| 模块 | 参考基线 | Chimera++ 适配状态 | v2.1.0 证据 / 说明 |
| --- | --- | --- | --- |
| Codex 第三方认证 | CC Switch `e3f80a98` | parity | `v2.0.14` 已清除第三方 `requires_openai_auth`；`provider_service` 回归测试覆盖官方/第三方切换。 |
| Codex 原生 Responses catalog | CC Switch `8ae1ce85` | parity | 采用官方 DeepSeek catalog 模板与保留显示名/context-window 的构建逻辑；Rust 单测随 CI 运行。 |
| provider switch / proxy | CC Switch provider service | adapt | Chimera++ 保留既有 per-app switch lock、live backup 和回滚实现；前端仅提交启动意图。 |
| Codex 安装发现与进程生命周期 | Codex App Manager | adapt | `codex_win_engine` 基于安装路径发现进程；`open_codex_runtime` 在目标已运行时先关闭并确认，再启动并健康检查。 |
| SQL 导入 | CC Switch `c98913df` | parity | 拒绝跨文件 SQL 语句，避免备份导入把同一语句拆分到多个文件。 |
| 终端 cwd 转义 | CC Switch `35486afd` | parity | POSIX 终端使用单引号转义，含空格/引号/控制字符的路径不再形成命令注入面。 |
| 通用配置遍历 | CC Switch `cd17912f` | parity | 忽略 `__proto__`、`constructor`、`prototype`，防止配置合并污染原型。 |
| Deeplink 与 MCP 风险提示 | CC Switch `6dbb944b` + `a443eae9` | parity | 导入确认界面展开 MCP command/args/env，遮蔽凭据并显示 endpoint、shell、敏感环境变量风险。 |
| ZIP Slip / 归档上限 | CC Switch `ff3bc242` | pending | 上游补丁包含与 Chimera++ 当前分支冲突的 TOML/MCP 改造；需按可达的导入/下载路径拆分适配，未作为 v2.1.0 候选包的完成项声明。 |
| 自动路由事务与启动自愈 | Chimera++ v2.1 计划 | pending | 现有热切换回滚和锁已存在；完整 journal/lease/recovery 契约仍需后续独立 PR 与故障注入测试。 |

## 行为契约

关键行为按外部可验证输入/输出定义：

1. **官方 → 第三方 Codex**：保存 API Key 后，live `config.toml` 不保留 `requires_openai_auth`，`auth.json` 不能留下会触发 ChatGPT 登录的伪 OAuth 状态。
2. **模型目录**：每一条映射生成独立的 `slug` 与 `display_name`；不以单一“自定义”条目代替多个真实模型。
3. **生命周期**：任何启动请求先获取跨进程锁；若同一受管目标已运行，必须先完全退出，退出失败则禁止启动第二实例；返回值为 `launched` 或 `restarted`。
4. **安全导入**：风险信息属于确认 UI，敏感环境变量值必须脱敏；配置合并不得触及原型链。

CI 中的 Rust 单元/集成测试、前端单元测试与候选构建日志是这些契约的可追溯证据。
