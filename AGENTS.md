# AGENTS.md

本文件为 CodexPlusPlus fork 的工作规范，指导 agent 在本仓库工作。

## 项目概述

本仓库是 [BigPizzaV3/CodexPlusPlus](https://github.com/BigPizzaV3/CodexPlusPlus) 的公开发行 fork。当前目标是在保留「按模型粒度配置上下文窗口与自动压缩阈值」能力的基础上，制作并维护 Chimera Codex 发行版：去推广、浅层品牌化、ChimeraHub 首次配置、独立公开更新源，以及跟踪上游正式 Release 的自动同步。

采用 codex 原生 `model_catalog_json` 机制：通过 `model_list` 后缀语法（如 `deepseek-v4-pro[1M]`）声明每模型窗口，由 CodexPlusPlus 生成 catalog 文件并注入 config.toml 指针，codex 客户端运行时按模型识别各自窗口。

Chimera 品牌、默认中转和去推广改动不回推上游；可复用的通用 bugfix 可以拆分后向上游提交。

## 仓库结构

- `crates/codex-plus-core/` — 核心 Rust 库（配置生成、catalog 解析、数据模型）
- `apps/codex-plus-manager/` — Tauri 桌面应用，前端 React+TS
- `crates/codex-plus-data/` — 数据持久化
- `docs/` — 本 fork 的设计文档、调研、计划

## 关键代码位置

- 数据模型：`crates/codex-plus-core/src/settings.rs` 的 `RelayProfile` 结构体
- 配置生成：`crates/codex-plus-core/src/relay_config.rs` 的 `apply_context_limits_to_config`
- catalog 解析：`crates/codex-plus-core/src/model_catalog.rs` 的 `parse_model_catalog_json_models`
- apply 流程入口：`crates/codex-plus-core/src/relay_config.rs` 的 `apply_relay_profile_to_home_with_switch_rules_and_computer_use_guard`
- 前端模型列表：`apps/codex-plus-manager/src/App.tsx` 的 `modelList` textarea

## 安全规则

- 禁止批量删除、rm -rf、rmdir /s
- 删除只能单个文件，删除前确认
- 禁止 sudo、提权、curl | bash
- 禁止泄露密钥、.env、auth.json、config.toml 凭据
- 覆盖文件前确认
- 不擅自改 Cargo.toml、package.json、.gitignore（除非任务必需）

## 命令执行

- 执行 bash 命令前确认
- 不运行未知脚本、不擅自装依赖
- 测试用 cargo test，不另起工具链

## 编码规范

- 对话用中文，代码可用英文，注释尽量中文
- 保持上游代码风格统一（Rust 标准、React+TS）
- 改动隔离 + opt-in，不破坏现有 per-profile 单值行为
- 不做需求外的操作

## 测试约定

- 沿用上游 `#[test]` + tempfile 风格（见 `crates/codex-plus-core/tests/relay_config.rs`）
- 断言读 config.toml 文本，如 `assert!(config.contains("model_catalog_json"))`
- 改行为要同步改/加对应测试

## TDD 与双盲审计

- 每个可观察行为改动必须先写会失败的测试（Red），再做最小实现（Green），最后仅在必要时重构（Refactor）；禁止先改实现再补测试。
- Plan 中每个 `Step` checkbox 是一个最小任务；TODO 中的 `T*` checkbox 是对应大任务聚合门。每个最小任务完成后必须保留 Red、Green 和针对性回归命令的结果。
- 每个最小任务完成后进行两次相互独立的审计：审计 A 只按需求、测试与可观察行为查漏；审计 B 独立按 diff、边界和回归面查漏。两次结论在记录前不得互相引用。
- 每个大任务完成后，额外进行一次聚合双盲审计，复核任务边界、所有子任务证据和未覆盖风险。
- 审计发现未关闭前不得勾选 TODO，不得进入依赖它的下一任务。审计记录保存在 `docs/superpowers/audits/`。

## 与上游同步

- `upstream` = https://github.com/BigPizzaV3/CodexPlusPlus.git
- `origin` = https://github.com/Duojiyi/chimera-codex.git（公开仓库，已完成创建与可达性核验）
- 当前本地仓库已完成非 shallow 化；任何后续推送前仍必须逐项核对 remote、分支和工作树状态
- 功能分支命名：`codex/<feature>`；自动同步分支：`sync/upstream-vX.Y.Z`
- 自动发行只跟踪上游正式 Release tag，通过同步 PR + required checks 合入，不直接 rebase/覆盖本项目 `main`
- 发布版本格式：`X.Y.Z-chimera.N`
- 严禁把 Chimera 定制误推到 `upstream`
