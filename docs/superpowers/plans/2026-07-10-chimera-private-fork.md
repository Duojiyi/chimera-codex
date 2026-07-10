# Chimera Codex 公开发行版 Implementation Plan

> 本计划不依赖特定 agent skill。实施者按 task 顺序执行，每个 task 先测试、再实现、再验证；checkbox 是唯一进度记录。

**Goal:** 将上游 Codex++ 制作为 ChimeraHub 公开发行版：去广告、浅层改名、全新安装默认中转、公开更新源，并建立跟踪上游正式 Release 的同步、构建和发布流水线。

**Architecture:** 以 `brand/product.toml` 作为机器可读真相源并生成 Rust/TS 品牌值；广告网络入口短路；公开 Release 提供带 SHA-256 的 `latest.json`；同步 workflow 创建 PR，发布 workflow 在构建全部成功后创建 Release。

**Tech Stack:** Rust (Cargo workspace)、Tauri/Vite 管理端、NSIS、GitHub Actions、PowerShell 同步脚本。

## TDD 与双盲审计门禁

Plan 中每个 `Step` checkbox 都是最小交付单元，TODO 中的 `T*` checkbox 只有在其全部 Step 与聚合审计通过后才可勾选。执行顺序固定为：

1. 写出会失败的行为测试或可验证前置条件，并记录 Red 证据。
2. 仅实现令该测试通过所需的最小改动，记录 Green 和针对性回归。
3. 审计 A 独立对照 Spec/本任务验收，不阅读实施者的结论；审计 B 独立检查 diff、边界、迁移和回归面，不阅读审计 A 结论。
4. 两份审计均无未关闭问题后，写入 `docs/superpowers/audits/<task-id>.md`，才可勾选该 checkbox。

Task 完成时再做聚合双盲审计：审计者复核所有子任务证据、接口契约、测试空洞、文档和发布/迁移影响。任何审计发现都回到 Red 阶段修复，不能以“后续处理”绕过门禁。

### 聚合审计账本

每个大 Task 都必须有独立的聚合审计文件；文件不存在或结论未通过时，不得勾选对应 TODO 项：

| 大 Task | 聚合审计文件 | 覆盖范围 |
|---------|--------------|----------|
| Task 0 | `audits/task-0-aggregate.md` | 仓库可见性、remote/tracking、分支保护、权限 |
| Task 1 | `audits/task-1-aggregate.md` | branding 真相源、生成漂移和占位门禁 |
| Task 2/2b | `audits/task-2-aggregate.md` | 广告、赞助注入和 ScriptMarket 网络入口 |
| Task 3 | `audits/task-3-aggregate.md` | 管理端 UI、注入菜单、品牌文案 |
| Task 4 | `audits/task-4-aggregate.md` | preset、首次配置、Key-first 原子性和升级兼容 |
| Task 5 | `audits/task-5-aggregate.md` | SemVer、latest.json、平台架构选择和完整性校验 |
| Task 6 | `audits/task-6-aggregate.md` | Windows/macOS 安装器与覆盖升级 |
| Task 7 | `audits/task-7-aggregate.md` | README、扫描门禁和归属声明 |
| Task 8 | `audits/task-8-aggregate.md` | 三平台 build-first 发布流水线 |
| Task 9 | `audits/task-9-aggregate.md` | 正式 Release 同步、PR、幂等与冲突告警 |
| Task 10 | `audits/task-10-aggregate.md` | 全量回归、升级矩阵、首次公开 Release |

每个 Step 的两份独立审计记录使用 `audits/task-<n>-step-<m>-a.md` 与 `audits/task-<n>-step-<m>-b.md`；审计者不得读取另一份结论后再作判断。

**Resolved defaults:**
- 显示名：`Chimera Codex` / `Chimera Codex 管理工具`
- 发行仓库：公开；固定为 `Duojiyi/chimera-codex`，禁止假占位通过 CI
- 默认中转：`https://api.chimerahub.org/v1`
- 一期不改二进制文件名 `codex-plus-plus(.exe)`
- 版本：上游 `X.Y.Z` 对应 `X.Y.Z-chimera.N`
- 上游源：正式 Release tag，不自动发布 upstream/main 快照
- Windows + macOS x64/arm64 均构建；macOS 无 Developer ID 签名/公证

---

## File map

