# Task 15 Step 15.1 独立审计 B

> 日期：2026-07-12
> 范围：workflow diff、权限边界、发布依赖链、最低支持版本、required-check 合约与测试回归面
> 独立性：本审计未读取、引用或等待审计 A，只按当前工作树、规格、Plan、测试和变异结果复核。
> 结论：**PASS AFTER SECOND REMEDIATION**

## Findings

### B1（已关闭）：首发 environment 合约测试的注释绕过

初次审计时，该测试从 `publish-release` 后的文本尾部做 `contains("environment: public-release")`。把真实 job 键改成注释后，断言仍为真：

```text
environment-comment-mutation-still-passes=True
```

复现变异：

```diff
-    environment: public-release
+    # environment: public-release
```

补救后，测试按 publish job scope 只接受唯一、未注释且正确缩进的 `environment: public-release`，并内置对应 comment mutation。独立重跑结果：

```text
environment-comment-mutation-detected=True
```

### B2（已关闭）：最低版本 workflow 生成命令假阳性

初次审计时，该测试只搜索全文件字符串和调用次数。把实际 manifest 生成命令注释掉后，全部谓词仍为真：

```text
manifest-generation-comment-mutation-still-passes=True
```

复现变异：

```diff
-          node scripts/release-manifest.mjs --generate release-assets
+          # node scripts/release-manifest.mjs --generate release-assets
```

补救后，workflow 合约只识别非空、非注释的精确活动命令，并要求 self-test、generate 和两处 validate-floor；generate comment mutation 已内置。独立重跑结果：

```text
manifest-generate-comment-mutation-detected=True
```

`scripts/release-manifest.mjs --self-test` 同时继续覆盖缺省 floor、跨上游 floor、非法/过高/溢出值。

### B3（已关闭）：build-first 等价 Release API 漏检

初次审计时，副作用扫描仅拒绝 `gh release create`、`gh release upload` 和一个 tag push 字面量。publish job 前加入等价 Release API 写操作时，测试仍返回通过：

```text
alternate-release-api-mutation-still-passes=True
```

复现示例：

```yaml
run: gh api --method POST repos/owner/repo/releases
```

补救后，publish 前缀扫描新增 `gh api` 禁止项和对应 API mutation；全文件 `contents: write` 唯一且仅属于 publish job 的权限防线继续保留。独立重跑结果：

```text
alternate-release-api-mutation-detected=True
```

当前 publish 前所有 job 仍只有顶层 `contents: read`，没有发现其他凭据或写权限路径。

### B4（已关闭）：floor env 的 step-wide trim 多行值绕过

第二轮补救把 manifest 命令和 floor 字符串限制到 `Create draft, upload assets, publish` step，但 `active_release_manifest_contract` 对整个 step 的所有行执行 `trim` 后搜索精确文本，没有验证该行是 `env:` mapping 的直属键。以下有效 YAML 变异移除真实 floor env，同时把相同文本放入另一个环境变量的多行字符串：

```diff
-          MINIMUM_SUPPORTED_VERSION: ${{ vars.MINIMUM_SUPPORTED_VERSION }}
+          FLOOR_CONTRACT: |
+            MINIMUM_SUPPORTED_VERSION: ${{ vars.MINIMUM_SUPPORTED_VERSION }}
```

独立复现：

```text
floor-env-block-scalar-mutation-still-passes=True
real-floor-env-binding-present=False
```

该变异下 workflow 仍是有效 YAML，但 Node 进程收不到 `MINIMUM_SUPPORTED_VERSION`；`release-manifest.mjs` 会按设计退回当前 `VERSION`，从而忽略仓库配置的兼容 floor 并仍可完成发布。

第二轮补救后，测试保留 step 作用域，但只把原始行恰好为 10 空格缩进的 `MINIMUM_SUPPORTED_VERSION: ${{ vars.MINIMUM_SUPPORTED_VERSION }}` 视为直属 env 键；12 空格的 block-scalar 内容不再被 `trim` 后误接受，并内置上述 mutation。独立复跑结果：

```text
block-scalar-mutation-detected=True
direct-floor-env-present=False
```

## 已确认边界

- `.github/workflows/release-assets.yml:502-513` 的当前 `publish-release` 明确依赖 `resolve-version`、`gates`、`windows-installer` 和 `macos-dmg`。macOS 是 x64/arm64 固定矩阵，任一依赖失败时 publish job 不会启动。
- workflow 顶层为 `contents: read`；当前唯一 `contents: write` 位于绑定 `public-release` 的 publish job，没有发现其他 job 获得 Release 写权限。
- gates、Windows、macOS 与 publish 均 checkout `resolve-version.outputs.target_sha`；发布前再次校验 checkout、tag 和 Release target，未发现跨 SHA 混发路径。
- `latest.json` 生成器实际写入必填 `minimum_supported_version`，缺省为当前版本，并拒绝非法、外来、超前和超过 `u64` 的版本。
- PR required-check 名称现在按 job scope 校验，并覆盖 Windows 名称改动加注释不能掩盖的 mutation；macOS x64/arm64 两项也被检查。
- 当前 sync/release workflow 没有修改 main protection 的 API 调用，也没有 `--admin` 合并；`gh pr merge --auto --squash` 依赖远端 branch protection。远端 required checks 与 `public-release` required reviewer 是否真实配置属于 Step 15.3/15.4，不能由本地 YAML 声明替代。

## 验证结果

| 命令 | 结果 |
|---|---|
| `cargo test -p codex-plus-core --test installers --test updater --locked` | PASS，28 + 53 tests |
| `node scripts/release-manifest.mjs --self-test` | PASS |
| `pwsh -NoProfile -File scripts/test-sync-upstream.ps1` | PASS |
| `pwsh -NoProfile -File scripts/verify-no-upstream-ads.ps1` | PASS |
| `pwsh -NoProfile -File scripts/test-verify-allowlist.ps1` | PASS |
| Step 15.1 三项独立 mutation probes | PASS：三项原假阳性均被检测 |
| floor env block-scalar mutation | PASS：真实 env 移除后合约正确失败 |
| `git diff --check`（Step 15.1 相关文件） | PASS；仅现有 LF/CRLF 转换警告 |

## Gate

当前实现结构未发现 publish 先于 gates/三平台、权限外溢或 main protection 降级的路径；第一次审计发现的三个 workflow 合约假阳性与第二次复核发现的 floor env block-scalar 绕过均已通过独立 mutation 复跑关闭。远端 environment 保护、required checks 和真实平台 runs 仍由 Step 15.2-15.4 验证，不属于本地 Step 15.1 的通过声明。**Step 15.1 审计 B 最终结论为 PASS AFTER SECOND REMEDIATION。**
