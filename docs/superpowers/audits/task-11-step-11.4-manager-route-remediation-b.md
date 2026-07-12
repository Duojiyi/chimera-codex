# Task 11 Step 11.4 manager 文件 IPC 独立审计 B

## 结论

**PASS**

当前最新修复已关闭上一轮四项阻断：批量扫描按条目隔离错误，不再丢弃已 claim 请求；pending drain 在队列真正为空前不标记 frontend ready，并增加 not-ready 握手；ACK timeout 保留 sender 仍拥有的 `.json`；single-instance guard 的非冲突错误改为 fatal fail-closed。新增的 `.tmp/.processing/.ack` 过期清理、稳定 FIFO 和 claim 后 ACK 顺序也形成闭合。当前生产路径、边界条件和 targeted tests 中未发现阻止 Step 11.4 通过的缺口。

本审计从当前最新工作树独立完成，未读取或引用 manager 审计 A，也未修改实现。

## 原阻断复核

### B1. partial claim 与坏条目隔离：已关闭

- `apps/codex-plus-manager/src-tauri/src/lib.rs:299-354` 对 `read_dir` 单条错误、metadata/read 失败和非特定 rename 失败均逐条 `continue`，不会再让整个批次返回 Err。
- 有效请求在 `:339-346` 原子 rename 为 `.processing` 后加入返回队列；后续坏条目不会丢弃此前已 claim 的请求。
- `:349-353` 在全部可用请求收集后按 `created_at_ms`、再按 `id` 稳定排序，消除 `read_dir` 非确定顺序。
- `broken_request_entry_does_not_discard_an_already_claimed_request` 使用合法 JSON 与目录型 `z-broken.json` 混合，验证合法请求仍以 `.processing` 返回。

**判定：关闭。** 单个 stale、invalid、oversize、目录或暂时不可读条目不再毒化同批合法请求。

### B2. frontend-ready drain 与 listener 生命周期：已关闭

- `StartRouteState::next_pending_or_mark_ready` 在 `lib.rs:43-49` 每次只 pop 队首；只有观察到队列为空时才设置 `frontend_ready=true`。
- `manager_frontend_ready` 在 `:426-436` 循环逐项取出和 emit。排空期间 frontend 仍为 not-ready，新到 route 会进入 VecDeque 尾部，因此不会插到旧 pending 前面。
- React 在 `apps/codex-plus-manager/src/App.tsx:1825-1846` 先完成 event listener 注册，再 invoke ready；cleanup 先 invoke `manager_frontend_not_ready`，并在其 settle 后才 unlisten，避免 listener 已移除而 backend 仍认为 ready 的正常卸载窗口。
- `manager_frontend_not_ready` 在 `lib.rs:439-443` 将状态恢复为 not-ready；重新挂载后新 listener 的 ready 会继续 drain 期间积累的 route。
- 状态测试覆盖 drain 中插入新 route、空队列后 ready、not-ready 后重新排队。

**判定：关闭。** 正常 React mount/unmount/reload 生命周期内，pending 和新请求保持 FIFO，backend 不会在 listener 移除后继续把 route 当作已交付。

### B3. 2 秒 ACK timeout 与请求所有权：已关闭

- sender 在 `lib.rs:268-296` 以 `.tmp` 写入、flush、sync 后 rename 为 `.json`；发布失败会 best-effort 清理 `.tmp`。
- `wait_for_start_route_ack` 在 `:237-255` 超时只报告 `pending` 或 `claimed`，不再删除 `.json`。
- 因此主实例已持 guard 但 Tauri setup/listener 冷启动超过 2 秒时，请求仍留在目录中；listener 启动后可以继续 claim 和 dispatch。
- `start_route_ack_timeout_keeps_request_for_slow_primary_startup` 明确验证 timeout 返回后 request 仍存在。

**判定：关闭。** ACK timeout 不再越权销毁 receiver 尚未读取的新鲜请求。

### B4. guard 通用错误 fail-open：已关闭