| File | Responsibility |
|------|----------------|
| `brand/product.toml` | 产品名、仓库、更新 URL、广告开关、默认中转、资产前缀 |
| `scripts/generate-branding.ps1` | 生成 Rust/TS 品牌文件；`-Check` 检测漂移和占位值 |
| `crates/codex-plus-core/src/branding.rs` | 生成的 Rust 品牌常量，不手改 |
| `apps/codex-plus-manager/src/branding.generated.ts` | 生成的前端品牌常量，不手改 |
| `crates/codex-plus-core/src/ads.rs` | 尊重 `ads_enabled`；清空远端/内置赞助 |
| `crates/codex-plus-core/src/update.rs` | SemVer 比较、公开 latest.json、严格选包与 SHA-256 校验 |
| `Cargo.toml` / core `Cargo.toml` | 版本源与 SemVer 依赖 |
| `apps/codex-plus-manager/package.json` / `src-tauri/tauri.conf.json` | 由版本校验步骤保持一致 |
| `crates/codex-plus-core/src/install/mod.rs` | 显示名常量改读 branding |
| `crates/codex-plus-core/src/install/macos.rs` | macOS bundle 显示名与旧 App 迁移契约 |
| `crates/codex-plus-core/src/script_market.rs` | 一期禁用上游 ScriptMarket |
| `crates/codex-plus-core/src/assets.rs` | 停止赞赏码注入 |
| `apps/codex-plus-manager/src/presets.ts` | ChimeraHub 预设；删推广中转 |
| `crates/codex-plus-core/src/settings.rs` | 仅在 settings 文件不存在时创建 ChimeraHub 默认 profile |
| `apps/codex-plus-manager/src/App.tsx` | 去 JOJO/推荐路由；About 改公开仓库；Key-first 启用体验 |
| `apps/codex-plus-manager/src/styles.css` | 删除 jojocode 样式块 |
| `apps/codex-plus-manager/src/i18n-en.ts` | 删除已无入口的推荐文案键 |
| `tools/i18n-keys.json` | 同步移除死文案键 |
| `apps/codex-plus-manager/index.html` | HTML title 品牌化 |
| `apps/codex-plus-manager/src-tauri/tauri.conf.json` | productName/title 浅层改名 |
| `apps/codex-plus-manager/src-tauri/src/lib.rs` | 窗口 title 字符串 |
| `assets/inject/renderer-inject.js` | 去推荐/赞赏/Ad-List URL/About 上游链接 |
| `assets/inject/stepwise-inject.js` | 替换用户可见的旧 Manager 名称 |
| `scripts/installer/windows/CodexPlusPlus.nsi` | DisplayName/Publisher/OutFile（InstallDir 一期不动） |
| `scripts/installer/macos/package-dmg.sh` | DMG/app 显示名与产物名 |
| `crates/codex-plus-core/src/install/windows.rs` | 快捷方式名、DisplayName、Publisher 文案 |
| `.github/workflows/release-assets.yml` | 公开产物命名、校验和与 latest.json |
| `.github/workflows/sync-upstream.yml` | 监视上游并自动合入 |
| `scripts/sync-upstream.ps1` | 本地/CI 同步入口 |
| `scripts/verify-no-upstream-ads.ps1` | 字符串扫描门禁 |
| `crates/codex-plus-core/tests/cdp_bridge.rs` | 赞赏/广告注入回归断言 |
| `docs/superpowers/todos/2026-07-10-chimera-private-fork-todo.md` | 执行清单 |
| `README.md` / `README_EN.md` | 去赞助，写 Chimera 说明 |

---

### Task 0: 公开仓库与安全初始化

**Prerequisite:** 公开仓库 `Duojiyi/chimera-codex` 已由用户授权创建；产品代码任务仍须等待用户明确批准。

**Files:**
- Existing: public GitHub repository `Duojiyi/chimera-codex`
- Modify: local Git remotes
- Configure: branch protection / Actions permissions / `CHIMERA_AUTOMATION_TOKEN`（仅在 GitHub App 暂不可用时）
- Create: `docs/superpowers/audits/task-0-*.md`

- [x] **Step 1: Create and verify repository**

仓库已创建为公开空仓库 `Duojiyi/chimera-codex`，未初始化额外 README/License；首次基线推送后确认 Release 资产允许匿名访问。

- [x] **Step 2: Normalize remotes safely**

初始化前 clone 的 `origin` 曾指向上游且是 shallow repository；该迁移现已完成。后续脚本仍必须先打印并核对 URL，再执行单项变更：

1. 将现有上游 remote 规范为 `upstream=https://github.com/BigPizzaV3/CodexPlusPlus.git`。
2. 将新公开仓库设为 `origin`，分别校验 fetch/push URL。
3. 补全历史后确认 `git merge-base main upstream/main` 可用。
4. 为 `upstream` 写入明确不可用的 `pushurl`，让误推上游在本地直接失败；首次基线只允许显式 `git push -u origin main`，随后验证 `main` 跟踪 `origin/main`。
5. 禁止脚本在 remote URL 不符合预期、工作树不干净或当前存在 merge/rebase 时继续。

- [x] **Step 3: Configure repository policy**

- `main` 禁止 force-push，要求 PR 与核心检查通过。
- Actions 默认只读，具体 workflow 显式声明最小权限。
- 自动同步优先使用 GitHub App；fallback token 使用 fine-grained PAT，保存在 `CHIMERA_AUTOMATION_TOKEN`，不写入仓库或客户端。
- 设置 `sync-upstream` concurrency group，禁止两个同步并行。

当前已启用 PR、1 次批准、dismiss stale review、last-push approval、线性历史、禁止强推/删除和 conversation resolution；CI 尚未落地，因此 required status checks 留待真实 check 名产生后追加。

- [ ] **Step 4: Record repository in brand config**

只有真实地址确定后才写 `brand/product.toml`；任何 `TBD`、`example`、`chimera-org` 占位值都必须使 `-Check` 失败。

- [x] **Step 5: Task 0 aggregate audit**

复核公开可见性、匿名访问、remote fetch/push URL、full history、branch protection、token 最小权限和审计证据。Task 0 的审计记录必须在任何产品代码任务开始前通过。

