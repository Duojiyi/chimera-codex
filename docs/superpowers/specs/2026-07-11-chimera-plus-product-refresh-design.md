# Chimera++ 客户发行版产品刷新规格

> 日期：2026-07-11
> 状态：已确认需求，待按 TDD 实施与双盲审计
> 基线：2026-07-10 本地实现已通过 747 项 workspace 测试和聚合双盲审计，但尚未完整推送或发行
> 覆盖关系：本规格覆盖 2026-07-10 规格中的产品显示名、About、可见 GitHub 链接、图标和更新交互；其余安全、同步、安装兼容与去推广决策继续有效

## 1. 产品目标

Chimera++ 是面向 ChimeraHub 客户的 Codex 外部增强启动器与管理工具。产品首要体验是：安装后输入 ChimeraHub Key 即可使用，不要求客户理解供应商、Base URL、GitHub Release 或手工下载安装包。

## 2. 已确认决策

1. 用户可见主名称统一为 `Chimera++`；需要区分管理入口时使用 `Chimera++ 管理工具`。
2. 全新安装默认内置 `https://api.chimerahub.org/v1`、Responses 协议和经发行前验证的默认模型；用户只填写 Key 并执行“保存并启用”。
3. 升级用户的现有 profile、Key、active id 和 Codex 配置不得被默认值覆盖。
4. 管理端和注入菜单彻底移除推荐、赞助、交流群、About、项目主页、Issues 和其他可点击 GitHub 入口。
5. GitHub 仅作为后台发行基础设施：客户端可匿名读取固定仓库的 `latest.json` 和版本化资产 URL，但不得把仓库链接、Release 概念或下载按钮暴露给普通用户。
6. 更新采用启动自动检查与自动下载。`latest.json` 声明 `minimum_supported_version`；只有当前版本低于该值时阻断继续使用。普通更新自动执行，但网络不可达或安装失败时不得让仍受支持版本无法启动。
7. Windows 在完整性校验后以静默参数启动安装器，安装器必须保留失败回滚；但未做 Authenticode 签名时仍可能出现 SmartScreen/系统确认，真正零干预更新需要后续购买代码签名证书。macOS 未 Developer ID 签名且未公证，只能自动下载并引导用户确认，不能承诺真正无感安装。
8. 所有发行图标必须是 Chimera++ 原创资产。Microsoft Store ChatGPT 图标只可作为尺寸、留白和小图标清晰度参考，不复制、不派生、不提交，以避免商标、版权和官方关联混淆。
9. 继续保留兼容性技术 ID：二进制名、`CodexPlusPlus` provider id、`codexplusplus://`、bundle id、状态目录、旧安装定位键和 Windows 安装目录。它们不得出现在普通用户可见品牌文案中。
10. 上游只跟踪正式 Release tag，经同步 PR 和 required checks 合入；不自动发行 upstream `main` 未发布提交。
11. Windows 桌面只保留一个使用原创图标的 `Chimera++` 快捷方式。仅全新且没有任何可用官方登录或 active relay 时，该入口打开 Key-first；已有可用登录/中转时直接启动 Codex。管理工具只保留在开始菜单/应用程序目录和托盘入口。覆盖安装必须清理旧 `Codex++`、旧 Chimera 和旧管理工具桌面快捷方式。

## 3. 更新状态机

```text
启动
  -> 拉取并验证 latest.json
     -> 无更新：进入应用
     -> 普通更新：后台下载、校验、启动平台安装流程；失败时记录中性错误并允许受支持版本继续
     -> 强制更新：显示不可关闭的更新状态，下载和校验成功后进入平台安装流程
     -> 清单不可达：若本地版本仍在支持范围或无法取得可信最低版本，允许离线继续并记录诊断
```

安全约束：

- `minimum_supported_version` 和清单 `version` 都必须是合法的 `X.Y.Z-chimera.N` 发行通道版本，minimum 不得高于 latest；两者允许跨上游版本，例如 latest `1.2.35-chimera.1`、minimum `1.2.34-chimera.3`。
- 最近一次成功验证的最低支持版本必须原子写入本地更新状态，并只允许单调升高。新清单声明更低 floor 时保留已缓存的较高值，防止服务端误配置或回滚。
- 清单不可达但可信缓存已判定当前版本过旧时继续阻断；没有缓存时允许离线继续。缓存损坏时隔离损坏文件、记录诊断并允许继续，明确该机制面向普通更新一致性而非抵抗有本机文件权限的恶意用户。
- 清单、平台/架构资产、大小、SHA-256、版本化 GitHub Release URL 和启动前文件身份均继续严格验证。
- 下载、哈希、文件身份或启动失败时不得删除或覆盖当前可运行版本。
- 客户 UI 使用“正在更新 / 更新失败 / 重试”等产品语言，不出现 GitHub、Release、asset、SHA 等实现术语。

## 4. UI 与信息架构

- 左侧导航不含推荐或 About。
- 首次配置首屏只突出 ChimeraHub Key 与“保存并启用”。Base URL 默认固定；高级用户仍可在供应商编辑页创建自定义 profile。
- 日志和诊断从 About 移到“安装与维护”的故障排查区域，避免删除支持能力。
- 不保留“检查更新”“下载并运行安装包”按钮；更新状态由全局启动流程展示。
- 注入菜单不显示关于、提出问题、项目主页或 GitHub。
- 桌面只有一个 `Chimera++` 客户入口；不得同时出现静默启动器和管理工具两个桌面快捷方式。