- `classify_single_instance_guard_result` 在 `lib.rs:575-589` 只把 `AddrInUse`/`WouldBlock` 解释为 existing instance；权限、状态目录、文件锁和其他 I/O 错误原样返回。
- `run` 在 `:93-113` 对 fatal guard error 记录不含敏感内容的 error kind 后退出，不再绑定不可发现的随机端口。
- 正常固定端口被无关服务占用时，core 仍持 `locks/loopback-port-<port>.lock` 并使用 file-lock fallback；第二实例通过同一 lock 被识别，不依赖被占用端口。
- manager 静态契约明确禁止 `TcpListener::bind(("127.0.0.1", 0))`；单测验证 PermissionDenied 为 fatal。

**判定：关闭。** 无法建立共享 guard 时 fail-closed，不会启动多个随机端口 manager。

## ACK、claim 与 artifact 边界

- 正常顺序为 `.json` 原子发布 -> `.processing` 原子 claim -> backend dispatch/queue -> 删除 processing -> 写独立 `<id>.ack`。ACK 不会在 claim 或 dispatch 之前出现。
- 删除 processing 失败不会重复消费，因为扫描器只读取 `.json`；遗留 processing 由 artifact cleanup 回收。
- `cleanup_stale_start_route_artifacts` 在 `lib.rs:357-385` 只处理 `.tmp/.processing/.ack` 普通文件，保留 `.json` ownership；listener 启动时执行一次，之后每 30 秒执行。
- cleanup 以 30 秒 age 保护活跃 sender/receiver 文件；测试验证 orphan artifact 被删除而 pending JSON 保留。
- 每个请求使用独立 id 和 ACK 路径，多请求不会共享确认槽；sender 成功观察 ACK 后 best-effort 删除对应文件。

## 输入与导航复核

- request 最大 256 bytes；可读的 oversize、非法 JSON、stem/id 不匹配、30 秒外 stale 和非白名单 route 被拒绝并尝试删除。
- route 白名单仅允许 `update`、`maintenance`、`relay`、`show`；IPC 和新增日志不携带 API Key、provider URL、auth/config 内容或任意命令文本。
- 有效 route 会 show、unminimize、focus 主窗口；`update` 打开 About 并检查更新，`maintenance`/`relay` 通过既有 `navigate` 路径刷新对应数据；首次启动 query route 映射一致。
- 固定端口可用、无关可连接服务占用、不可连接占用和第二实例 lock conflict 均有 core guard 测试。

## 测试结果

- `cargo test -p codex-plus-manager --lib --locked`：**PASS**，54 passed、0 failed。覆盖 FIFO、partial claim、timeout ownership、artifact cleanup、not-ready 与 fatal guard。
- `cargo test -p codex-plus-manager --test windows_subsystem --locked`：**PASS**，40 passed、0 failed。
- `cargo test -p codex-plus-core --lib ports::tests --locked`：**PASS**，10 passed、0 failed。
- `npm run check`：**PASS**。
- `npm run vite:build`：**PASS**，1608 modules transformed。
- `cargo fmt --all -- --check`：**PASS**。
- 未运行 workspace 全套测试，也未执行真实双进程 Tauri/WebView fault-injection。

## 剩余风险

- receiver 在 claim 后、dispatch 前发生进程级崩溃时，`.processing` 会在 30 秒后作为 orphan 清理而不是重放；这是异常终止下的 at-most-once 取舍。sender 不会收到 ACK，并可通过再次启动重试，不构成本步骤正常运行阻断。
- 硬 WebView/进程崩溃可能跳过 React cleanup，not-ready 握手不能提供进程崩溃级保证；正常 effect cleanup/reload 路径已闭合。
- `app.emit`、processing 删除和 ACK 写入仍为 best-effort，真实 ACL、磁盘故障和 renderer crash 的端到端注入未覆盖。失败不会导致 `.json` 重复扫描，但可能表现为 sender timeout 或 orphan cleanup。
- request id 为 `pid-current_time_ms`，正常多进程足够区分，但不是密码学随机票据；同用户本地进程可制造已知 route，风险限于窗口导航和可用性。

## 最终判定

文件 IPC 已具备稳定 FIFO、逐条错误隔离、原子发布与 claim、明确 ownership timeout、ready/not-ready 生命周期、fatal guard 和 orphan artifact 回收。考虑上述异常终止残余后，未发现新的阻断性回归，Step 11.4 manager 文件 IPC 独立审计 B 判定为 **PASS**。
