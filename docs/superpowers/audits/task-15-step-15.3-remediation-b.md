# Task 15 Step 15.3 Remediation Independent Audit B

> Date: 2026-07-13
> Scope: final repository-governance readback, built-in token permissions, explicit dispatch/auto-merge ordering, mutations and bypass surface
> Method: independent read-only GitHub API/CLI readback plus local source and executable-contract inspection; no remote or implementation mutation

## Blocking Finding

### B1 - Candidate branch controls the workflow that certifies the candidate

**Severity: blocking.** `publish-sync-pr` runs:

```text
gh workflow run pr-build.yml --ref $branch
gh pr merge $pr --auto --squash
```

The `--ref $branch` dispatch executes the `pr-build.yml` definition at the generated sync branch. That branch is the merge result of an upstream Release tag and can therefore modify `.github/workflows/pr-build.yml` itself. A candidate can retain the four required job names while replacing their commands with successful no-ops. Those checks would still be emitted by GitHub Actions App `15368`, satisfying the current context plus `app_id` branch-protection bindings before auto-merge.

The risk is broader than false Green: a same-repository `workflow_dispatch` candidate can declare its own workflow permissions, so untrusted candidate workflow content is being executed as repository automation rather than only compiled/tested by a workflow definition anchored to trusted `main`.

The local pre-gate does not close this path. `scripts/sync-upstream.ps1` runs branding, ad scan, formatting and tests from the merged candidate worktree, but it does not pin or independently attest the trusted `pr-build.yml` definition. App binding proves only the producer application, not which revision supplied the workflow program.

Until required checks are produced by a gate definition anchored to trusted `main` (while testing the immutable candidate SHA), or equivalent protection prevents candidate workflow changes from certifying themselves, Step 15.3's required-check/auto-merge governance claim is not fail-closed.

## Additional Test Gap

### B2 - The missing-permission mutation is line-ending-sensitive

`Test-BuiltInTokenWorkflowContract` normalizes CRLF/CR to LF, but its `actions: write` mutation operates on raw input:

```text
$syncWorkflow.Replace("      actions: write`n", '')
```

Independent probes showed:

```text
LF:   changed=true,  contract_after_mutation=false
CRLF: changed=false, contract_after_mutation=true
```

The repository's `.gitattributes` currently pins `.github/workflows/*.yml` to LF, so the real checked-in fixture does mutate and the normal regression passes. This is not a current production-line-ending bypass, but the mutation test itself does not prove its intended fail-closed property for every input accepted by the normalizing contract. It should normalize before mutation or assert that every mutation changed the fixture.

## Verified Remediation Facts

- `main` retains `strict: true` and the four exact required checks. Every check is now bound to GitHub Actions `app_id: 15368`.
- `enforce_admins`, linear history and conversation resolution remain enabled; force pushes and deletion remain disabled.
- Repository Actions defaults remain `default_workflow_permissions: read` and `can_approve_pull_request_reviews: false`.
- `upstream-sync` is restricted to protected branches and its secret listing is exactly `total_count: 0`; no secret values were requested or read.
- Workflow top level and `prepare` retain `contents: read`. `report-blocked` has only `contents: read` and `issues: write`.
- Only `publish-sync-pr` receives `contents: write`, `pull-requests: write`, `actions: write`, and `issues: write`. Each permission has a direct operation in that job: branch push, PR create/edit/auto-merge, workflow dispatch, and closing a prior blocked Issue.
- `publish-sync-pr` uses `${{ github.token }}` and contains no `CHIMERA_AUTOMATION_TOKEN` reference. The environment's zero-secret readback matches the source contract.
- Explicit `pr-build.yml` dispatch textually precedes auto-merge enabling, and a nonzero dispatch exit is handled before the merge command.
- `pwsh -NoProfile -File scripts/test-sync-upstream.ps1` passes on the repository fixture. External-secret and commented-dispatch mutations change the fixture and fail the contract; the LF missing-permission mutation also changes the fixture and fails.
- `git diff --check -- .github/workflows/sync-upstream.yml scripts/test-sync-upstream.ps1` passes (Git reports only expected checkout line-ending warnings).

## Read-only Commands Replayed

```text
gh api repos/Duojiyi/chimera-codex/branches/main/protection
gh api repos/Duojiyi/chimera-codex/actions/permissions/workflow
gh api repos/Duojiyi/chimera-codex/environments/upstream-sync
gh api repos/Duojiyi/chimera-codex/environments/upstream-sync/secrets
gh api repos/Duojiyi/chimera-codex/environments/public-release
pwsh -NoProfile -File scripts/test-sync-upstream.ps1
git check-attr -a -- .github/workflows/sync-upstream.yml scripts/test-sync-upstream.ps1
git diff --check -- .github/workflows/sync-upstream.yml scripts/test-sync-upstream.ps1
```

## Conclusion

**FAIL.** The long-lived-token and unbound-check-source findings are remediated: all four checks are strict and App-bound, `upstream-sync` has zero secrets, and job-scoped built-in-token permissions are locally minimal. However, the explicitly dispatched required-check workflow is selected from the candidate branch it certifies, so App-bound successful contexts do not establish that trusted gates ran. B1 must be closed and regression-tested before Step 15.3 can pass. B2 should be corrected in the same remediation cycle so its mutation evidence is unambiguously effective.
