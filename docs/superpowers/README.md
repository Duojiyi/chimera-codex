# Superpowers 文档索引

本目录保存历史设计、实施计划和当前发行计划。**只有标为 Active 的文档可以直接驱动实施**；Historical/Superseded 文档仅用于理解演进，执行前必须重新对照当前代码与 `AGENTS.md`。

## Active

| 主题 | Spec | Plan | Checklist | 状态 |
|------|------|------|-----------|------|
| Chimera Codex 公开发行版 | `specs/2026-07-10-chimera-private-fork-design.md` | `plans/2026-07-10-chimera-private-fork.md` | `todos/2026-07-10-chimera-private-fork-todo.md` | 文档与仓库初始化已完成；等待用户批准产品代码开工 |

对应交叉验证：`reports/2026-07-10-cross-verification.md`。

## Current Model-Catalog Documents

按模型上下文窗口的当前依据不在本目录，而在：

- `docs/specs/2026-06-23-model-catalog-prototype-design.md`
- `docs/specs/2026-06-24-前端模型后缀提示-design.md`
- `docs/specs/2026-06-25-model-list-window-split-design.md`
- `docs/plans/2026-06-23-阶段一-model-catalog-原型.md`
- `docs/plans/2026-06-24-前端模型后缀提示.md`
- `docs/plans/2026-06-25-model-list-window-split-plan.md`

`specs/2026-05-25-codex-context-management-design.md` 与对应 plan 中的 supplier-level `contextWindow` / `autoCompactLimit` 属于旧方案，已被 model-catalog 方案取代；其中 MCP/skills/plugins 部分只能作为历史参考。

## Superseded

- `plans/2026-05-13-provider-sync.md`：Python 实施计划，已被 Rust/Tauri 迁移和当前 Rust 实现取代。
- `specs/2026-05-16-rust-tauri-migration-design.md` / 对应 plan：迁移基线，当前仓库已经是 Rust/Tauri，不能再当作待执行计划。
- `specs/2026-05-25-codex-context-management-design.md` / 对应 plan：上下文窗口部分被当前 model-catalog 设计取代。

## Historical Reference

其余 2026-05 至 2026-06 的 user scripts、conversation timeline、script market、upstream worktree/branch、provider configuration/sync、PR build、plugin unlock 文档记录功能形成过程。它们可能包含已删除文件、旧 API、旧产品名或已经实现的步骤，默认状态均为 Historical。

## Maintenance Rules

1. 新 spec 顶部必须写 `状态`、日期、代码基线和 `supersedes/superseded by`。
2. 新 plan 必须链接唯一 spec，列出真实文件路径、测试和回滚条件，不依赖未声明的 agent skill。
3. 完成实施后记录实现 commit 和验证报告；被替代时更新本索引。
4. 历史文档中的命令不得直接执行，尤其是远端写、安装依赖、删除、rebase/merge 和发布命令。
5. 文档与代码冲突时，以 `AGENTS.md`、当前代码和最新 Active spec 为准，并先修正文档。
