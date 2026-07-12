# Task 11 Step 11.2 Evidence

## Scope

将中英文 README 重写为 ChimeraHub 客户路径：Key-only、固定 `/v1`、自动更新、Windows 单桌面入口说明；客户区不使用 GitHub/About/手动更新路径。必要开源归属和仓库链接只保留在开发与归属区。未经 LICENSE/NOTICE 验证前不声明 MIT。

## Red

- 新增 `customer_readmes_are_key_first_and_do_not_use_github_or_manual_update_paths`。
- targeted test：`0 passed / 1 failed`。
- 首个失败：Chinese customer section 仍包含 GitHub URL。

## Green

- targeted README contract：`1 passed / 0 failed`。
- installer/document contracts：`21 passed / 0 failed`。
- `generate-branding.ps1 -Check`：PASS。
- `verify-no-upstream-ads.ps1`：OK。
- allowlist 与 docs/assets fail-closed 自测：PASS。
- scoped `git diff --check`：PASS。

README 行数变化使两条必要的旧 macOS App 迁移 allowlist 从 72 行移动到 65 行；只更新了精确行号，完整行和理由不变。

## Audit B Gap Closure

- README 增加中英文发行准备警示：当前快照未完成单桌面/自动强更验收，不得作为客户正式发行版交付；对应内容明确为目标行为。
- 测试改用 `split_once` 强制客户区与开发/归属区同时存在。
- 开发/归属区正向要求 upstream、cc-switch 和本项目公开仓库链接。
- 客户区使用大小写归一的 GitHub/About 检查，并扩展中英文手动更新等价词。
- 加强后的 Red 首先命中 English customer section 的 `about` 普通措辞；改写无歧义文案并补警示后 Green `1/1`。
- 完整 installer/document contracts `21/21`，生成器、生产扫描、allowlist fail-closed 和 scoped diff 均 PASS。
- 最终 README 迁移说明位于中英文第 67 行，allowlist 仅更新精确行号。

## Audit B Gate Hardening

- 归属标题要求全篇恰好出现一次，并继续使用 `split_once`。
- 全文按独立 ASCII token、大小写不敏感拒绝 `MIT`。
- 客户区拒绝 `Chimera Codex` / `ChimeraCodex`；每个 `Codex++` 行必须带迁移语境。
- 手动更新使用同一行内“人工动作 + 获取动作 + 更新/安装包对象”组合规则。
- 负例 fixture 覆盖 `mit License`、`MIT 许可证`、`自行下载最新版安装包`、`Manually fetch the latest package`；合法 `Manually delete the legacy app` 不误报。
- 初次整篇组合扫描因跨段词汇误报而 Red；修正为逐行组合后 Green。
- 最终 installer/document contracts `21/21`、rustfmt、生产扫描和 full diff 均 PASS。