---

### Task 1: Branding 模块（单一真相源）

**Files:**
- Create: `brand/product.toml`
- Create: `scripts/generate-branding.ps1`
- Generate: `crates/codex-plus-core/src/branding.rs`
- Generate: `apps/codex-plus-manager/src/branding.generated.ts`
- Modify: `crates/codex-plus-core/src/lib.rs`（`pub mod branding;`）
- Test: `crates/codex-plus-core/tests/branding.rs`

- [x] **Step 1: Write the failing test**

```rust
// crates/codex-plus-core/tests/branding.rs
use codex_plus_core::branding::{ADS_ENABLED, ARTIFACT_PREFIX, DEFAULT_RELAY_BASE_URL,
    DEFAULT_RELAY_MODEL, DISPLAY_MANAGER_NAME, DISPLAY_SILENT_NAME, LATEST_JSON_URL,
    PUBLISHER, REPOSITORY};

#[test]
fn public_chimera_branding_does_not_point_at_upstream_release() {
    assert!(!ADS_ENABLED);
    assert_eq!(DISPLAY_SILENT_NAME, "Chimera Codex");
    assert_eq!(DISPLAY_MANAGER_NAME, "Chimera Codex 管理工具");
    assert_eq!(PUBLISHER, "ChimeraHub");
    assert!(!REPOSITORY.contains("TBD"));
    assert!(LATEST_JSON_URL.contains(REPOSITORY));
    assert!(!LATEST_JSON_URL.contains("BigPizzaV3/CodexPlusPlus"));
    assert_eq!(DEFAULT_RELAY_BASE_URL, "https://api.chimerahub.org/v1");
    assert_eq!(DEFAULT_RELAY_MODEL, "gpt-5.5");
    assert_eq!(ARTIFACT_PREFIX, "ChimeraCodex");
}
```

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test -p codex-plus-core --test branding -- --nocapture`
Expected: FAIL（brand config / generated module missing）

- [x] **Step 3: Write minimal implementation**

`brand/product.toml` 至少声明显示名、Publisher、真实 repository、latest URL、广告开关、ChimeraHub URL、默认模型 `gpt-5.5`、`ChimeraCodex` 资产前缀和正整数 `macos_build_number`。生成脚本以确定性顺序写 Rust/TS 文件，并提供 `-Check`：重新生成到临时位置后逐字节比较，不修改工作树。

在 `lib.rs` 按字母序插入 `pub mod branding;`。NSIS/DMG/Actions 不再复制 Rust 常量，改为从 TOML 读取或接收生成脚本输出。

- [x] **Step 4: Run test to verify it passes**

Run: `cargo test -p codex-plus-core --test branding`
Run: `pwsh -File scripts/generate-branding.ps1 -Check`
Expected: 两项均 PASS，且 `git diff --exit-code` 无生成漂移

- [x] **Step 5: Commit**

```bash
git add brand/product.toml scripts/generate-branding.ps1 crates/codex-plus-core/src/branding.rs apps/codex-plus-manager/src/branding.generated.ts crates/codex-plus-core/src/lib.rs crates/codex-plus-core/tests/branding.rs
git commit -m "feat: add generated Chimera branding configuration"
```

---

### Task 2: 短路广告链路

**Files:**
- Modify: `crates/codex-plus-core/src/ads.rs`
- Modify: `crates/codex-plus-core/tests/ads.rs`
- Modify: `crates/codex-plus-core/src/assets.rs`
- Modify: `crates/codex-plus-core/tests/cdp_bridge.rs`
- Modify: `assets/inject/renderer-inject.js`（Ad-List URL 与推荐/赞赏 Tab）

- [ ] **Step 1: Write/adjust failing tests**

在 `tests/ads.rs` 增加生产入口测试，同时保留 `fetch_ad_list_from_urls` 的本地 fixture 测试：

```rust
#[tokio::test]
async fn ads_disabled_returns_empty_list_without_network() {
    assert!(!codex_plus_core::branding::ADS_ENABLED);
    let payload = codex_plus_core::ads::fetch_ad_list().await.unwrap();
    assert_eq!(payload["ads"].as_array().unwrap().len(), 0);
}
```

删除上游默认 URL 与 builtin sponsors 必须存在的断言；新增断言保证生产 URL 列表为空、`normalize_ad_payload` 不追加 builtin。不要让配置开关改变纯 normalize 函数的语义。

在 `cdp_bridge.rs` 将 sponsor images “存在”断言改成“变量和 data URI 均不存在”，并断言脚本不含 Ad-List URL、推荐 Tab 和赞赏 fallback URL。

- [ ] **Step 2: Run tests to see failures**

Run: `cargo test -p codex-plus-core --test ads`
Expected: 旧断言失败 / 新断言失败

- [ ] **Step 3: Implement**

在 `ads.rs`：

```rust
pub async fn fetch_ad_list() -> anyhow::Result<Value> {
    if !crate::branding::ADS_ENABLED {
        return Ok(json!({ "version": 1, "ads": [] }));
    }
    fetch_ad_list_from_urls(&DEFAULT_AD_LIST_URLS).await
}
```

将生产 `DEFAULT_AD_LIST_URLS` 改为空数组；`normalize_ad_payload` 保留现有纯转换逻辑但删除 `append_builtin_sponsors` 调用；本地 fixture 测试直接传 URL 给 `fetch_ad_list_from_urls`。

`assets.rs`：停止注入 `__CODEX_PLUS_SPONSOR_IMAGES__`，避免空对象仍留下兼容入口；赞赏图片文件可暂留历史资源，但不得被生产二进制 include 或下发。

`renderer-inject.js`：删除/短路广告 URL 常量与「推荐内容」「赞赏」Tab 渲染入口（保留其它增强功能）。

- [ ] **Step 4: Re-run ads tests**

Run: `cargo test -p codex-plus-core --test ads`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/codex-plus-core/src/ads.rs crates/codex-plus-core/tests/ads.rs crates/codex-plus-core/src/assets.rs crates/codex-plus-core/tests/cdp_bridge.rs assets/inject/renderer-inject.js
git commit -m "feat: disable ad list, builtins, sponsor images, and inject recommendation UI"
```

