# Task 15 Step 15.4 Release Gate Remediation 独立审计 B

> 日期：2026-07-13
> 范围：Release gate 前端预构建的最终 diff、job/step 边界、执行顺序、mutation 有效性与相关回归面
> 独立性：仅按当前 diff、workflow、测试实现和本审计复跑结果检查；未参考审计 A
> 结论：**PASS AFTER REMEDIATION**

## Finding

### B1（已关闭）：目标 step 曾可被后续 decoy step 假阳性满足

初始测试从 `Build frontend` 名称一直切到 `Rust tests` 名称，因此目标 step 即使执行错误的 `npm run build`，只要两者之间另一个 step 含正确 working directory 与 `npm run vite:build`，helper 仍返回 true。独立 mutation 的旧结果为：

```text
baseline=True
wrong-target-plus-unrelated-correct-step=True
```

补救后，helper 只检查 `Build frontend` 到下一个 active `- name:` 边界。测试同时构造并拒绝以下 mutations：

- 注释 `Build frontend` step 名；
- 删除整个目标 step；
- 将目标命令改为完整 `npm run build`；
- 将目标 step 移到 `Rust tests` 后；
- 目标 step 使用错误命令、后续 named decoy step 使用正确 Vite 命令。

五个 mutation fixture 均经独立检查确认相对原 workflow 实际发生变化；focused test 全部通过。B1 已关闭。

## Diff 与边界复核

- `.github/workflows/release-assets.yml` 仅在 `gates` job 新增一个 `Build frontend` step。
- step 位于 `Install frontend dependencies` / `npm ci` 和 TypeScript check 之后，位于 Rust formatting 与 `Rust tests` / `cargo test --workspace --locked` 之前。
- 命令精确为 `npm run vite:build`，working directory 精确为 `apps/codex-plus-manager`；未误用会触发 launcher/Tauri release 打包的完整 `npm run build`。
- 测试先将文本限制在 `gates` job，再定位目标 step；Windows/macOS artifact jobs 中已有的同名 frontend build 不能满足 release gate 合约。
- 当前 `package.json` 将 `vite:build` 定义为 `vite build`，与 Tauri `frontendDist: ../dist` 的生产资源路径一致。

## 验证

| 验证 | 结果 |
|---|---|
| `cargo test -p codex-plus-core --test installers release_gate_builds_frontend_before_rust_tests --locked -- --exact --nocapture` | PASS，1/1 |
| `cargo test -p codex-plus-core --test installers --locked` | PASS，29/29 |
| comment / missing / wrong-command / after-Rust / decoy mutations | PASS，五个 fixture 均有效且被拒绝 |
| `npm run vite:build`（manager 目录） | PASS，1608 modules，生成 `dist/index.html` 与 production assets |
| `cargo test -p codex-plus-manager --locked` | PASS，lib 54/54、workflow/packaging 43/43、doc tests 0 failure |
| scoped `git diff --check` | PASS；仅现有 LF/CRLF checkout 提示，无 whitespace error |

## 结论

当前 remediation 修复了 Release run `29204323955` 在 Rust tests 编译 Tauri 时缺少 `../dist` 的门禁顺序问题。实现保持在 release `gates` 的最小范围，使用正确的轻量 Vite build；最终测试能拒绝所要求的注释、缺失、错误命令、错误顺序和 decoy 假阳性。未发现剩余阻断项。
