# Chimera Codex 公开发行版设计规格

> 状态：方案已收敛；公开仓库已创建并完成初始化，等待用户批准产品代码开工
> 日期：2026-07-10
> 初始化代码快照：`BigPizzaV3/CodexPlusPlus` @ `a0506ae646172d32b72652794411b4891c90dade`（main，含未发布提交）
> 首发 Release 基线：正式 tag `v1.2.34` @ `c136029`；未发布的 `main` 提交不得伪装为 release-derived 版本
> 发行仓库：`Duojiyi/chimera-codex`（公开）
> 本地工作区：`D:\Desktop\codex plus plus`

> 说明：历史文件名保留 `private-fork`，避免破坏已有链接；实际发行仓库确定为公开仓库。

## 1. 目标

基于上游 Codex++ 做 **ChimeraHub 公开发行版**：

1. **去广告**：移除远端 Ad-List、内置赞助、JOJO 横幅、「推荐内容」、赞赏码、README 赞助区、带推广属性的中转预设。
2. **浅层改名**：用户可见产品名、安装包名、About/托盘文案改为自有品牌（默认工作名：`Chimera Codex` / 管理工具 `Chimera Codex 管理工具`）。
3. **默认中转**：预设与文档默认指向 `https://api.chimerahub.org/v1`（Codex OpenAI 兼容路径必须带 `/v1`）。
4. **公开更新源**：客户端匿名读取本项目公开 GitHub Release 的 `latest.json`，禁止回落到上游官方包。
5. **上游同步自动化**：跟踪上游正式 Release，经同步分支、PR、测试后自动合入并构建发布；冲突时告警人工介入。
6. **低门槛首次配置**：全新安装自动创建并选中 ChimeraHub profile，用户只需填写 API Key 后执行“保存并启用”。
7. **兼容原版升级**：Windows 支持原安装目录覆盖升级；macOS 提供明确的旧 App 迁移与未签名放行流程。

## 2. 非目标（本期不做）

- 不深度改名：`CodexPlusPlus` provider id、`codexplusplus://` 协议、`.codex-session-delete` 状态目录（避免破坏已有用户配置）。
- 不重写 CDP 注入架构；继续依赖上游对 Codex App 的兼容修复。
- 不引入 Windows Authenticode / Apple Developer ID 签名（可后续单独立项）。
- 不把 Chimera 品牌、默认中转和去广告定制回推上游；可复用的通用修复仍允许单独向上游贡献。
- 不在客户端内置 GitHub PAT、API Key 或任何发布凭据。

## 3. 成功标准

| ID | 标准 | 验证方式 |
|----|------|----------|
| S1 | 管理工具与注入菜单无「推荐内容」入口，无 JOJO/赞助商 UI | UI 手工 + 字符串扫描 |
| S2 | 运行时不请求 `BigPizzaV3/Ad-List` | 网络抓包 / 单元测试断言 URL 为空或禁用 |
| S3 | 更新检查 URL 指向本项目公开仓库 `latest.json` | `update.rs` 常量 + 匿名集成测试 |
| S4 | 默认/推荐预设含 ChimeraHub，`baseUrl=https://api.chimerahub.org/v1` | `presets.ts` 审查 |
| S5 | Windows 安装包可构建并安装，显示自有产品名 | CI artifact + 本机安装冒烟 |
| S6 | 上游新正式 Release 被轮询发现；无冲突时经 PR 和门禁后发本项目 Release | Actions/PR/Release 日志 |
| S7 | 同步冲突时不静默吞掉，创建 Issue/失败通知 | 故意制造冲突演练 |
| S8 | `1.2.34-chimera.1` 能升级到 `.2`，并能升级到 `1.2.35-chimera.1` | SemVer 单元测试 + 更新冒烟 |
| S9 | 全新配置自动选中 ChimeraHub；Key 为空时不写入中转配置、不发业务请求 | 设置迁移测试 + 网络断言 |
| S10 | 原版 Windows 覆盖安装无重复入口；macOS 旧 App 有明确迁移结果 | 升级矩阵冒烟 |
| S11 | `latest.json` 和安装资产可匿名下载，安装前 SHA-256 校验通过 | 无登录下载 + 篡改用例 |
| S12 | Windows x64、macOS x64/arm64 均产出安装资产；macOS 明示未公证 | CI artifact + 双架构检查 |