---

### Task 2b: 禁用上游 ScriptMarket，保留本地脚本

**Files:**
- Modify: `crates/codex-plus-core/src/script_market.rs`
- Modify: `apps/codex-plus-manager/src/App.tsx`（`SCRIPT_MARKET_REPOSITORY_URL`）
- Test: 相关 `bridge_routes` / market 测试若依赖上游 URL 则改为本地 fixture

- [ ] **Step 1: Short-circuit market fetch**

将 `DEFAULT_MARKET_INDEX_URL` 置空，并在 `fetch_market_manifest` 发请求前对空 URL 返回 `ScriptMarketManifest { version: 1, updated_at: None, scripts: Vec::new() }`；非空 URL 的现有 fixture 能力保持不变。

- [ ] **Step 2: UI**

隐藏市场刷新和上游仓库外链，但保留本地脚本的启用、禁用、导入和删除能力。后端空 manifest 仅作为旧调用兼容，不向空 URL 发请求。

- [ ] **Step 3: Test + commit**

```bash
cargo test -p codex-plus-core --test bridge_routes
git add crates/codex-plus-core/src/script_market.rs apps/codex-plus-manager/src/App.tsx
git commit -m "feat: disable upstream ScriptMarket in Chimera edition"
```

---

### Task 3: 管理端去 JOJO / 推荐页 + About + 窗口名

**Files:**
- Modify: `apps/codex-plus-manager/src/App.tsx`
- Modify: `apps/codex-plus-manager/src/styles.css`
- Modify: `apps/codex-plus-manager/src/i18n-en.ts`
- Modify: `tools/i18n-keys.json`
- Modify: `apps/codex-plus-manager/index.html`
- Modify: `apps/codex-plus-manager/src-tauri/tauri.conf.json`
- Modify: `apps/codex-plus-manager/src-tauri/src/lib.rs`
- Modify: `assets/inject/stepwise-inject.js`

- [x] **Step 1: Locate and remove JOJO overview panel**

删除 `App.tsx` 中 `jojocode-overview` 相关 JSX（约概览页硬编码横幅），以及打开 `https://jojocode.com/` 的按钮处理。

- [x] **Step 2: Remove recommendations route/nav item**

删除侧栏「推荐内容」导航项、路由分支、`load_ads` 页面。

- [x] **Step 3: Replace About links + window branding**

将 GitHub 链接改为本项目公开仓库；上游归属放在 README/About 的独立致谢中。删除不属于 Chimera 的 Discord/Telegram/赞赏入口；品牌标题改为 `Chimera Codex`。

`tauri.conf.json`：`productName` / `app.windows[0].title` 改为 Chimera 名称。
`src-tauri/src/lib.rs`：凡硬编码 `Codex++ Manager` / 窗口 title 一并替换。
`index.html` 与 `stepwise-inject.js`：替换用户可见旧名称；内部日志前缀可保留以兼容诊断。

- [x] **Step 4: Remove CSS + unused i18n keys**

删除 `styles.css` 中 `.jojocode-overview*` 规则块；删除已无入口的「推荐内容」相关 i18n 键并同步 `tools/i18n-keys.json`。

- [x] **Step 5: Typecheck**

Run: `cd apps/codex-plus-manager && npm ci && npm run vite:build && npx tsc --noEmit`
Expected: exit 0

- [x] **Step 6: Commit**

```bash
git add apps/codex-plus-manager/src/App.tsx apps/codex-plus-manager/src/styles.css apps/codex-plus-manager/src/i18n-en.ts tools/i18n-keys.json apps/codex-plus-manager/index.html apps/codex-plus-manager/src-tauri/tauri.conf.json apps/codex-plus-manager/src-tauri/src/lib.rs assets/inject/stepwise-inject.js
git commit -m "feat: remove promotional UI and apply Chimera display branding"
```

---

### Task 4: ChimeraHub 预设与首次启动 Key-first 配置

**Files:**
- Modify: `apps/codex-plus-manager/src/presets.ts`
- Modify: `crates/codex-plus-core/src/settings.rs`
- Modify: `apps/codex-plus-manager/src-tauri/src/commands.rs`
- Modify: `apps/codex-plus-manager/src/App.tsx`
- Test: core settings tests / manager command tests

- [x] **Step 1: Add a complete ChimeraHub preset**

