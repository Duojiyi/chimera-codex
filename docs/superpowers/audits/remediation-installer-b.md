# Remediation Installer Audit B - 宏展开、恢复边界与打包回归

> Status: **PASS**
> Audit date: 2026-07-10
> Recorded: 2026-07-11
> Scope: NSIS 宏标签、注册表恢复、品牌门与 macOS packager

## 独立审计 B

- NSIS 宏展开标签按 slot 唯一，不存在跨宏跳转冲突。
- registry probe 覆盖缺失键、空键和有值/子键状态；删除失败会保留卸载器并中止。
- 快捷方式、注册表与备份清理顺序可重复执行，不把部分失败伪装为成功。
- `.bak` 和 staged 文件均在成功路径显式清理。
- 品牌门覆盖 workflow 活动资产名、中英文 README 与旧前缀负向检查。
- macOS 输入架构拒绝未知值，两个 bundle 二进制分别执行 `lipo` 校验与 strict codesign 验证。

## 验证证据

- installer tests：18/18。
- branding `-Check`：PASS。
- `git diff --check`：PASS。

## 结论

PASS。静态实现与回归面无阻断项；真实 NSIS 编译、Windows 故障注入和 macOS 双架构 DMG 冒烟仍属于外部验收。