单入口启动判定顺序固定为：

1. 已缓存或在线确认的强制更新优先，进入更新界面，不先进入 Key-first 或启动 Codex。
2. settings 无法严格读取时打开管理工具的恢复界面，不覆盖或重建原文件。
3. 已有可用的任意 active relay（不限 ChimeraHub）或有效官方 ChatGPT 登录态时直接启动 Codex。
4. profile 只有 Key 但 live 配置尚未成功应用时打开管理工具继续配置。
5. 仅全新/未配置状态进入 ChimeraHub Key-first。

判定实现必须是可独立单测的纯函数，覆盖非 ChimeraHub profile、官方登录、仅 `auth.json` 有 Key、损坏 settings、未应用 profile 和强制更新优先级。

## 5. 品牌与图标

- `brand/product.toml` 仍是显示名、发行仓库、更新 URL、默认中转和资产前缀的机器可读真相源。
- 资产前缀改为 URL 友好的 `ChimeraPlusPlus`，不在文件名中使用 `+`。
- 原创图标必须提供主 SVG，并导出仓库现有打包链实际使用的 PNG/ICO；macOS 打包若当前从 PNG 生成 `.icns`，继续由同一主资产生成。
- 16、32、48、192、512 和 1024 像素下检查边缘、透明背景、对比度与辨识度。
- 不复用上游 Codex++、OpenAI、ChatGPT 或其他第三方产品图标。
- `brand/icon/PROVENANCE.md` 记录生成工具/模型、完整设计 brief、输入参考边界、创建日期、人工选择结论和发行许可声明；不得把 ChatGPT 图标文件、像素或矢量路径复制进工作区。
- 图标审计同时检查视觉相似性和资源清单，不能只用“哈希不同”证明原创。所有 Tauri、NSIS、文档、内置 assets 和 macOS `.icns` 输入必须可追溯到同一主 SVG。

## 6. README 边界

README 首页改为客户安装与使用说明，先讲下载、安装、输入 Key、启动和升级。开发、架构和上游同步移到后半部分。README 可以为开源归属与许可证保留必要的源码仓库和上游链接；“软件内无 GitHub 链接”不等于删除公开仓库中的许可证归属。

许可证在首发前必须单独核验并补齐仓库许可文件，不能继续用未经证实的 MIT badge 或文案。若同步包含上游改为 AGPL-3.0 后的提交，必须按对应许可证履约。

## 7. 验收标准

| ID | 标准 |
|---|---|
| R1 | 生产 UI、注入菜单、窗口、托盘、安装器和客户 README 使用 Chimera++ 品牌 |
| R2 | 管理端无 About route/nav/screen，注入菜单无 About/Issues/GitHub |
| R3 | 推荐、赞助、交流群和推广网络入口继续为零 |
| R4 | 全新用户只输入 Key 即可启用 `https://api.chimerahub.org/v1`，升级用户配置不变 |
| R5 | 启动自动更新；普通更新失败不锁死受支持版本；最低支持版本触发阻断状态 |
| R6 | Windows 安装流程自动化；macOS 明确需要用户确认且不声称已公证 |
| R7 | 所有发行图标均由原创主资产生成，不含第三方图标哈希或像素副本 |
| R8 | 上游正式 Release 同步 PR、Windows x64 与 macOS x64/arm64 三个构建目标、本项目 Release 自动化在线通过 |
| R9 | 每个可观察行为都有 Red、Green、针对性回归和独立 A/B 审计记录 |
| R10 | Windows 全新/覆盖安装后桌面只有一个 `Chimera++` 快捷方式；仅全新且无可用官方登录/active relay 时进入 Key-first，否则直接启动 |
| R11 | 已验证的最低支持版本单调缓存；断网不能绕过已知强制更新，损坏缓存不永久锁死应用 |

## 8. 当前真实进度

- 上游最新正式 Release：`v1.2.34`，tag commit `c1360294d43fce06116428555cbcf812902aced5`。
- 本地 HEAD 与 `origin/main` 都包含该 tag；上游 `main` 另有 5 个未发布提交，因此不进入自动发行基线。
- PR #1 的 `f23ab82` 已由 run `29201732498` 完成综合门、Windows、macOS x64 与 macOS arm64 全绿；治理补救仍在本地等待最终审计与推送。
- `Duojiyi/chimera-codex` 当前仍没有 Release；首发必须等待 PR 合并和 `public-release` 审批。
- 同步 workflow 改用 job-scoped `GITHUB_TOKEN`，恢复并复核可信 main workflow 树后显式 dispatch PR checks，不再需要长期 `CHIMERA_AUTOMATION_TOKEN`；该 workflow 尚待 PR 合入 `main` 后上线。

## 9. 首发与远端写入门

首次公开 Release 不得由当前脏工作树或未经用户确认的 push 直接触发。先把首发 publish job 绑定到受保护的 `public-release` environment，由仓库管理员确认一次；首发匿名下载、Windows/macOS 资产和更新冒烟通过后，后续正式版本才切换为全自动发布。自动同步仍受 PR checks 约束，不使用降低 main 保护规则的方式换取自动合入。