```ts
{
  id: "chimerahub",
  name: "ChimeraHub",
  websiteUrl: "https://api.chimerahub.org/",
  apiKeyUrl: "https://api.chimerahub.org/",
  category: "aggregator",
  baseUrl: "https://api.chimerahub.org/v1",
  protocol: "responses",
  model: "gpt-5.5",
  modelList: ["gpt-5.5"],
},
```

Codex Base URL 必须带 `/v1`。默认模型属于 `brand/product.toml`，首个 Release 前必须用真实 ChimeraHub Key 验证；`websiteUrl` / `apiKeyUrl` 也必须验证为真实无邀请码页面。若服务端默认变化，只改品牌配置并重新生成。

- [x] **Step 2: Remove promo presets**

删除 `jojocode`、`jojocode-max`；删除 `siliconflow` 等预设中的邀请码，若有无推广的官方入口则替换为官方入口。

- [x] **Step 3: Add first-run settings constructor**

不要直接改变所有 serde 缺省值。新增只在 `settings.json` 不存在时使用的 `chimera_first_run_settings()`：

- active profile id: `chimerahub`
- relay mode: `PureApi`
- protocol: `Responses`
- base URL: `https://api.chimerahub.org/v1`
- model/model list: 品牌配置中的已验证默认模型
- API Key: empty
- `relayProfilesEnabled=true`

读取已有 settings 时继续走兼容反序列化，保留 legacy `relayBaseUrl` / `relayApiKey` / profiles / active id。仅“文件不存在”可判定为全新安装，空 Key 不能作为覆盖已有配置的理由。

- [x] **Step 4: Add atomic save-and-enable command**

提供 Key-first 后端命令：验证 Key 非空，保存 profile，生成现有格式的 `configContents` / `authContents`，设置 active id 并应用。任一步失败都返回错误且不记录 Key；写 live Codex 文件继续使用现有备份/原子写路径。

UI 对全新默认状态显示 API Key 输入和单一主操作“保存并启用”；空 Key 时禁用。已有用户继续看到原 profile，不自动切换。

- [x] **Step 5: Tests**

- settings 文件不存在 → Chimera profile 被选中但未写 live config。
- 现有 settings → 内容逐项保持，不注入 Chimera active id。
- Key 为空 → 命令失败且 `config.toml` / `auth.json` 不变。
- 有效 Key → Base URL 带 `/v1`，profile 被保存并启用。
- 日志和返回消息不含 Key。

- [x] **Step 6: Commit**

```bash
git add apps/codex-plus-manager/src/presets.ts crates/codex-plus-core/src/settings.rs apps/codex-plus-manager/src-tauri/src/commands.rs apps/codex-plus-manager/src/App.tsx
git commit -m "feat: add ChimeraHub preset with key-first first-run enablement"
```

---

### Task 5: 版本协议、公开更新源与资产完整性

**Files:**
- Modify: root `Cargo.toml`, `Cargo.lock`, `crates/codex-plus-core/Cargo.toml`
- Modify: workspace package versions, `apps/codex-plus-manager/package.json`, package lock, `src-tauri/tauri.conf.json`
- Modify: `crates/codex-plus-core/src/update.rs`
- Modify: `crates/codex-plus-core/tests/updater.rs`

- [x] **Step 1: Write failing SemVer tests**

新增以下更新顺序断言：

```rust
assert!(is_newer_version("1.2.34-chimera.2", "1.2.34-chimera.1")?);
assert!(is_newer_version("1.2.35-chimera.1", "1.2.34-chimera.9")?);
assert!(!is_newer_version("1.2.34-chimera.1", "1.2.34-chimera.1")?);
assert!(!is_newer_version("1.2.34-chimera.1", "1.2.34-chimera.2")?);
```

同时拒绝非法版本、其它发行通道和缺少 `chimera` 标识的 manifest，避免公开上游资产被误装。

- [x] **Step 2: Use standard SemVer and synchronize build versions**

引入 `semver` crate，替换当前遇到 `-` 即截断的解析逻辑。版本格式固定为 `X.Y.Z-chimera.N`。Cargo workspace version 是构建源；`scripts/generate-branding.ps1 -Check` 同时验证前端 package 与 Tauri version 一致，并验证 `macos_build_number` 为正整数且高于上一发布 tag 中的值。

原版覆盖升级通过用户手工运行 Chimera 安装包完成；安装后的第一个 Chimera 版本已经嵌入完整后缀，后续自动更新均按标准 SemVer 比较。

- [x] **Step 3: Point updater to public branding URL**

`DEFAULT_REPOSITORY` / `DEFAULT_LATEST_JSON_URL` 读取生成的 branding 值。请求不携带 GitHub token，测试通过本地 HTTP fixture 验证 JSON；真实公开 Release 冒烟验证匿名 200。

- [x] **Step 4: Strict platform and arch selection**

只接受以下形状：

- `ChimeraCodex-<version>-windows-x64-setup.exe`
- `ChimeraCodex-<version>-macos-x64.dmg`
- `ChimeraCodex-<version>-macos-arm64.dmg`

保留并更新现有 `is_macos_native_arch_asset` 排序，禁止退化为“文件名包含 chimera 即接受”。同时拒绝 zip、source archive、其它架构和近似前缀。

