# Task 15 Step 15.3 远端治理独立审计 B

> 日期：2026-07-13
> 仓库：`Duojiyi/chimera-codex`
> 范围：权限最小化、environment 边界、main branch protection、secret 暴露面、auto-merge 与发布回归风险
> 独立性：未读取、询问或引用审计 A；GitHub API 仅执行只读请求；未读取或输出任何 secret 值；未修改远端。
> 结论：**FAIL**

## Findings

### B1（阻断）：主 gates required check 未绑定 GitHub Actions App

`GET /branches/main/protection` 返回四个 required checks，但来源绑定不一致：

| Context | app_id |
|---|---:|
| `Branding / ads / Rust / frontend` | `null` |
| `Windows artifacts` | `15368` |
| `macOS DMG (x64)` | `15368` |
| `macOS DMG (arm64)` | `15368` |

当前 PR head 的真实 `Branding / ads / Rust / frontend` check-run 来自 `github-actions`，`app.id=15368`。保护规则中的 `app_id=null` 只锁 context 名称、不锁来源，其他 App/commit status 可用同名 context 满足该项；在启用 auto-merge 且批准数为 0 的治理下，这是实际绕过面。

关闭要求：把该 check 与其余三项一样绑定 `app_id=15368`，再只读回验四项 context、四项 app_id、`strict=true`，并确认一次真实 PR check-run 的 name/App 对应。

### B2（阻断）：`CHIMERA_AUTOMATION_TOKEN` 的最小权限无法由现有证据证明

Environment secret API 安全地证明：

```json
{"total_count":1,"names":["CHIMERA_AUTOMATION_TOKEN"]}
```

但 API 不返回 secret 类型或权限，Step 15.3 evidence 只写“authorized automation credential”，没有记录它是独立 fine-grained PAT 还是 GitHub App token，也没有记录非敏感权限清单/仓库限制。当前本机唯一可观察的 GitHub OAuth 授权包含 `repo`、`workflow`、`gist`、`read:org`；这不能证明 environment secret 与它相同，但如果存入的是该凭据，则明显超出 workflow 所需权限。

按当前 workflow，长期凭据只需要限定 `Duojiyi/chimera-codex` 的 Contents、Pull requests、Issues read/write（以及平台隐含 Metadata read），不需要 Actions/workflow、Gist、组织读取或其他仓库权限。

关闭要求：不披露 token 值，只补充可审计的凭据类型、仅限仓库和权限清单；若当前 secret 来自宽权限 OAuth token，替换为独立 fine-grained PAT 或 GitHub App 安装 token，再仅回验 secret 名称/数量和非敏感权限元数据。

## 已通过的远端治理项

### Environments

- `public-release` 存在，required reviewer 为 `Duojiyi`，`prevent_self_review=false`，适配单一仓库管理员的首次发行审批。
- `public-release` deployment policy 为 `protected_branches=true`、`custom_branch_policies=false`；非保护分支不能取得发布环境。
- `upstream-sync` 同样只允许 protected branches，环境中恰有一个预期名称的 secret。
- sync workflow 另有 `github.ref == refs/heads/main` 条件；长期 token 只进入 `publish-sync-pr` job 的单个 PowerShell step。
- prepare/gates job 只使用默认 `github.token` 的读权限，candidate artifact、测试代码和日志路径均接触不到长期 secret。

### Main Protection

- main 已保护，仍要求 PR；仅把单操作员无法满足的 approvals 从 1 降为 0，并关闭 last-push approval。
- `required_status_checks.strict=true`，四个 context 名称与 PR workflow job 名一致。
- `enforce_admins=true`、linear history 和 conversation resolution 均启用。
- force pushes 与 branch deletion 均禁用；无额外 ruleset 降低保护。
- repository `allow_auto_merge=true`，`gh pr merge --auto --squash` 不使用 `--admin`；当前 checks 失败/运行中时不会立即合并。

### Actions 与发布

- 默认 Actions workflow 权限为 `read`，`can_approve_pull_request_reviews=false`。
- sync workflow 顶层及 prepare/publish-sync-pr job 的 `GITHUB_TOKEN` 为 `contents: read`；blocked Issue job 单独只有 `issues: write`。
- release workflow 顶层只读，唯一 `contents: write` 位于依赖 gates、Windows、macOS x64/arm64 的 `publish-release`，且绑定 `public-release`。
- release workflow 只允许 main，发布 target 固定到解析出的 commit SHA；environment 审批前构建 job没有 release 写权限。
- `upstream` push URL 仍为 `no_push://upstream`，未发现 Chimera 定制误推上游的路径。

## 只读回验摘要

| API / 验证 | 结果 |
|---|---|
| repository metadata | public，default=`main`，auto-merge enabled |
| Actions workflow permissions | default=`read`，cannot approve PR reviews |
| main protection | strict checks、PR required、admins enforced、no force/delete |
| environments | `public-release` + `upstream-sync`，均 protected branches only |
| upstream-sync secret list | 1 个，仅名称 `CHIMERA_AUTOMATION_TOKEN` |
| current PR head check-runs | 四个构建 check 均来自 GitHub Actions 15368；gate 已成功，其余运行中/成功 |
| `pwsh -NoProfile -File scripts/test-sync-upstream.ps1` | PASS |

## Residual Risks

- `required_signatures=false`，提交签名不是当前规格门禁；发行真实性仍依赖 GitHub 仓库、environment 审批和不可复用 tag。
- approval count 为 0 是单操作员 auto-merge 的明确取舍；安全性依赖 required checks 的来源绑定，因此 B1 必须先关闭。
- `public-release` 当前会门控所有正式发布；Step 15.4 首发完成后若切换全自动，必须用新的治理变更和审计记录完成，不应静默删除环境保护。

## Gate

Environment 边界、main 的大部分保护、Actions 默认权限和 secret 暴露时机均符合设计；但一个核心 required check 未锁来源，且长期 automation credential 的最小权限没有可审计证据。**Task 15 Step 15.3 独立审计 B 结论为 FAIL；B1/B2 关闭前不得勾选 Step 15.3。**
