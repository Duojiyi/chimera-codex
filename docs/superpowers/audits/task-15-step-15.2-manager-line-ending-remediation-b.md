# Task 15 Step 15.2 Manager Diagnostics Line-Ending Remediation 独立审计 B

> 日期：2026-07-13
> 范围：manager diagnostics 静态合约的 LF/CRLF/CR-only 归一化、test module 边界、production append 计数和三项 raw-error 事件范围
> 独立性：只按当前 diff、源码结构和独立 normalization mutation 复核。
> 结论：**PASS AFTER REMEDIATION**

## Finding

### B1（已关闭）：CR-only regression 未被测试覆盖

当前测试把 `raw_source` 规范为 LF，并额外构造 `synthetic_crlf` 验证 CRLF round-trip；没有构造 CR-only 源码，也没有证明删除 `.replace('\r', "\n")` 会触发 Red。

独立 mutation 将测试中的 CR-only normalization 去掉，仅保留 CRLF → LF。当前工作树和 synthetic CRLF 仍满足原断言：

```text
remove-cr-normalization-current-and-crlf-still-pass=True
```

但同一 broken normalizer 处理 CR-only 源码时找不到边界：

```text
remove-cr-normalization-cr-marker-found=False
```

补救后，测试使用同一个 `normalize_line_endings` closure 处理真实 source、synthetic CRLF 和 synthetic CR；两种 synthetic source 都必须规范回同一个 LF source。原 mutation 独立复跑结果：

```text
current-crlf-roundtrip=True
current-cr-roundtrip=True
remove-cr-normalization-crlf-still-passes=True
remove-cr-normalization-cr-detected=True
```

因此删除 `.replace('\r', "\n")` 时 synthetic CR 明确触发 Red，CR-only 回归已被约束。

## 当前实现边界

独立把 `commands.rs` 规范成三种换行后，再使用当前两步 normalization 运行完整合约：

```text
LF marker=True append=2 events-clean=True
CRLF marker=True append=2 events-clean=True
CR marker=True append=2 events-clean=True
```

- 精确 marker `\n#[cfg(test)]\nmod tests` 命中第 3866-3867 行的测试模块，不会误切第 3414 行单独修饰 `log_manager_event` test stub 的 `#[cfg(test)]`。
- production 截断后 `append_diagnostic_log` 恰好两处：公开 `write_diagnostic_event` 和 `#[cfg(not(test))] log_manager_event`；测试模块中的断言字符串不参与计数。
- 三个事件在 production 中均先命中实际调用点，截取到对应 `);`，范围内没有 `error.to_string()`。
- 测试模块自身再次包含三个事件名和 append 字符串，但位于 marker 之后，当前 production 切分能排除这些自引用污染。

## 验证

| 验证 | 结果 |
|---|---|
| `cargo test -p codex-plus-manager commands::tests::manager_diagnostics_do_not_submit_raw_errors_or_write_logs_in_unit_tests --lib --locked -- --exact --nocapture` | PASS，1/1 |
| LF / CRLF / CR-only 当前实现独立分析 | PASS，均 marker / append=2 / events-clean |
| remove CR-only normalization mutation | PASS：synthetic CR 正确触发 Red |
| test module 切分边界 | PASS，命中唯一 `#[cfg(test)] mod tests` |
| production append 与事件污染复核 | PASS，2 个 append、3 个事件范围干净 |
| `cargo fmt --all -- --check` | PASS |
| `git diff --check -- apps/codex-plus-manager/src-tauri/src/commands.rs` | PASS；仅现有 LF/CRLF 转换警告 |

## Gate

生产静态合约在 LF、CRLF、CR-only 三种输入下均成立，test module 切分、append 计数和 raw-error 事件范围没有发现实现缺陷；同一 normalization closure 现在覆盖 CRLF 与 CR synthetic source，移除 CR-only normalization 会触发 Red。**本独立审计 B 最终结论为 PASS AFTER REMEDIATION。**
