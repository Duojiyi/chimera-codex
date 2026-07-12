# Chimera++ 客户发行版刷新 TODO

> 日期：2026-07-11
> 状态：T30-T33 已完成；T34-T35 进行中

## 决策门

- [x] N1. 用户可见名称改为 `Chimera++`，管理入口可使用 `Chimera++ 管理工具`
- [x] N2. 普通用户 UI 和注入菜单不显示 About、GitHub、Issues、推荐、赞助或交流群
- [x] N3. 后台 updater 可匿名使用本项目 GitHub Release，UI 不暴露实现细节
- [x] N4. 全新用户只填写 Key；固定 `https://api.chimerahub.org/v1`；升级不覆盖现有配置
- [x] N5. 启动自动检查/下载；仅最低支持版本硬阻断；受支持版本断网不锁死
- [x] N6. Windows 尽量自动安装；macOS 无签名时仍需用户确认
- [x] N7. 图标必须原创自有，不复制 Microsoft Store ChatGPT/OpenAI 图标
- [x] N8. 保留兼容性技术 ID 与原版覆盖升级能力
- [x] N9. 只跟踪上游正式 Release，不发行 upstream main 快照
- [x] N10. 每个 Step 和 Task 均执行 TDD 与双盲审计
- [x] N11. Windows 桌面只保留一个原创图标的 `Chimera++` 入口；管理工具不创建第二个桌面图标

## 大任务

- [x] T30. 品牌真相源、打包名称、单桌面入口和中英文客户 README
- [x] T31. 删除 About/GitHub UI 与注入菜单残留，迁移日志/诊断
- [x] T32. 启动自动更新、`minimum_supported_version` 与平台安装策略
- [x] T33. 原创 Chimera++ 图标及全平台资源替换
- [ ] T34. GitHub 同步/Release 上线与远端全绿
- [ ] T35. Windows/macOS 安装升级冒烟与最终聚合双盲审计

## 当前阻断事实

- [ ] 本地大量改动尚未提交/推送；远端 PR #1 只到 `161c4f4`，本地 HEAD 为 `693893d` 且另有未提交修复
- [ ] 本项目当前没有 GitHub Release 或 `latest.json`
- [ ] 最近三次 Actions 都失败；最新 PR run 的 Windows watcher 两项测试失败
- [ ] 远端尚无 `sync-upstream` workflow，`CHIMERA_AUTOMATION_TOKEN` 未配置
- [ ] 未完成 Windows/macOS 实机安装、覆盖升级、Gatekeeper 和自动更新冒烟
- [ ] 未确认并补齐本仓库最终发行许可证文件；不得继续声称未经证实的 MIT