- [x] **Step 5: Extend latest.json and verify downloads**

`ReleaseAsset` 增加 `sha256` 与 `size`。`perform_update` 下载到临时文件，检查实际大小与 SHA-256，验证成功后原子重命名并启动；失败时删除该单个临时文件并返回错误，绝不启动。

测试覆盖正确哈希、错误哈希、大小不符、路径穿越、缺字段、下载中断和当前平台双架构选择。

- [x] **Step 6: Test + commit**

Run: `cargo test -p codex-plus-core --test updater`
Expected: PASS；`update.rs` 无上游 Release URL，`.1 → .2` 更新成立

```bash
git add Cargo.toml Cargo.lock crates/codex-plus-core/Cargo.toml crates/codex-plus-core/src/update.rs crates/codex-plus-core/tests/updater.rs apps/codex-plus-manager/package.json apps/codex-plus-manager/package-lock.json apps/codex-plus-manager/src-tauri/tauri.conf.json
git commit -m "feat: add SemVer Chimera updates with asset verification"
```

---

### Task 6: 双平台安装品牌与原版覆盖升级

**Files:**
- Modify: `crates/codex-plus-core/src/install/mod.rs`
- Modify: `crates/codex-plus-core/src/install/windows.rs`
- Modify: `crates/codex-plus-core/src/install/macos.rs`
- Modify: `scripts/installer/windows/CodexPlusPlus.nsi`
- Modify: `scripts/installer/macos/package-dmg.sh`
- Modify: `.github/workflows/release-assets.yml`（macOS bundle 验证路径）
- Modify: `crates/codex-plus-core/tests/installers.rs`（若断言旧名）

- [x] **Step 1: Wire display constants**

```rust
pub const SILENT_NAME: &str = crate::branding::DISPLAY_SILENT_NAME;
pub const MANAGER_NAME: &str = crate::branding::DISPLAY_MANAGER_NAME;
// 一期保持二进制名不变：
pub const SILENT_BINARY: &str = "codex-plus-plus";
pub const MANAGER_BINARY: &str = "codex-plus-plus-manager";
```

- [x] **Step 2: windows.rs 文案**

替换硬编码：新建的 `.lnk`、`DisplayName`、`Publisher=BigPizzaV3`、协议描述中的显示名。保留 legacy 名称常量用于检测和清理。
**不要**把 `default_install_root()`（桌面快捷方式根）误当成 NSIS `Programs` 目录。

- [x] **Step 3: NSIS**

- 改 `Name`、卸载 `DisplayName`、`Publisher`、`OutFile` → `ChimeraCodex-${VERSION}-windows-x64-setup.exe`
- **`InstallDir` 一期保持** `$LOCALAPPDATA\Programs\Codex++`（写死决策，降低迁移成本）
- 保留 `InstallDirRegKey` 和旧卸载 subkey，确保识别原版安装；只改变其中的用户可见值
- 安装前终止旧进程，删除旧、新桌面快捷方式与开始菜单目录，再只创建 Chimera 入口
- 卸载同时清理两套入口和历史乱码快捷方式；修复当前 NSIS 中已经出现的乱码字面量
- 升级失败不得提前删除现有二进制；使用临时 staging/可恢复顺序覆盖

- [x] **Step 4: macOS package-dmg.sh**

显示名与 `ChimeraCodex-*-macos-*.dmg` 产物名同步；Bundle ID 和 bundle executable 一期保留兼容值。`CFBundleShortVersionString` 写 `X.Y.Z`，`CFBundleVersion` 写每次 Release 严格递增的 `macos_build_number`，不能原样写 `X.Y.Z-chimera.N`。继续使用 ad-hoc codesign 以保证 bundle 结构有效，但明确没有 Developer ID 签名和 notarization。

新 app 路径为 `Chimera Codex.app` / `Chimera Codex 管理工具.app`。由于拖拽无法覆盖旧文件名，manager 首次启动检测同目录或 `/Applications` 下的 `Codex++.app` / `Codex++ 管理工具.app`，显示迁移提示与“打开 Applications”操作；不在后台擅自删除旧 App。README 提供退出旧进程、拖入新 App、删除旧 App、右键打开/Gatekeeper 放行的步骤。

同步修改 release workflow 的 bundle 验证路径，避免打包已改名而 CI 仍检查旧 App。

- [x] **Step 5: Test installers unit tests**

Run: `cargo test -p codex-plus-core --test installers`
Expected: PASS；测试覆盖原版 Windows 安装计划、旧快捷方式清理、新快捷方式创建、macOS legacy 检测与双架构 bundle 名

- [x] **Step 6: Commit**

```bash
git add crates/codex-plus-core/src/install scripts/installer crates/codex-plus-core/tests/installers.rs .github/workflows/release-assets.yml
git commit -m "feat: add Chimera installers with legacy upgrade handling"
```

---

### Task 7: README、品牌一致性与去推广门禁

**Files:**
- Modify: `README.md`, `README_EN.md`
- Create: `scripts/verify-no-upstream-ads.ps1`
- Create: `scripts/verify-allowlist.txt`（仅在确有历史 fixture 时）

- [x] **Step 1: Rewrite public documentation first**

去掉赞助商表格、JOJO/邀请码和赞赏图片，增加：

