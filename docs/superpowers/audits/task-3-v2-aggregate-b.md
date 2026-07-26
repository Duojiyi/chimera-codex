# Task 3 (T43) Audit B — Diff / 边界 / 失败恢复

**Date:** 2026-07-26
**Scope:** Step 3.1–3.6
**Commits:** `4681735`, `5d4c7d2`, `181beef`
**Auditor:** Independent B（只看 diff/边界/失败恢复，不参考 Audit A 的结论）

方法：不重复检查 Spec 条款是否"实现"，只检查已实现代码在异常输入、并发、缺失依赖、崩溃恢复路径下的行为。

## providerForm.ts — 边界

- 空字符串、纯空格 Key 均被拒绝且给出 recovery 提示（`providerForm.ts:26-34`，测试 `providerForm.test.ts:16-24`）✓
- URL 解析异常（`new URL()` throw）被 `try/catch` 捕获，转成结构化错误而不是让异常冒泡到调用方（`providerForm.ts:73-83`）✓
- Origin-only URL（无 path）被标记为 `severity: "warning"` 而不是 `"error"`——但**这只是数据结构上的区分**，调用方 `handleAdd`（`providers/index.tsx:51-59`）用 `.filter(e => e.severity === "error")` 过滤掉警告，这意味着 origin-only 的警告在 UI 上**从未被展示给用户**，用户看不到"将会请求 /v1"这条提示就直接保存了。这是一个静默降级：数据模型区分了 warning/error,消费端把 warning 直接丢弃。
- 未测试：URL 中包含非 ASCII/超长字符串（Spec 15.1 提到"长 URL/名称"边界场景）、重复添加同一 URL（Spec 2.1 要求"重复 URL"检测，但该检测在 `chimera-provider` DB 层，Provider-first UI 层完全没有前置校验，用户可以对同一个供应商添加两次同名条目）。

## providerState.ts — 并发与不变式

- `switchProvider` 对不存在的 id 静默忽略、返回原状态（`providerState.ts:56-61`；测试 `providerState.test.ts:94-99`）——纯函数层面安全，无异常抛出。✓
- `deleteProvider` 删除当前激活项后正确回退到 Official 模式（`providerState.ts:73-85`；测试 `providerState.test.ts:112-119`）✓
- **这些状态转换全部是同步、单线程、纯内存的 React state**，不涉及真正的并发（没有两个窗口/进程同时操作同一份 state 的场景）。Spec 7.4 要求的"跨进程锁 + CAS + journal"发生在 `chimera-provider::transaction` crate（Task 2 交付，5 个测试全部通过：happy path、外部改写检测、journal-before-rename、锁竞争、hash 检测字节变化——见 `crates/chimera-provider/tests/step2_5_transaction.rs`），**但 Task 3 的前端从未调用这条事务路径**。

## 关键发现：`switch_provider` 命令的失败路径是无提示静默失败

`commands.rs:152-158`：

```rust
pub fn switch_provider(_provider_id: Option<String>) -> Result<(), String> {
    Err(
        "Provider switching is not enabled in this build. Task 6 connects the config transaction."
            .to_string(),
    )
}
```

前端调用点 `providers/index.tsx:74-79`：

```ts
async function handleSwitch(id: string | null) {
  setBusy(true);
  try { await invoke("switch_provider", { id }); setState(s => switchProvider(s, id)); }
  catch (err) { console.error("switch failed", err); }
  finally { setBusy(false); }
}
```

这是一个**失败恢复反模式**：命令必定失败 → `catch` 块只打日志 → `setState` 因为在 `try` 块内、命令失败即抛异常，从未执行 → UI 上 `busy` 短暂置真后又置假，用户看到的效果是"点击后按钮闪烁一下，什么都没变"。没有 `role="alert"` 提示、没有 toast、没有 disabled 状态锁定。对比 `HomeFeature.handleLaunch`（`home/index.tsx:39-49`）对同类失败路径处理得更好——它把错误存进 `launchError` 并通过 `role="alert" aria-live="polite"` 展示（`home/index.tsx:130-134`）。Providers 页面没有对等的错误展示通道，这是两个页面之间处理失败路径不一致的具体证据。

## AddProviderForm — 输入边界

- URL 输入框类型为 `type="url"`（`providers/index.tsx:221`），依赖浏览器原生校验作为第一道防线，但 `onSubmit` 用 `e.preventDefault()`（`providers/index.tsx:198`）绕过了原生 `required`/`type=url` 的表单级拦截,实际校验完全落在 `validateCustomProviderInput`——这条路径已用 7 个测试覆盖，边界合理。
- API Key 输入框 `autoComplete="off"`（`providers/index.tsx:232`）+ `type="password"`，防止浏览器/OS 自动填充历史记录里意外带出 Key，这是一个正确的最小防护,但没有测试验证（DOM 属性，`node --test` 的纯 TS 测试无法覆盖，需要 Playwright，而 Playwright 套件不存在于 CI）。
- 表单没有对 Key 长度设上限——一个异常超长字符串（比如粘贴了整份文件内容）会被原样存进 `keyInput` state 并传给 `invoke`。当前 `test_provider`/`switch_provider` 都是 stub，所以这条路径目前无害，但一旦 Task 6 接上真实事务，缺少长度上限意味着没有防护。记录为待关注项，不阻断当前判定（因为下游还未接线）。

