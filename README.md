# Chimera Codex

<p align="center">
  <img src="docs/images/codex-plus-plus.png" alt="Chimera Codex 图标" width="160">
</p>

<p align="center">
  中文 | <a href="README_EN.md">English</a>
</p>

<p align="center">
  <img alt="Release" src="https://img.shields.io/github/v/release/Duojiyi/chimera-codex">
  <img alt="License" src="https://img.shields.io/badge/license-MIT-blue">
  <img alt="Rust" src="https://img.shields.io/badge/rust-1.85%2B-orange">
  <img alt="Tauri" src="https://img.shields.io/badge/tauri-2.x-24C8DB">
</p>

Chimera Codex 是面向 Codex App 的外部增强启动器与管理工具（基于上游 [CodexPlusPlus](https://github.com/BigPizzaV3/CodexPlusPlus) 的公开发行版）。它不修改 Codex App 原始安装文件，而是通过外部 launcher 启动 Codex，并使用 Chromium DevTools Protocol 注入增强脚本。

本发行版默认对接 **ChimeraHub** 中转，去除上游推广与赞助内容，并通过公开仓库 [Duojiyi/chimera-codex](https://github.com/Duojiyi/chimera-codex) 发布更新。

## 快速使用

从 [GitHub Releases](https://github.com/Duojiyi/chimera-codex/releases) 下载最新版安装包：

- Windows：`ChimeraCodex-*-windows-x64-setup.exe`（另有 zip 便携包）
- macOS Intel：`ChimeraCodex-*-macos-x64.dmg`
- macOS Apple Silicon：`ChimeraCodex-*-macos-arm64.dmg`

安装后会有两个入口：

- `Chimera Codex`：静默启动入口，不显示管理界面，只负责启动 Codex 并注入增强功能。
- `Chimera Codex 管理工具`：Tauri 控制面板，用于启动、检查、修复、更新、配置中转注入、管理增强功能和用户脚本。

## 首次配置（ChimeraHub Key-first）

全新安装会自动创建并选中 **ChimeraHub** 中转配置：

| 项 | 值 |
|---|---|
| Base URL | `https://api.chimerahub.org/v1`（必须带 `/v1`） |
| 协议 | Responses |
| 默认模型 | `gpt-5.5` |

你只需要：

1. 打开 `Chimera Codex 管理工具`。
2. 在 ChimeraHub 配置页填写 API Key。
3. 点击「保存并启用」。

空 Key 不会写入 live 配置，也不会发起业务请求。文档与截图中**不要粘贴真实 Key**；示例一律使用占位符（如 `sk-...`）。

已有用户升级时，不会覆盖你现有的中转 profile / 激活项。

## Windows 覆盖升级

Windows 安装包支持在原版 Codex++ 安装目录上覆盖升级（一期仍使用 `$LOCALAPPDATA\Programs\Codex++` 作为安装根，以降低迁移成本）：

1. 退出正在运行的 `Codex++` / `Chimera Codex` 及相关管理工具进程。
2. 运行新的 `ChimeraCodex-*-windows-x64-setup.exe`。
3. 安装程序会清理旧快捷方式后，只创建 Chimera 入口。
4. 从新的桌面 / 开始菜单入口启动即可。

更新检查匿名读取本仓库公开的 `latest.json`，不会回落到上游更新源。

## macOS 安装、Gatekeeper 与旧 App

Release 分别提供 `macos-x64` 与 `macos-arm64` DMG。当前 macOS 构建使用 **ad-hoc codesign**，**未**使用 Developer ID 签名，也**未** notarize。Gatekeeper 可能提示无法打开或已损坏——这是预期行为，不是安装包损坏。

推荐步骤：

1. 退出旧的 `Codex++.app` / `Codex++ 管理工具.app` 以及新的 Chimera 进程。
2. 打开 DMG，将 `Chimera Codex.app` 与 `Chimera Codex 管理工具.app` 拖入 `/Applications`。
3. 若同目录仍有旧版 `Codex++*.app`，手动移到废纸篓（拖拽不会覆盖不同文件名的旧 App）。
4. 首次打开：在 App 上 **右键 → 打开**，或在「系统设置 → 隐私与安全性」中允许。
5. 若仍被隔离，可在终端解除 quarantine（按实际路径调整）：

```bash
xattr -rd com.apple.quarantine "/Applications/Chimera Codex.app"
xattr -rd com.apple.quarantine "/Applications/Chimera Codex 管理工具.app"
```

## 主要功能

- Rust 后端和静默 launcher，启动时不依赖额外运行时。
- Tauri + React 管理工具，支持深色/浅色切换。
- 外部 CDP 注入，不改 `app.asar`，不向 Codex 安装目录写入 DLL。
- 中转注入：多 profile、写入兼容 provider，并可切回官方 ChatGPT 登录态。
- 传统增强：插件市场解锁、会话删除、Markdown 导出、项目移动等。
- 粘贴修复、Stepwise 建议、用户脚本、Provider 同步、Zed 远程打开等。
- 按模型粒度配置上下文窗口（`model_list` 后缀语法 → `model_catalog_json`）。
- 公开 GitHub Release 自动更新（管理工具与静默启动器均可检测）。

## 中转注入

中转注入适合已在 Codex/ChatGPT 完成官方账号登录，同时希望把模型请求转到兼容 API 的场景：

- 官方登录态继续负责账号能力与插件入口。
- 中转配置只接管 Base URL、Key 与模型名。
- 清除 API 模式后可回到官方登录态。

应用前建议：确认 ChatGPT 登录可用、Base URL（含 `/v1`）可达、用目标 Key 做最小认证探测；**只记录 Key 是否存在与结果，不要把真实 Key 写入日志、截图或 issue**。

ChimeraHub 写入的配置形态类似：

```toml
model_provider = "CodexPlusPlus"

[model_providers.CodexPlusPlus]
name = "CodexPlusPlus"
wire_api = "responses"
requires_openai_auth = true
base_url = "https://api.chimerahub.org/v1"
experimental_bearer_token = "sk-..."
```

（`CodexPlusPlus` 为兼容用的 provider id，不是产品显示名。）

## 自动更新

Chimera Codex 通过本仓库 GitHub Release 发布安装包，并提供公开 `latest.json`（含资产名、大小与 SHA-256）。管理工具「关于」页可检查并启动更新；静默启动器发现新版本时会拉起管理工具。

## 数据位置

- Codex 配置：`~/.codex/config.toml`
- Codex 登录状态：`~/.codex/auth.json`
- Codex 本地数据库：优先 `~/.codex/sqlite/*.db`，旧版回退 `~/.codex/state_5.sqlite`
- 本工具状态与日志：`~/.codex-session-delete/`
- Provider 同步备份：`~/.codex/backups_state/provider-sync`

## 常见问题

### 菜单没出现

确认从 `Chimera Codex` 入口启动，而不是原版 Codex。也可在管理工具的「诊断」「日志」页查看注入状态。

### 插件内显示后端连不上

先测试本地接口：

```powershell
Invoke-RestMethod -Method Post -Uri http://127.0.0.1:57321/backend/status -Body "{}" -ContentType "application/json"
```

若接口正常但插件仍超时，通常是 CDP bridge 或脚本缓存问题：重启 Chimera Codex，或查看日志中的 `renderer.script_loaded` / `bridge.request`。

### macOS 提示无法打开或已损坏

见上文「macOS 安装、Gatekeeper 与旧 App」。当前发行**不声称**受信任安装或已公证。

## 开发

```bash
# 前端检查
cd apps/codex-plus-manager
npm install
npm run check
npm run vite:build

# Rust 检查
cd ../..
cargo fmt --check
cargo test
cargo build --release

# 品牌与去推广门禁
pwsh -File scripts/generate-branding.ps1 -Check
pwsh -File scripts/verify-no-upstream-ads.ps1
```

主要结构：

```text
apps/
  codex-plus-launcher/          静默启动入口
  codex-plus-manager/           Tauri 管理工具
assets/inject/
  renderer-inject.js            注入到 Codex 渲染端的增强脚本
brand/
  product.toml                  品牌单一来源
crates/
  codex-plus-core/              启动、注入、配置、更新、安装、桥接等核心逻辑
  codex-plus-data/              会话数据、导出、Provider 同步
scripts/
  generate-branding.ps1         品牌生成 / -Check
  verify-no-upstream-ads.ps1    去推广扫描门禁
```

## 归属与许可

本项目以 **MIT** 许可发行，基于上游开源项目：

- [BigPizzaV3/CodexPlusPlus](https://github.com/BigPizzaV3/CodexPlusPlus)（Codex++）
- 相关能力亦受益于 [cc-switch](https://github.com/farion1231/cc-switch) 等社区工作

Chimera 品牌、默认 ChimeraHub 中转与去推广改动属于本 fork，不回推上游。可复用的通用 bugfix 可拆分后向上游贡献。

公开仓库：<https://github.com/Duojiyi/chimera-codex>

## 说明

Chimera Codex 是外部增强工具，不修改 Codex App 原始文件。Codex App 更新后若页面结构变化，可能需要更新注入脚本。
