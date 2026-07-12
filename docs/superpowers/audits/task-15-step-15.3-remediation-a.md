# Task 15 Step 15.3 Remediation Audit A - Short-Lived Automation

> Status: **PASS**
> Date: 2026-07-13
> Auditor: independent audit A (final requirements and observable governance behavior)
> Independence: reviewed the final requirement, evidence, workflow contract and read-only GitHub API state; did not read, request or reference audit B
> Mutation boundary: no remote governance or secret configuration was modified during this audit

## Decision

The final remediation removes the long-lived external automation secret, binds every required check to the GitHub Actions App, and uses a short-lived job-scoped `github.token` to push the gated branch, open the PR, explicitly dispatch required checks, and enable auto-merge. The human first-release gate and satisfiable single-operator protection remain intact. No blocking finding was identified.

## Remote Governance Readback

| Requirement | Current observable result | Result |
|---|---|---|
| First-release approval | `public-release` requires reviewer `Duojiyi`, allows self review, and permits only protected branches | PASS |
| Single-person merge | Only direct collaborator is admin `Duojiyi`; approvals `0`, last-push approval false, auto-merge enabled | PASS |
| Exact required checks | Four expected contexts remain required with `strict: true` | PASS |
| Trusted check source | Every required check reports GitHub Actions `app_id: 15368` | PASS |
| No long-lived environment token | `upstream-sync` secrets endpoint returns `total_count: 0` and no names | PASS |
| Other main protection | Admin enforcement, stale-review dismissal, linear history and conversation resolution enabled; force-push/deletion disabled | PASS |
| Actions default least privilege | Repository default is `read`; Actions cannot approve PR reviews | PASS |

## Workflow Behavior

- Workflow-level permissions remain `contents: read`.
- Only `publish-sync-pr` receives the write set it uses: `contents`, `pull-requests`, `actions`, and `issues`.
- The publish step binds `GH_TOKEN` to `${{ github.token }}` and contains no `CHIMERA_AUTOMATION_TOKEN` reference.
- After creating or updating the sync PR, it actively runs `gh workflow run pr-build.yml --ref $branch`; a nonzero dispatch exit fails the job.
- Dispatch occurs before `gh pr merge --auto --squash`, allowing the app-bound required checks to attach to the gated branch commit before branch protection permits merge.
- The prepare job uses its read-scoped `github.token` only for the public upstream Release query. A local script compatibility fallback for a manually supplied environment token does not configure or expose a CI secret.

## TDD And Verification

`scripts/test-sync-upstream.ps1` scopes the contract to `publish-sync-pr` and requires the exact permission block, short-lived token binding, and active dispatch command. It rejects three mutations:

- replacement with `${{ secrets.CHIMERA_AUTOMATION_TOKEN }}`;
- removal of `actions: write`;
- commenting out the dispatch command.

Verification results:

- `pwsh -NoProfile -File scripts/test-sync-upstream.ps1`: PASS.
- Required-check workflow-name contract: PASS, 1/1.
- Read-only environment, secret, branch-protection, collaborator, repository and Actions-permission APIs: PASS.
- Targeted `git diff --check`: PASS; line-ending warnings only.

## Residual Execution Gate

The remediated sync workflow still requires deployment and a real `workflow_dispatch` exercise. Step 15.5 must prove that the built-in token can push the gated branch, create/update the PR, dispatch `pr-build.yml`, attach all four App-15368 checks and enable merge without any long-lived secret. This is the intended end-to-end sync gate, not a missing Step 15.3 contract.

## Gate

**PASS.** Independent remediation audit A approves the final Step 15.3 governance and short-lived automation design. The remediation may close only after independent audit B also passes.
