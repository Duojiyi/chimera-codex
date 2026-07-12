# Task 11 Step 11.1 Audit B

## Round 1

结论：**FAIL**。

独立按 diff、边界与回归面检查后发现：

- installer 契约实测 `9 passed / 11 failed`，覆盖 Windows 名称/快捷方式、macOS app/DMG、release workflow 和 README。
- manager workflow 契约实测 `33 passed / 2 failed`，仍期望旧产物前缀和 Tauri 产品名。
- Windows 安装器尚未清理历史 `Chimera Codex` 快捷方式和开始菜单项，覆盖升级会遗留重复入口；该风险可在 Step 11.4 关闭，但必须保留为明确风险。

已确认 branding 测试 `1/1`、生成器 `-Check` 和 scoped `git diff --check` 通过。以上阻断关闭前不得勾选 Step 11.1。

## Round 2

结论：**FAIL**。

- `apps/codex-plus-manager/src/App.tsx` 的英文分支将 `windowTitle` 硬编码为 `Chimera++ Manager`，绕过 `DISPLAY_MANAGER_NAME`。
- 生成器与当时的 manager 契约均未捕获该漂移，属于 Step 11.1 窗口触点和测试伪绿。

其余 branding/installer/updater `58/58`、生成器、i18n 和 scoped diff 门通过。

## Round 3

结论：**FAIL**。

- updater 与 plugin marketplace 的出站 User-Agent 仍为旧 `ChimeraCodex/<version>`，不属于允许保留的兼容技术 ID。
- branding 测试未覆盖相关生产文件，且只拒绝带空格旧名。
- Tauri capability 源文件和生成 schema 的描述仍为 `Chimera Codex Manager`。

窗口、托盘、Tauri、installer 和发行产物名称已确认正确。

## Round 4

结论：**FAIL**。

同样独立发现 5 个 `CodexPlusPlus` 出站 User-Agent 触点，并确认现有 branding 测试与生产扫描器对此伪绿。其他品牌触点和兼容技术 ID 边界通过。

## Round 5

结论：**PASS**。

内建 User-Agent、capability、真相源、窗口/UI、installer/workflow、i18n 与 scanner 均通过；未发现新的阻断。provider sync 外部 marker 由审计 A 随后独立发现，因此本轮总体仍按 FAIL 处理。

## Round 6 Final

结论：**PASS**。

未发现 Step 11.1 范围内阻断或测试伪绿。生成器 `-Check` 通过；主线程已有完整定向回归证据。注入、About 和单桌面入口按计划留给后续 Step。