- ChimeraHub Key-first 配置和 `/v1` 说明
- 公开仓库、公开 Release 与更新机制
- 上游 CodexPlusPlus 与 cc-switch 的 MIT 归属，保留 LICENSE notice
- Windows 原版覆盖升级说明
- macOS x64/arm64、ad-hoc signing、未 notarize、右键打开和旧 App 清理步骤
- 不在文档示例中放真实 Key

- [x] **Step 2: Implement scanner**

脚本扫描生产源、root README、打包和 workflow。排除 `.git`、`target`、构建产物和历史设计文档；测试 fixture 若必须保留旧域名，使用逐行说明的窄 allowlist，禁止整个目录豁免。

禁止残留：
- `BigPizzaV3/Ad-List`
- `BigPizzaV3/CodexPlusPlusScriptMarket`
- `jojocode.com`
- `append_builtin_sponsors` 仍被调用
- `DEFAULT_LATEST_JSON_URL` / `update.rs` 含 `BigPizzaV3/CodexPlusPlus`
- `__CODEX_PLUS_SPONSOR_IMAGES__` 非空注入路径（按实现调整）
- `TBD` / `example owner` / `chimera-org/chimera-codex` 品牌占位
- 用户可见路径中的 `Codex++ Manager` / `Codex++ 管理工具`（legacy 迁移常量除外）
- Cargo/package/Tauri 版本不一致
- NSIS/DMG/workflow 资产前缀或品牌值与 `brand/product.toml` 不一致

允许残留：上游 remote URL、LICENSE/归属说明、兼容 provider/protocol/state id、legacy 安装清理常量、经 allowlist 标注的测试 fixture。

- [x] **Step 3: Run locally**

Run: `pwsh -File scripts/verify-no-upstream-ads.ps1`
Expected: exit 0。README 已在本 Task 前一步清理，因此不存在“扫描先落地但仓库必红”的过渡状态。

- [x] **Step 4: Commit**

```bash
git add README.md README_EN.md scripts/verify-no-upstream-ads.ps1 scripts/verify-allowlist.txt
git commit -m "docs: document Chimera distribution and add branding gates"
```

---

### Task 8: Build-first 公开 Release workflow

**Files:**
- Modify: `.github/workflows/release-assets.yml`
- Modify: `.github/workflows/pr-build.yml`（产物名一致）

- [x] **Step 1: Replace the release-created trigger**

`release-assets.yml` 使用 `push: main` + `workflow_dispatch`。入口读取 Cargo workspace version：若远端已存在 `v<version>` tag 则幂等退出；tag 不存在才继续。不要先创建 Release 再期待 `release: published` 触发第二个 workflow。

添加同版本 concurrency group，避免定时同步与手工发布重复。

- [x] **Step 2: Build all platforms before publishing**

Windows x64、macOS x64、macOS arm64 各自构建、验证并上传 Actions artifact。所有构建都使用 `npm ci` 和 lockfile。macOS bundle 检查使用 Chimera 新路径，并验证 plist、架构和 ad-hoc codesign；不声称 notarized。

任一矩阵失败都不得创建公开 tag/Release。

- [x] **Step 3: Rename artifacts strictly**

将 `CodexPlusPlus-$version-windows-x64.zip/setup.exe` 改为 `ChimeraCodex-$version-windows-x64.*`；macOS DMG 同理。

- [x] **Step 4: Final publish job**

最终 job 下载三平台 Actions artifacts，计算每个发布资产的 SHA-256 和 size，生成 `latest.json`。然后在当前 `main` SHA 创建 `v<version>` draft Release，上传安装资产与 manifest；上传全部成功后才 publish，且 `prerelease=false`。

失败时保留 draft 供排查，不改变 `/releases/latest/download/latest.json`。成功后用匿名 HTTP 请求验证 `latest.json` 和至少一个资产可下载。

- [x] **Step 5: PR build parity**

`pr-build.yml` 使用同一构建脚本、命名与 bundle 验证，但只上传 Actions artifact，不创建 tag/Release。加入 `generate-branding -Check`、扫描、Rust 测试、前端 build/typecheck。

- [x] **Step 6: Commit**

```bash
git add .github/workflows/release-assets.yml .github/workflows/pr-build.yml
git commit -m "ci: build and publish verified Chimera releases"
```

---

### Task 9: 上游同步自动化

**Files:**
- Create: `scripts/sync-upstream.ps1`
- Create: `.github/workflows/sync-upstream.yml`

- [x] **Step 1: sync script behavior**

```powershell
# scripts/sync-upstream.ps1（行为规格）
# 1. 校验 clean worktree、非 shallow、origin/upstream 精确 URL、无进行中的 git 操作
# 2. 查询上游最新正式 Release；忽略 draft/prerelease
# 3. 若对应 Chimera tag/同步 PR 已存在则幂等退出
# 4. 显式 fetch 该 tag，在隔离工作树创建 sync/upstream-vX.Y.Z
# 5. merge 上游 tag；冲突时记录文件、git merge --abort、exit 2
# 6. 设置 X.Y.Z-chimera.1，运行 branding generation/check 和所有门禁
# 7. 成功后由 workflow 推分支并创建/更新 PR；脚本本身不直接改 main 或创建 Release
```

