# Task 15 Step 15.2 Installer Fixture Remediation 独立审计 B

> 日期：2026-07-13
> 范围：`windows_legal_files_share_the_binary_transaction_and_uninstall_mutex` 的 NSIS mutation fixture、LF/CRLF 边界及后续 uninstall section 回归面
> 独立性：只按当前 diff、NSIS 实际顺序和独立内存 mutation 复核。
> 结论：**PASS**

## Diff 结论

旧 fixture 使用带 `\n` 的双行整段替换，只能匹配 LF；工作树中的 NSIS 为 CRLF 时 mutation 不发生，`assert_ne!` 先失败，无法证明 helper 的负向行为。

补救改为：

```rust
nsi.replacen(
    "  IfErrors uninstall_failed",
    "  ; IfErrors uninstall_failed",
    1,
)
```

该 token 不携带换行，因此 LF/CRLF 均可匹配。`replacen(..., 1)` 将变异限制为首次出现，避免批量注释后续卸载路径。

## 边界复核

- NSIS 中 `!macro UninstallFile PATH SLOT` 从第 98 行开始，目标 `IfErrors uninstall_failed` 首次出现于第 102 行，macro 在第 104 行结束。
- `Section "Uninstall"` 从第 667 行开始，后续同名 active jump 位于第 714、717、720、723、726 行。
- 因此首次匹配明确属于 `UninstallFile` macro，不属于后续 uninstall section。
- helper 只提取该 macro 到首个 `!macroend`，过滤空行与 `;` 注释，并验证 exists → clear → delete → fail 的严格顺序；目标行被注释后 helper 必然返回 false。

## 独立 Mutation

对同一 NSIS 分别规范为 LF 和 CRLF 后执行等价 mutation：

```text
LF baseline-helper=True
LF first-match-in-macro=True
LF mutation-helper=False
LF mutation-count=1
LF uninstall-section-unchanged=True
LF later-active-jumps-source=5
LF later-active-jumps-mutated=5

CRLF baseline-helper=True
CRLF first-match-in-macro=True
CRLF mutation-helper=False
CRLF mutation-count=1
CRLF uninstall-section-unchanged=True
CRLF later-active-jumps-source=5
CRLF later-active-jumps-mutated=5
```

比较 uninstall section 时分别从原字符串和变异字符串各自的 `Section "Uninstall"` 起点截取，避免 mutation 增加 `; ` 后产生的索引位移误报。两种换行下，后续 section 内容逐字符相同。

## 验证

| 验证 | 结果 |
|---|---|
| `cargo test -p codex-plus-core --test installers windows_legal_files_share_the_binary_transaction_and_uninstall_mutex --locked -- --exact --nocapture` | PASS，1/1 |
| LF 独立 mutation | PASS，helper Green → Red |
| CRLF 独立 mutation | PASS，helper Green → Red |
| 后续 uninstall section 完整性 | PASS，内容不变且 5 个 active jump 保留 |
| `cargo fmt --all -- --check` | PASS |
| `git diff --check -- crates/codex-plus-core/tests/installers.rs scripts/installer/windows/CodexPlusPlus.nsi` | PASS；仅现有 LF/CRLF 转换警告 |

## Gate

fixture 已不依赖宿主换行形式，首次匹配与 helper 作用域一致，mutation 真实触发 Red，且没有误改后续 uninstall section。**本独立审计 B 结论为 PASS，无新增阻断。**
