# Task 11 Step 11.4 Manager Route Remediation - Audit A

## 结论

**PASS**

当前实现已闭合 manager 文件 IPC 的可观察主路径：单实例锁冲突后转发 request，文件原子发布，receiver 局部隔离坏条目、原子 claim 为 `.processing` 并稳定排序；frontend-ready 排空期间保持 not-ready，React cleanup 先标记 not-ready 再卸载 listener；timeout 保留 request ownership，fatal guard 错误 fail-closed，陈旧 IPC artifact 定期清理。未发现阻断项。

## 审计范围

- 仅按需求、测试与可观察行为审计 manager 文件 IPC 第二版。
- 未读取或引用任何 manager 审计 B。
- 未修改实现代码。

## 核验结果

### A1. 文件入口与 pending 队列保持确定顺序

- `apps/codex-plus-manager/src-tauri/src/lib.rs:349` 在目录枚举后按 `created_at_ms` 排序，相同时间戳再按 `id` 确定性排序，不依赖 `read_dir` 顺序。
- `apps/codex-plus-manager/src-tauri/src/lib.rs:27-55` 使用 `VecDeque<String>`；pre-ready route 通过 `push_back` 全部保留，`next_pending_or_mark_ready` 逐个 FIFO pop，只有队列为空时才设置 ready。
- 状态测试在排空中途插入新 route，验证排空期间仍为 not-ready，新 route 会继续入队而不会越过旧 route。
- 文件顺序测试真实创建文件名顺序与创建时间顺序相反的两个 request，并验证返回 `relay, maintenance`。

### A2. 原子 claim、ACK 与请求消费顺序成立

- `apps/codex-plus-manager/src-tauri/src/lib.rs:339-346` 在验证合法 `.json` 后先原子 rename 为 `.processing`；只有 claim 成功的请求才会进入排序和 dispatch。
- listener 先 dispatch，使 route 入 pending 或 emit，再尝试删除 `.processing` 并写 ACK；即使清理失败，该文件也不再匹配 `.json` 扫描条件，因此不会重复消费。
- sender 仅在 ACK 出现后成功返回；timeout 返回 `pending` 或 `claimed` 状态，但不删除 request，使慢启动 primary 后续仍可消费。
- dispatch 对主窗口执行 show、unminimize、set_focus。
- `apps/codex-plus-manager/src/App.tsx:1825-1846` 先完成 `listen("chimera-start-route")` 注册，promise 返回后才 invoke `manager_frontend_ready`；cleanup 先 invoke `manager_frontend_not_ready`，finally 中才 unlisten。
- `maintenance` / `relay` 进入 React `navigate`，`update` 进入 About 并检查更新；冷启动 query 参数由 `loadInitialRoute` 处理。

### A3. 坏条目、guard 失败与 artifact 清理不会扩大故障

- `take_start_route_requests` 对单个 `read_dir` entry、metadata、read 和 claim 错误均局部 continue；坏目录条目不会丢弃已经 claim 的合法 request。
- 单实例结果只把 `AddrInUse` / `WouldBlock` 解释为已有实例；权限等 fatal 错误返回 `Err`，`run` 记录后直接退出，不再绑定随机端口启动第二实例。
- 发布失败会清理 `.tmp`；启动时及每 30 秒清理超过阈值的 `.tmp`、`.processing`、`.ack`，明确保留 pending `.json`。

## 测试与伪绿检查

- 状态测试覆盖多个 pre-ready route、排空中途新 route、仅空队列才 ready、mark_not_ready 后重新排队。
- 文件测试覆盖原子 request、无残留 `.tmp`、单请求 claim、timeout 保留 `.json`、stale/非法删除、逆目录顺序排序、坏条目局部隔离及 artifact 清理保留 pending request。
- guard 分类测试覆盖 held、成功和 `PermissionDenied` fatal 三种分支。
- 精确 allowlist 只接受 `update`、`maintenance`、`relay`、`show`；因此非法 URL 与任意未列入 allowlist 的 32-byte route 均被拒绝。
- `windows_subsystem` 继续提供单实例、窗口激活、事件名与 React 接线的静态契约；完整 manager lib 与前端编译/构建为其提供回归保障。

## 已确认行为

- pending 真队列：`VecDeque<String>` 保存全部 pre-ready route；`next_pending_or_mark_ready` 在 drain 完成前保持 not-ready，完成后 route 才直接 emit。
- 单实例：第二实例遇到文件锁冲突后进入文件 IPC 转发。
- fallback lock：`crates/codex-plus-core/src/ports.rs:195-213` 在 guard 端口被无关服务占用时保留独占文件锁并允许 manager 启动；第二 manager 会因锁冲突被拒绝。
- 原子 request：同目录 `create_new` 临时文件、flush、`sync_all`、rename 后发布 `.json`。
- 原子 claim：合法 request 在返回 listener 前 rename 为 `.processing`；后续删除失败不会再次进入 `.json` 扫描。
- 请求过滤：拒绝超过 256 bytes 的文件、非法 JSON、文件名/id 不一致、超过 30 秒及非 allowlist route，并删除对应 request；任意未列入 allowlist 的 32-byte route 同样会被拒绝。
- ACK 与消费：receiver 先 dispatch，使 route 进入 pending 或 emit，再删除 claim 并写入 `.ack`；sender 最多等待 2 秒，成功时清理 ACK，timeout 保留 request。
- 生命周期：React listener 注册成功后才 ready；cleanup 先 not-ready，再 unlisten。
- 故障隔离：坏条目局部跳过；fatal guard fail-closed；陈旧 `.tmp/.processing/.ack` 被清理但 `.json` 保留给 listener。
- 窗口激活：dispatch 调用 show、unminimize、set_focus。
- 冷启动路由：首次 manager 通过 `showUpdate=1` 或 `startRoute=maintenance|relay` 初始化 React route。
- 正常运行路由：frontend ready 后通过 `chimera-start-route` 事件导航。

## 验证结果

- `cargo test -p codex-plus-manager --lib -- --nocapture`
  - PASS：54 passed，0 failed。
- `cargo test -p codex-plus-core resilient_guard_ --lib -- --nocapture`
  - PASS：4 passed。
- `cargo test -p codex-plus-manager --test windows_subsystem -- --nocapture`
  - PASS：40 passed，0 failed。
- `npm run check`
  - PASS：TypeScript `tsc --noEmit` 通过。
- `npm run vite:build`
  - PASS：exit code 0，1608 modules transformed，构建完成；仅出现 Node `module.register()` deprecation warning。
- `cargo fmt --check`
  - PASS。

## 残余风险

- 当前未运行真实双进程 Tauri GUI 自动化；窗口 show/unminimize/focus、事件交付和 ACK 的跨进程组合仍由代码审查、状态/文件行为测试与静态契约共同覆盖。
- `id` 作为同毫秒请求的 tie-breaker 提供确定性顺序，但不声明同毫秒内跨进程的真实墙钟先后；这不影响稳定消费和请求不丢失。
- 若 manager 在 claim 成 `.processing` 后、dispatch 前崩溃，该 request 不会重放，30 秒后会作为陈旧 artifact 清理；发送方超时且不会收到错误 ACK。该 at-most-once 崩溃窗口不影响本 Step 的正常运行与删除失败不重复消费要求。
