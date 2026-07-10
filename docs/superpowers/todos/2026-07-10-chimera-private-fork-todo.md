# Chimera Codex 公开发行版 — TODO Checklist

> 对应：
> - Spec: `docs/superpowers/specs/2026-07-10-chimera-private-fork-design.md`
> - Plan: `docs/superpowers/plans/2026-07-10-chimera-private-fork.md`
> 日期：2026-07-10
> 状态：产品代码已开工（TDD + 双盲）；本地提交，待用户决定是否推送

## 决策与开工门

- [x] D1. 显示名：`Chimera Codex` / `Chimera Codex 管理工具`
- [x] D2. 发行仓库公开；真实地址为 `Duojiyi/chimera-codex`，不使用假占位
- [x] D3. 一期不改二进制名、provider id、协议 id、状态目录
- [x] D4. 跟踪上游正式 Release；同步分支 → PR → checks → auto-merge
- [x] D5. 版本采用 `X.Y.Z-chimera.N`，更新器按完整 SemVer 比较
- [x] D6. NSIS `InstallDir` 保持 `Programs\Codex++`，支持原版覆盖升级
- [x] D7. 产物名固定 `ChimeraCodex-*`，严格匹配平台与架构
- [x] D8. 全新安装默认选中 ChimeraHub；Key 为空不应用；升级不覆盖已有配置
- [x] D9. Windows x64、macOS x64/arm64 都构建；macOS 仅 ad-hoc sign，不 notarize
- [x] D10. 用户明确授权开工
- [ ] D11. 写入真实 repository，配置 branch protection 和最小权限自动化 token（仓库与 branch protection 已完成；自动化 token/Actions 最小权限仍待实施）
- [x] D12. 每个 checkbox 坚持 Red → Green → 双盲审计；每个大任务另做聚合双盲审计

## P0 — 仓库、品牌、去推广与安全更新

- [x] T1. 安全规范 remotes：公开仓库=`origin`，BigPizzaV3=`upstream`，补全历史并阻断 upstream push
- [x] T2. 新增 `brand/product.toml`、生成脚本、Rust/TS generated branding 与 `-Check`
- [ ] T3. 短路生产 `ads.rs` 网络入口，删除 builtin append，保留纯 normalize 测试
- [ ] T4. 停止 `assets.rs` sponsor 变量/图片注入，更新 `cdp_bridge.rs`
- [ ] T5. 清理 `renderer-inject.js` 推荐/赞赏/Ad-List
- [ ] T6. 禁用远端 ScriptMarket，保留本地脚本管理
- [ ] T7. 删除 App JOJO/推荐 UI、CSS、i18n 与生成键
- [ ] T8. 品牌化 About、HTML title、Tauri 窗口/托盘、Stepwise 用户文案
- [ ] T9. 引入标准 SemVer，统一 Cargo/package/Tauri 版本
- [ ] T10. updater 改公开 latest.json，严格 Win/macOS+arch matcher
- [ ] T11. latest.json 增加 size/SHA-256，下载验证后才启动安装器
- [ ] T12. updater 覆盖 `.1→.2`、跨上游版本、错误哈希/大小/通道测试

## P1 — Key-first 中转与覆盖升级

- [ ] T13. 完整 ChimeraHub preset：`/v1`、responses、已验证默认模型
- [ ] T14. 仅 settings 文件不存在时创建并选中 Chimera profile
- [ ] T15. Key-first “保存并启用”原子命令；空 Key/失败不写 live config、不泄密
- [ ] T16. 现有 settings/profile/active id 升级保持测试
- [ ] T17. `install/mod.rs/windows.rs/macos.rs` 显示名接 branding，保留 legacy 常量
- [ ] T18. NSIS 原目录覆盖、旧快捷方式/乱码清理、新入口、卸载回归
- [ ] T19. macOS 新 App/DMG 名、纯数字 plist 版本、legacy App 检测提示、release 验证路径
- [ ] T20. README/README_EN 去推广，保留 MIT 归属，写双平台升级/Gatekeeper 指南
- [ ] T21. branding + no-promo 门禁通过，allowlist 仅覆盖明确 fixture/legacy 常量

## P2 — CI、上游同步与发布

- [ ] T22. `pr-build.yml` 跑生成检查、扫描、Rust、前端和三平台构建
- [ ] T23. `release-assets.yml` 改为 build-first；全部成功后 draft → upload → publish
- [ ] T24. 三平台产物命名一致，生成 size/SHA-256/latest.json
- [ ] T25. 写安全 `sync-upstream.ps1`，DryRun 前后 refs/files/status 不变
- [ ] T26. `sync-upstream.yml` 轮询正式 Release、去重、建 PR、auto-merge、Issue 去重
- [ ] T27. 同步 workflow 不直接创建 Release；main 新版本由发布 workflow 处理
- [ ] T28. 冲突/test/hash failure 演练：main/latest 不变，无重复 Issue/Release
- [ ] T29. 首次公开 Release：匿名 latest/asset 下载成功

## 验收（S1–S12）

- [ ] V1. UI 无推荐/JOJO/赞助/赞赏（S1）
- [ ] V2. 无 Ad-List 网络请求（S2）
- [ ] V3. 更新检查匿名读取公开 latest.json，不回落上游（S3）
- [ ] V4. 全新安装默认 ChimeraHub，Key-first，`/v1`；升级不覆盖（S4/S9）
- [ ] V5. Windows 原版覆盖安装无重复入口，更新器识别资产并校验哈希（S5/S10/S11）
- [ ] V6. 上游正式 Release → PR → checks → merge → build-first Release（S6）
- [ ] V7. 冲突告警可用（S7）
- [ ] V8. `.1→.2`、跨上游版本比较正确（S8）
- [ ] V9. Windows + macOS x64/arm64 资产齐全；macOS 未 notarize 说明清楚（S12）
- [ ] V10. macOS 原版旧 App 被检测，完成迁移后只留新入口（S10）
- [ ] V11. `generate-branding -Check` 与 `verify-no-upstream-ads.ps1` exit 0
- [ ] V12. `cargo fmt --check`、`cargo test`、manager build/typecheck 全绿
- [ ] V13. ScriptMarket 不访问上游，本地脚本仍可用
- [ ] V14. 错误哈希/大小时不启动安装器，latest 仍指向上一成功版本（S11）

## 明确不做（防范围蔓延）

- [x] 不改 `CodexPlusPlus` provider id
- [x] 不改 `codexplusplus://` 协议
- [x] 不改 `.codex-session-delete` 目录
- [x] 一期不改 NSIS `InstallDir`
- [x] 一期不改二进制文件名
- [x] 一期不做 Authenticode / Developer ID 可信签名与公证（macOS 保留 ad-hoc sign）
- [x] 不回推 Chimera 品牌定制；通用修复可独立贡献上游
- [x] 不在客户端内置 GitHub token 或 ChimeraHub Key