## 4. 架构

```text
upstream BigPizzaV3/CodexPlusPlus Releases
        │  schedule / workflow_dispatch 轮询正式 tag
        ▼
public GitHub repo (`Duojiyi/chimera-codex`)
        │
        ├─ brand/product.toml          # 品牌与发布配置的机器可读真相源
        ├─ scripts/generate-branding.* # 生成/校验 Rust、TS 与打包参数
        ├─ scripts/sync-upstream.ps1   # fetch tag + sync branch + gate
        └─ .github/workflows/
              sync-upstream.yml
              pr-build.yml
              release-assets.yml       # build → checksum → publish
        │
        ▼
public Release + latest.json + SHA-256
        │
        ▼
已安装客户端 check_for_update()
```

### 4.1 品牌配置（单一真相源）

**真相源固定为** `brand/product.toml`。Rust/TypeScript 中的生成文件不允许手工编辑；NSIS、DMG 和 Actions 从同一配置读取参数，CI 用 `scripts/generate-branding.ps1 -Check` 检查生成结果没有漂移。

至少包含：

- `DISPLAY_SILENT_NAME` / `DISPLAY_MANAGER_NAME`
- `PUBLISHER` / `REPOSITORY` / `LATEST_JSON_URL`（固定为 `Duojiyi/chimera-codex` 的真实公开地址，禁止提交假 owner）
- `ADS_ENABLED = false`
- `DEFAULT_RELAY_*`（ChimeraHub + `/v1`）
- `DEFAULT_RELAY_MODEL = "gpt-5.5"`（首个 Release 前用真实 Key 验证）
- `ARTIFACT_PREFIX = "ChimeraCodex"`
- 一期二进制名保持 `codex-plus-plus*`（不在 branding 里改 bin 名）

生成流程至少产出 Rust `branding.rs` 和前端 `branding.generated.ts`；打包脚本通过参数或读取 TOML 获得显示名、Publisher、仓库和资产前缀。无法直接读取 TOML 的文件必须由 `-Check` 做精确一致性验证，不能只扫描上游 URL。

### 4.2 去广告策略

| 触点 | 动作 |
|------|------|
| `DEFAULT_AD_LIST_URLS` | `ads_enabled=false` 时在网络入口短路 `fetch_ad_list`；纯 normalize 函数保持可测试 |
| `append_builtin_sponsors` | 删除调用；无测试价值时同时删除函数与生产 include |
| `App.tsx` JOJO 概览 | 删除面板与样式 |
| 侧栏「推荐内容」路由 | 删除入口与页面 |
| `renderer-inject.js` 推荐/赞赏 Tab | 删除入口 |
| `assets.rs` 赞赏码 data URI | 停止生成并停止注入 `__CODEX_PLUS_SPONSOR_IMAGES__` |
| `presets.ts` 推广项 | 删除 JOJO/带邀请码项；新增 ChimeraHub |
| `script_market.rs` | 一期禁用上游 `CodexPlusPlusScriptMarket`（空 manifest 或短路 fetch） |
| README 赞助区 | 替换为 ChimeraHub 说明 |
| `tauri.conf.json` / 窗口 title | 浅层改名 |

去广告验收以“没有网络请求、没有入口、没有赞赏图片注入”为准。测试 fixture 中可保留虚构域名，但生产路径不允许保留上游广告 URL。

### 4.3 改名策略（分两期）

**一期（本计划）写死：**

