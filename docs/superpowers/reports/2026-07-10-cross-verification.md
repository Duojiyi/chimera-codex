# 交叉验证报告 — Chimera Codex 公开发行版文档

**日期：** 2026-07-10

**对照代码：** `D:\Desktop\codex plus plus`（初始化快照为上游 `main` / `a0506ae646172d32b72652794411b4891c90dade`；首发 release 基线为正式 tag `v1.2.34` / `c136029`）

**本轮依据：** 用户确认仓库公开、跟踪上游 Release、全新安装默认 ChimeraHub、支持原版覆盖升级、Windows/macOS 均构建且 macOS 暂无可信签名。
**文档：**
- `docs/superpowers/specs/2026-07-10-chimera-private-fork-design.md`
- `docs/superpowers/plans/2026-07-10-chimera-private-fork.md`
- `docs/superpowers/todos/2026-07-10-chimera-private-fork-todo.md`
- `docs/superpowers/README.md`
- `AGENTS.md`

## 方法

1. 对照 S1–S12、Plan Task 0–10（含 Task 2b）与 TODO T/V 映射
2. 核对 updater 的版本解析、匿名下载、平台/架构选择和下载执行路径
3. 核对 GitHub Actions 事件递归、build/publish 顺序和同步幂等性
4. 核对当前 remotes/shallow 状态、Windows NSIS、macOS DMG、旧安装迁移
5. ripgrep 广告、赞赏、上游 URL、用户可见旧品牌和遗漏测试触点
6. 核对首次 settings 默认值与已有设置兼容路径
7. 检查文档是否进入 Git 版本控制、是否存在旧方案路由冲突
8. 机械检查 Markdown fence、S/D/T/V 编号连续性、Modify 路径存在性和 whitespace

## 本轮发现与修复

| 检查 | 初始问题 | 文档修复 |
|------|----------|----------|
| 项目定位 | `AGENTS.md` 仍以单 feature 上游 PR 为唯一目标 | 改为公开 Chimera 发行版，同时保留 model-catalog 能力和通用上游贡献 |
| 更新可达性 | 私有 Release 无匿名下载方案 | 仓库改公开；S3/S11 要求匿名 latest/asset 冒烟 |
| 版本比较 | parser 会忽略 `-chimera.N` | Task 5 改标准 SemVer 并统一 Cargo/package/Tauri 版本 |
| macOS 版本元数据 | DMG 脚本会把完整 prerelease 字符串同时写入两个 plist 版本字段 | Task 5/6 规定完整 app SemVer 与纯数字 plist 版本分离 |
| Actions 触发 | `GITHUB_TOKEN` 创建 Release 后期待触发 release workflow | Task 8 改 build-first，同一 workflow 最后 publish，不依赖事件递归 |
| 同步安全 | 初始化时 origin=upstream 且 shallow；main 默认跟踪上游可能误推 | 已完成 origin/upstream 分离、补全历史并阻断 upstream push；首次基线推送后修正 tracking；Task 9 仍需实现 remote guard、隔离分支、PR、并发与幂等 |
| 品牌真相源 | Rust 常量 + NSIS/TS/YAML 手工双写 | 改 `brand/product.toml` + 生成脚本 + `-Check` |
| 首次配置 | “默认中转”未区分新装与升级 | 仅 settings 不存在时创建 active Chimera profile；Key 空不应用，旧设置不覆盖 |
| 下载完整性 | 下载后直接启动，无哈希 | latest schema 增加 size/SHA-256，验证后才启动 |
| 安装迁移 | Windows 旧快捷方式、macOS 改 App 名后并存未处理 | Windows legacy cleanup；macOS 检测提示和人工迁移验收 |
| 触点完整性 | 漏 `index.html`、Stepwise、cdp_bridge、macOS workflow 旧路径 | 已补进 File map、Task 2/3/6 和测试 |
| 扫描顺序 | scanner 落地时 README 尚未清理，CI 必红 | Task 7 先改 README，再启用门禁 |
| 文档生命周期 | 旧 Python/Rust/context 计划无状态路由 | 新增 `docs/superpowers/README.md` Active/Superseded 索引 |
| Git 可交付性 | 新 Chimera 文档被 `.gitignore` 忽略 | `.gitignore` 只放行 Active 四件套和索引 |
| 机械一致性 | 新文档未进入常规 diff，容易漏查格式与路径 | S1–S12、D1–D11、T1–T29、V1–V14 连续；fence 成对；所有 Modify 路径存在；whitespace check 通过 |

## Spec / Plan / TODO 一致性

- S1/S2 → Task 2/2b/3/7 → T3–T8/T21 → V1/V2/V13
- S3/S8/S11 → Task 5/8/10 → T9–T12/T23–T24 → V3/V5/V8/V14
- S4/S9 → Task 4 → T13–T16 → V4
- S5/S10/S12 → Task 6/8/10 → T17–T19/T22–T24 → V5/V9/V10
- S6/S7 → Task 0/9/10 → T1/T25–T29 → V6/V7

TODO 中 D1–D9 是用户已确认或授权按最佳实践确定的方案；D10 保留为产品代码开工授权，D11 的仓库创建已完成但 branch protection、自动化凭据和品牌真相源仍待实施。

## 复验结论

**PASS（文档与仓库初始化层面）→ 可以进入等待产品代码开工状态。**

未发现仍会阻断文档初始化的内部矛盾。公开仓库真实地址、完整历史和 remote 安全门已核验；剩余外部决策只有用户是否批准产品代码开工，且 branch protection、品牌真相源和 CI 权限仍属于后续实现任务。本文档结论不代表产品代码已经实现或构建通过。

## 证据摘录

- 版本后缀被截断：`crates/codex-plus-core/src/update.rs:42-67`
- updater 匿名请求/直接启动：`update.rs:169-213`
- macOS arch 排序必须保留：`update.rs:255-315`
- 当前版本分散：root `Cargo.toml`、manager `package.json`、`src-tauri/tauri.conf.json`
- 默认 settings/profile：`crates/codex-plus-core/src/settings.rs:128-152, 328-378, 558-572`
- sponsor 注入回归：`crates/codex-plus-core/tests/cdp_bridge.rs:49,113`
- 用户可见遗漏：manager `index.html:6`、`assets/inject/stepwise-inject.js:1141,1193`
- Windows legacy 安装：NSIS `InstallDir`/registry/shortcut 在 L9-80，且 L39-40 存在历史乱码
- macOS 新旧 App 文件名迁移：`package-dmg.sh:120-132`、release workflow bundle check L131-145
- 当前 Git 状态：`origin=Duojiyi/chimera-codex`、`upstream=BigPizzaV3/CodexPlusPlus`，仓库已非 shallow；`main` 基线已推送并跟踪 `origin/main`，`upstream` push 已阻断，`main` protection 与 Actions read-only 已核验
- 文档跟踪：`.gitignore` 原规则忽略整个 `docs/superpowers/`，现仅放行 Active 文档与索引
