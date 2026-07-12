# Remediation Key-first Audit A - 需求与行为

> Status: **PASS**
> Date: 2026-07-10
> Scope: ChimeraHub Key-first 保存、live 应用与失败脱敏

## TDD 证据

- Red: manager 失败测试证明 Key 会留在 settings；relay switch 两条 Red 证明缺失 settings 被创建、后置校验后 live 文件未恢复。
- 审计追加 Red: atomic write rename 失败遗留 `.tmp`；已有 model catalog 被覆盖后未恢复。
- Green: atomic temp 1/1、relay switch 8/8、manager Key-first 4/4。

## 独立审计 A

首次 FAIL 发现：固定 `.tmp` 可能残留 Key、save 后 reload 失败绕过回滚、settings 恢复证据不足。

关闭复核：

- `atomic_write` 的 write/rename 错误路径均尝试删除 temp，清理失败进入错误链。
- `save_normalized` 一次落盘并返回同一规范化对象，移除 save/load 半提交窗口。
- settings 按原始 bytes 恢复，完整失败响应不含输入 Key。
- config、auth 与当前 profile 生成的 model catalog 都纳入事务。

## 结论

PASS。断电、进程崩溃或 ACL 同时拒绝正式文件回滚与 temp 清理属于环境级残余风险，错误会显式报告。