- 显示名、窗口标题、About、托盘、NSIS `Name`/`DisplayName`/`Publisher`、`install/windows.rs` 快捷方式与卸载 DisplayName、README 标题
- 更新源改为本项目公开 Release
- **产物文件名必须**改为 `ChimeraCodex-*-windows-x64-setup.exe`（及对应 zip/dmg），并在**同一 PR** 修改 `is_windows_installer_asset` **与** `is_macos_installer_asset`
- **NSIS `InstallDir` 一期不改**（仍可用 `Programs\Codex++`），只改显示名，降低迁移成本
- Windows 保留原注册表安装定位键和协议 id，安装时删除旧、新两套快捷方式后只创建 Chimera 入口；卸载同时清理旧入口
- macOS 新包使用 Chimera App 显示名。由于 DMG 拖拽无法用新文件名覆盖旧 `Codex++.app`，首次迁移必须检测/提示旧 App，并提供明确的退出、替换和清理步骤
- 一期沿用现有图标，避免在没有正式品牌资产时临时生成低质量图标；新图标单独立项

**二期（可选）：深层**

- 二进制名、Bundle ID、`codexplusplus://`、provider id、状态目录、NSIS InstallDir 迁移

### 4.4 上游同步策略

跟踪对象固定为：**上游正式 GitHub Release tag**。不把上游 `main` 的每个提交自动发布给用户。

流程：

1. `sync-upstream.yml` 定时或手工查询上游最新正式 Release，忽略 draft/prerelease。
2. 若 tag 已记录为已处理则安全退出，保证幂等。
3. 全历史 checkout，校验 `origin` 是本项目公开仓库、`upstream` 是 `BigPizzaV3/CodexPlusPlus`。
4. 从本项目 `main` 创建 `sync/upstream-vX.Y.Z`，merge 上游 tag，不直接修改用户本地 `main`。
5. 将版本设为 `X.Y.Z-chimera.1`，重新生成品牌文件，运行扫描、Rust 测试、前端检查和双平台构建门禁。
6. 推送同步分支并创建 PR；检查全绿后自动合并。冲突或失败保留分支/PR，并创建或更新一个去重 Issue。
7. `main` 上的新版本由发布 workflow 构建全部资产、生成哈希和 `latest.json`，全部成功后才创建公开 Release。

冲突热点预期：`App.tsx`、`renderer-inject.js`、`ads.rs`、`update.rs`、NSIS。

自动化优先使用仓库级 GitHub App；若初期使用 fine-grained PAT，命名为 `CHIMERA_AUTOMATION_TOKEN`，仅授予目标仓库 Contents/Pull requests/Issues/Actions 所需最小权限。客户端永远不接触该 token。

### 4.5 版本与更新协议

- 版本格式：`<upstream-major>.<minor>.<patch>-chimera.<revision>`。
- 同一上游版本上的 Chimera 修复递增 `revision`；同步到新上游版本时重置为 `1`。
- Cargo workspace version 是构建版本源，生成/校验脚本同步 `package.json` 与 `tauri.conf.json`。
- macOS plist 不直接写 prerelease 字符串：`CFBundleShortVersionString=X.Y.Z`，`CFBundleVersion` 使用 `brand/product.toml` 中每次发布严格递增的正整数 `macos_build_number`；应用 About/更新器仍显示完整 Chimera SemVer。
- 更新比较使用标准 SemVer 库，不能继续截断 `-chimera.N`。
- GitHub Release 虽使用带后缀 tag，但发布属性为正式 Release，不标记为 prerelease，以便 `/releases/latest/download/latest.json` 稳定工作。
- `latest.json` 每个资产包含 `name`、`url`、`sha256`、`size`；客户端下载后先验证大小和 SHA-256，再启动安装器。
- 更新器只接受 `ChimeraCodex-<version>-<platform>-<arch>` 严格命名，并保留 macOS 原有 native-arch 排序逻辑。
- 不回落到上游 URL；网络、JSON、哈希或资产选择失败时只报告错误，不启动任何文件。

### 4.6 首次配置与升级保护

全新安装（`settings.json` 不存在）：

1. 自动创建并选中 `ChimeraHub` profile，启用供应商配置总开关。
2. 固定 `base_url=https://api.chimerahub.org/v1`、`protocol=responses`、`model=gpt-5.5`；首个 Release 前必须用真实 ChimeraHub Key 验证该默认模型。
3. UI 首屏突出 API Key 输入和单一主操作“保存并启用”；Key 为空时按钮禁用。
4. 保存成功后才写 `config.toml` / `auth.json` 并启用中转；此前不向 ChimeraHub 发业务请求。