## Tauri 命令层 DTO 边界（`command_dto.rs`）

- 已用 5 个测试锁定 wire contract：camelCase 序列化、`providerName: null` 在 Official 模式下正确出现、**`ProviderDto` 不携带任何 secret 字段**（测试显式断言 JSON 中不出现 `apiKey`/`api_key`/`secretRef`/`secret`，`command_dto.rs:47-72`）。这是本次审计里最扎实的边界测试：它是针对一个真实发生过的回归写的（commit message 记录："A DTO test asserting no secret crosses the IPC boundary caught that ProviderDto carried secret_ref to the frontend"）,说明测试确实抓住过一次真实缺陷,而不是装饰性断言。✓
- `AppState::initialise()`（`state.rs:96-113`）在 DB 打不开或 runtime 目录建不出来时返回 `Err(String)`,`lib.rs:16-21` 收到错误后 `eprintln!` + `process::exit(1)`——这是"fail loud, not degrade silently"的正确选择,符合 Spec 对启动失败处理的期望,但**没有测试验证这条路径**（需要模拟一个不可写目录，`AppState::initialise` 没有对应的失败注入测试）。记录为覆盖缺口。
- `get_system_status`/`list_providers` 对 DB mutex 中毒（`lock().map_err`）有防御,返回用户可读的"Restart Chimera++"提示而非 panic（`commands.rs:37-38`,`67-68`）✓,但同样没有并发/中毒场景的测试验证——纯粹是代码审查层面的确认。

## V16（design-tokens）自测——已独立验证

不依赖 commit message 的说法,亲自执行了自测:
1. 正常运行 `node scripts/verify-design-tokens.mjs` → PASS（10 项全绿）。
2. 向 `home/index.tsx` 注入一行 `const _leak = "#123456";` → 重新运行 → **FAIL**，报错精确指出该文件和字面量。
3. 撤销注入,确认 `git diff` 回到干净状态 → 重新运行 → PASS。

这证明 V16 的自测机制不是文档声称,而是可复现的真实行为,这是本次审计里少数"文档所述与实测完全一致"的部分。

## Step 3.5 a11y 门禁——CI 层面的真实失败,不是"未覆盖"

用 `node scripts/verify-v2.mjs --only=V8` 复现了完整的 V8 门禁链条:

```
V8 Frontend check + test + a11y + build   ✗ FAIL (61350ms)
> npm run check   (tsc --noEmit)          — 通过
> npm test        (25 TS tests)           — 全部通过
> npm run test:a11y  → Cannot find module 'scripts/test-a11y.mjs'  — 中断
```

这意味着如果这条门禁真的接入 CI（`.github/workflows/v2-build.yml` 目前的 frontend job 并**没有**调用 `npm run test:a11y`，只跑了 `check`/`test`/`vite:build`,见 workflow 第 153-163 行——这是 CI 配置与 `package.json` 脚本清单之间的又一处不一致：`verify-v2.mjs` 编排器会调用 `test:a11y` 并失败，但真正跑在 GitHub Actions 上的 `v2-build.yml` 绕开了这一步,两者对"V8 是否包含 a11y"的定义不一致）。这是一个需要在 Task 3 收尾前统一的具体缺陷,不是可以忽略的边角案例。

## Failure-recovery 总评

| 场景 | 处理 | 证据 |
|---|---|---|
| 添加供应商时格式错误 | 展示，不崩溃 | `providers/index.tsx:235-239` |
| 添加供应商时探测失败 | **不存在**（因为探测本身不存在） | Audit A Step 3.1/3.2 |
| 切换供应商失败 | 静默吞掉，无 UI 反馈 | `providers/index.tsx:74-79` |
| 启动 Codex 失败 | 展示 + `aria-live` | `home/index.tsx:130-134` |
| DB 打开失败（启动时） | fail-fast + 退出 | `lib.rs:16-21` |
| DB mutex 中毒 | 返回可读错误，不 panic | `commands.rs:37-38` |
| a11y 门禁脚本缺失 | CI 编排器里会报错；真正跑的 workflow 未接入 | 已复现验证 |

## 结论

**FAIL。**

已实现的纯函数模块（`providerForm`/`providerState`）边界处理扎实，DTO 层的 no-secret-leak 测试证明测试确实在防止真实回归。但用户可感知的失败路径存在明显不一致：Providers 页面的切换失败是静默的，而 Home 页面的启动失败有恰当提示——这种不一致本身就是一个应被记录的缺陷，而不只是"功能未完成"。更严重的是 Step 3.5 对应的 `test:a11y` 脚本在当前提交状态下指向不存在的文件，导致独立编排器 `verify-v2.mjs` 的 V8 门禁实测失败；即使实际 CI workflow 目前绕开了这一步，这仍是一处配置层面的真实缺陷，且反映出 CI workflow 与本地编排脚本对"V8 包含什么"的定义已经不同步。按 Plan 规则 4，A/B 均需 PASS 才可勾选；本审计给出 FAIL。
