# Task 11 Step 11.1 Evidence

## Scope

将品牌真相源、安装器、打包产物、窗口标题和 workflow 契约统一为：

- 静默入口：`Chimera++`
- 管理入口：`Chimera++ 管理工具`
- 发行产物前缀：`ChimeraPlusPlus`

兼容性技术 ID、二进制名、安装目录和旧版本清理标识保持稳定。

## Initial Red

- `cargo test -p codex-plus-core --test branding --locked`
  - 修改期望后：`0 passed / 1 failed`
  - 实际值仍为 `Chimera Codex`。

## Partial Green

- 修改 `brand/product.toml` 并运行品牌生成器。
- `cargo test -p codex-plus-core --test branding --locked`
  - `1 passed / 0 failed`
- `pwsh -NoProfile -File scripts/generate-branding.ps1 -Check`
  - PASS
- scoped `git diff --check`
  - PASS

## Audit-Revealed Red

- `cargo test -p codex-plus-core --test installers --locked`
  - `9 passed / 11 failed`
- `cargo test -p codex-plus-manager --test windows_subsystem --locked`
  - `33 passed / 2 failed`

因此 partial Green 不足以关闭 Step 11.1，必须继续完成安装器、workflow 和运行时窗口触点。

## Contract Red

- 更新 installer/workflow/Tauri 契约为新品牌后：
  - installers：`19 passed / 1 failed`
  - manager workflow contracts：`34 passed / 1 failed`
  - 失败分别定位到硬编码入口导出和硬编码初始窗口标题。
- 扩展生产管理 UI 契约，拒绝 `Chimera Codex`：
  - targeted manager test：`0 passed / 1 failed`
  - 首个失败触点为 `index.html`。
- 扩展生产 Rust/mobile relay 品牌契约：
  - targeted branding test：`0 passed / 1 failed`
  - 首个失败触点为 `launcher.rs`。

## Final Green

- `cargo test -p codex-plus-core --test branding --test installers --locked`
  - branding：`2 passed / 0 failed`
  - installers：`20 passed / 0 failed`
- `cargo test -p codex-plus-manager --test windows_subsystem --locked`
  - `35 passed / 0 failed`
- `npm run check`
  - PASS
- `npm run vite:build`
  - PASS，`1607 modules transformed`
- `pwsh -NoProfile -File scripts/generate-branding.ps1 -Check`
  - PASS
- scoped `git diff --check`
  - PASS

生产 Rust、管理 UI、HTML 标题和 mobile relay 已无旧显示名。注入脚本的旧显示名保留为 Task 12 Step 12.2 的独立 TDD Red，不计作本 Step 已关闭行为。

## Gap Closure Red/Green

- i18n 键清单契约 Red：manager targeted test `0 passed / 1 failed`，检测到 `tools/i18n-keys.json` 仍含旧显示名。
- 公开品牌契约 Red：branding targeted test `0 passed / 1 failed`，检测到 Issue 模板仍含旧显示名。
- i18n 生成器精确门 Red：`575` 个引用键仅生成 `2` 个普通键、`0` 个模板键。
- 修正 codemod 对已包装调用、嵌套调用和 dry-run 候选键的收集后：
  - 普通键：`575 referenced / 575 translated / 575 manifest`
  - 模板键：`40 referenced / 40 translated / 40 manifest`
- 公开 Issue 模板和 CONTRIBUTING 已改用 Chimera++；targeted branding/manager tests 均 `1 passed / 0 failed`。

## Stable Regression

- core branding：`2 passed / 0 failed`
- core installers：`20 passed / 0 failed`
- manager workflow contracts：`35 passed / 0 failed`
- `cargo fmt --all -- --check`：PASS
- full `git diff --check`：PASS
- 非测试生产路径旧显示名扫描：无命中；Task 12 注入资产按边界排除。

## Scanner Closure

- `verify-no-upstream-ads.ps1` 初次品牌刷新后 Red：`15 finding(s)`，其中 1 项为扫描器仍要求旧 `ChimeraCodex` 产物前缀，14 项为 provider import 测试夹具 allowlist 行号漂移。
- 扫描器产物前缀改为 `ChimeraPlusPlus` 后，该项消失。
- 逐项确认 7 个 `jojocode.com` 命中均位于 `#[cfg(test)] mod tests`，更新精确行号与完整行 allowlist。
- `pwsh -NoProfile -File scripts/verify-no-upstream-ads.ps1`：OK。
- `pwsh -NoProfile -File scripts/test-verify-allowlist.ps1`：PASS，包含 allowlist 与 docs/assets fail-closed fixtures。

## Audit B Window-Title Closure

- 审计 B 发现英文分支硬编码 `windowTitle: "Chimera++ Manager"`，绕过品牌真相源。
- 新增契约要求 `windowTitle: DISPLAY_MANAGER_NAME` 并拒绝硬编码品牌：Red `0 passed / 1 failed`。
- 最小实现改用已生成的 `DISPLAY_MANAGER_NAME`：targeted Green `1 passed / 0 failed`。
- manager workflow contracts：`35 passed / 0 failed`。
- `npm run check`、`npm run vite:build`：PASS，build `1607 modules transformed`。

## Audit B Client-Token Closure

- 扩展 branding 契约覆盖 updater/plugin marketplace 并拒绝 `ChimeraCodex`：Red `0 passed / 1 failed`。
- 扩展 manager 契约覆盖 capability 源文件与生成 schema：Red `0 passed / 1 failed`。
- 三处出站 User-Agent 改由 `ARTIFACT_PREFIX` 派生；capability 描述统一为 `Chimera++ Manager`。
- 两组 targeted Green 均 `1 passed / 0 failed`。
- 扩大回归：branding `2/2`、installers `20/20`、updater `36/36`、manager `35/35`，共 `93/93`。
- 去推广扫描、rustfmt、full `git diff --check`：PASS。
- 非测试生产源码扫描 `ChimeraCodex|Chimera Codex Manager`：无命中。

## Comprehensive User-Agent Closure

- branding 契约扩展到 HTTP client、model catalog、relay config、protocol proxy、ads：Red `0 passed / 1 failed`。
- 生产扫描器新增 `CodexPlusPlus/` 与精确默认 UA 规则：Red `5 finding(s)`。
- 新增 `branded_user_agent(component)`，统一输出 `ChimeraPlusPlus/<component-or-version>`；所有内建请求路径共用该 helper。
- branding `2/2`、生产扫描器 `OK`。
- 受影响扩大回归：core unit `152/152`、ads `7/7`、model catalog `7/7`、protocol proxy `45/45`、relay config `95/95`、updater `36/36`，共 `342/342`。
- allowlist fail-closed、自有图片 fail-closed、rustfmt、full `git diff --check`：PASS。

## Provider-Sync Marker Closure

- data 集成契约要求新 `managedBy`：Red `0 passed / 1 failed`。
- branding 静态契约只禁止旧 marker 的写入形式，允许只读兼容：Red `0 passed / 1 failed`。
- 生产扫描器新增旧 marker writer 规则：Red `1 finding(s)`。
- 新 marker 从 `DISPLAY_SILENT_NAME` 派生为 `Chimera++ provider sync`；prune 同时识别新值和只读 legacy 值。
- targeted data/branding/scanner 三门 Green。
- data provider_sync 全量 `17/17`，其中历史旧 marker prune/回滚场景通过；branding `2/2`。
- allowlist fail-closed、rustfmt、full `git diff --check`：PASS。