覆盖升级（已有 settings、profile 或已配置 Codex 文件）：

- 不新增或抢占 active profile，不覆盖 Base URL、模型、Key 或总开关。
- 仅把 ChimeraHub 加入可选预设；用户主动选择后才切换。
- 日志、Issue、Actions 输出和错误消息不得包含 API Key。

## 5. 风险与缓解

| 风险 | 缓解 |
|------|------|
| 更新器仍指向上游 | S3 强制改 `DEFAULT_LATEST_JSON_URL`；CI 扫描禁止上游 URL |
| 公开仓库地址变更或误填 | `brand/product.toml` 固定 `Duojiyi/chimera-codex`，占位值和上游 URL 门禁必须失败 |
| `-chimera.N` 被旧解析器忽略 | 使用标准 SemVer；覆盖 `.1→.2` 与跨上游版本测试 |
| Chimera SemVer 不符合 macOS bundle 数字格式 | plist 使用纯数字 marketing/build version；发布门禁要求 `macos_build_number` 高于上一 tag |
| 产物改名导致更新选包失败 | **同 PR** 改 Win+macOS matcher + `tests/updater.rs` 样例名 |
| ScriptMarket 仍拉上游 | 一期短路 `DEFAULT_MARKET_INDEX_URL` |
| CDP 注入随 Codex 失效 | 必须持续跟上游；同步失败告警 |
| 多语言/打包品牌值漂移 | `brand/product.toml` + 生成脚本 `-Check`，禁止手工双写 |
| 自动同步重复 PR/Issue/Release | tag 状态、固定分支名、并发组和 Issue 去重键 |
| 原版覆盖后出现重复入口 | Windows 清理 legacy shortcut；macOS 执行旧 App 迁移验收 |
| macOS 未可信签名/未公证 | 仅 ad-hoc sign；Release 与 README 明示 Gatekeeper 放行步骤，不声称受信任安装 |
| 下载损坏或资产与 manifest 不一致 | `latest.json` 携带 SHA-256 和大小，下载后强制验证 |
| Release 账号或 manifest 同时被攻破 | SHA-256 不能提供独立真实性；启用 2FA、最小发布权限、branch protection 和不可复用 tag |
| 自动默认中转覆盖旧用户 | 仅对未配置的全新 settings 引导；升级路径保持原值 |

## 6. 决策状态

已确认：

1. 产品显示名：`Chimera Codex` / `Chimera Codex 管理工具`。
2. 仓库公开，真实地址为 `Duojiyi/chimera-codex`；不得使用虚构的 `chimera-org/chimera-codex`。
3. 一期不改二进制名、provider id、协议 id、状态目录和 Windows InstallDir。
4. 跟踪上游正式 Release；同步使用 merge + PR，不 rebase 本项目 `main`。
5. 全新用户默认选中 ChimeraHub，Key 为空不应用；升级用户配置不被覆盖。
6. Windows 与 macOS x64/arm64 都构建；macOS 一期无 Developer ID 签名/公证。

开工时唯一外部决策门：用户明确批准开始产品代码实施；仓库地址与 remote 初始化已完成，品牌配置和 Actions 权限将在获批后按任务执行。

## 7. 参考触点（代码事实）

- 广告：`crates/codex-plus-core/src/ads.rs`
- 更新：`crates/codex-plus-core/src/update.rs`
- 安装名常量：`crates/codex-plus-core/src/install/mod.rs`
- macOS 安装与 bundle：`crates/codex-plus-core/src/install/macos.rs`
- 预设：`apps/codex-plus-manager/src/presets.ts`
- UI：`apps/codex-plus-manager/src/App.tsx`
- 首次启动设置：`crates/codex-plus-core/src/settings.rs`
- 注入：`assets/inject/renderer-inject.js`
- Stepwise 用户文案：`assets/inject/stepwise-inject.js`
- 发布：`.github/workflows/release-assets.yml`
- 安装器：`scripts/installer/windows/CodexPlusPlus.nsi`
- macOS 打包：`scripts/installer/macos/package-dmg.sh`