`-DryRun` 只使用 `git remote get-url`、`git status`、`git ls-remote`/GitHub API 输出计划，不 fetch、不创建 branch/worktree、不写文件。退出码：0 无变更或可同步，2 merge 冲突，3 门禁失败，4 配置/权限错误。

- [x] **Step 2: workflow**

`sync-upstream.yml`：
- `schedule: cron` 每天 2 次 + `workflow_dispatch`
- `fetch-depth: 0`，使用 concurrency 防止并发同步
- 使用最小 `contents: write`、`pull-requests: write`、`issues: write`；自动触发 PR checks 时使用 GitHub App 或 `CHIMERA_AUTOMATION_TOKEN`
- 推送 `sync/upstream-vX.Y.Z` 并创建 PR，正文记录 upstream tag/SHA、Chimera version 和验证结果
- required checks 全绿后开启 auto-merge；merge 后由 Task 8 的 `push: main` workflow 发布
- 冲突或测试失败时创建/更新标题 `[sync:vX.Y.Z] upstream sync blocked` 的单一 Issue，正文不含 secret；恢复后关闭对应 Issue
- 不在 sync workflow 中调用 `gh release create`

版本策略：上游新版本第一次同步为 `X.Y.Z-chimera.1`；同一上游版本的 Chimera 修复由人工或发布准备 PR 递增 `N`。每个发布准备 PR 同时把 `macos_build_number` 加一。

- [x] **Step 3: Dry-run on a branch**

Run: `pwsh -File scripts/sync-upstream.ps1 -DryRun`
Expected: 打印目标 upstream tag、分支、版本与 gates；`git status`、refs 和文件哈希前后不变

- [x] **Step 4: Commit**

```bash
git add scripts/sync-upstream.ps1 .github/workflows/sync-upstream.yml
git commit -m "ci: add upstream sync watcher with conflict issues"
```

---

### Task 10: 全量验证、升级矩阵与首次公开 Release

**Files:**
- Modify: `docs/superpowers/todos/2026-07-10-chimera-private-fork-todo.md`（勾选）

- [ ] **Step 1: Full automated verification**

```powershell
pwsh -File scripts/verify-no-upstream-ads.ps1
cargo fmt --check
cargo test
cd apps/codex-plus-manager
npm ci
npm run vite:build
npx tsc --noEmit
```

另运行 `scripts/generate-branding.ps1 -Check`，并确认工作树无生成漂移。Expected: all green。

- [ ] **Step 2: Windows clean-install and upgrade smoke**

1. 全新安装：默认显示 ChimeraHub Key-first；空 Key 不写配置；填 Key 后保存并启用。
2. 从上游 v1.2.34 覆盖：安装目录不变、已有 settings/profile 不变、旧快捷方式被清理、只剩 Chimera 入口。
3. 卸载：新旧快捷方式、注册表入口和程序文件均清理，用户配置仅按既有卸载选项处理。
4. `.1 → .2` mock update：匿名读取 manifest、选中 x64 setup、哈希验证、启动安装器。

- [ ] **Step 3: macOS x64/arm64 smoke**

1. 两种架构 DMG 均含两个 Chimera App 和 `/Applications` 链接，plist 架构正确。
2. 新安装按 README 右键打开/放行后可启动；Release 明示未 notarized。
3. 机器存在旧 `Codex++.app` 时显示迁移提示，不静默删除；完成文档步骤后只保留新 App。
4. updater 只选择本机架构 DMG并验证 SHA-256。

- [ ] **Step 4: Sync failure drills**

- 对已处理 tag 重跑：无新 PR/Issue/Release。
- 人为制造 merge 冲突：merge 被 abort，Issue 去重，main 不变。
- 人为制造 test/hash failure：不创建公开 Release，latest 仍指向上一版本。
- 正常同步：PR checks → auto-merge → build-first publish → 匿名下载成功。

- [ ] **Step 5: First public release and rollback**

首次先创建 draft，检查 README、版本、资产名、SHA-256、latest.json 和 macOS 未可信签名/未公证提示后再 publish。保留上一版本 Release；若发现严重问题，标记当前 Release 非 latest/撤回 manifest，并发布递增的修复版本，不复用或移动已发布 tag。

- [ ] **Step 6: Update checklist and commit**

```bash
git add docs/superpowers/todos/2026-07-10-chimera-private-fork-todo.md
git commit -m "docs: record Chimera release verification"
```

---

## Self-review (writing-plans)

1. **Spec coverage:** S1–S12 均有 Task；新增公开下载、SemVer、哈希、首次配置、升级迁移和双平台验收。
2. **Placeholders:** 真实公开仓库是唯一开工外部决策；生成门禁拒绝假 owner。
3. **Consistency:** `brand/product.toml` 是机器可读真相源；生成/校验替代手工双写。
4. **Release safety:** 同步只产 PR，发布 build-first；不依赖 `GITHUB_TOKEN` 产生的 Release 事件递归触发。
5. **Upgrade safety:** 新安装自动选中 ChimeraHub；已有用户不被覆盖；Windows 与 macOS 分别定义迁移路径。

## Execution Handoff

文档阶段完成后停止，不创建仓库、不改产品代码。等待用户明确授权开工，再从 Task 0 开始。
