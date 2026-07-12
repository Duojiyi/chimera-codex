# Task 15 Step 15.3 Workflow Trust Remediation - Independent Audit A

> Date: 2026-07-13
> Scope: requirements, Red/Green evidence, and observable workflow-trust behavior only
> Remote access: read-only

## Verdict

**PASS.** The second workflow-trust remediation closes the candidate-controlled workflow path. New candidates restore the complete protected workflow tree from a resolved immutable main commit, verify the restored index, and verify the committed HEAD. Resume candidates are compared with the freshly fetched `origin/main`. The publishing job independently compares the gated candidate with `${{ github.sha }}` before any push, dispatch, or auto-merge operation. No blocking requirement or observable-behavior gap was found.

## Requirement Readback

| Requirement | Evidence | Result |
|---|---|---|
| Restore the complete workflow tree | `Restore-ProtectedWorkflowTree` invokes `git restore --source=<trusted SHA> --staged --worktree -- .github/workflows`; Git semantics cover tracked additions, modifications, and deletions in the protected tree | PASS |
| Verify the restored index | Restore immediately calls `Assert-ProtectedWorkflowTree ... -Cached`, which executes `git diff --cached --quiet <trusted SHA> -- .github/workflows` and rejects both exit `1` and verification errors | PASS |
| Verify the committed candidate | After the sync commit and SHA validation, the script compares `HEAD` with the same resolved `$mainSha` before running gates | PASS |
| Verify resume candidates | `Test-RemoteSyncBranch` freshly fetches `origin/main` and the remote sync branch, then compares the candidate ref with `origin/main` before creating the resume worktree or running gates | PASS |
| Independent publish verification | `publish-sync-pr` binds `TRUSTED_MAIN_SHA` to `${{ github.sha }}`, validates it as a commit, and runs `git diff --quiet $trustedMainSha refs/heads/$branch -- .github/workflows` | PASS |
| Fail before side effects | Textual and mutation contracts enforce `verify < push < dispatch < auto-merge`; the diff check is therefore before push on `prepared` and before dispatch/auto-merge on both `prepared` and `resume` | PASS |
| Trusted required-check definition | The dispatched `pr-build.yml` is selected from the gated branch only after all three candidate paths establish that `.github/workflows` matches trusted main | PASS |
| Job-scoped built-in token | Only `publish-sync-pr` receives `contents`, `pull-requests`, `actions`, and `issues` write; these map respectively to branch push, PR create/edit/merge, workflow dispatch, and closing the prior blocked Issue. `GH_TOKEN` is `${{ github.token }}` | PASS |

## Red / Green and Mutation Evidence

- The recorded second Red identifies the observable failure: a candidate branch could previously supply the workflow definition used to certify itself. The Green adds restoration plus index/HEAD/resume checks and an independent publish-time check anchored to `${{ github.sha }}`.
- The current sync contract rejects secret-token substitution, missing `actions: write`, commented dispatch, post-push verification, and removal of the cached-index, committed-candidate, or resume-candidate checks.
- The missing-permission mutation is effective for normalized LF and CRLF fixtures. The test normalizes its input before contract evaluation, so line-ending form does not bypass the assertion.
- An isolated `GIT_INDEX_FILE` fixture dynamically stages an added workflow, a modified workflow, and a deleted workflow; all three are rejected by the real `Assert-ProtectedWorkflowTree -Cached` implementation. A clean `HEAD` index passes. This confirms behavior rather than only source-text presence.

## Remote Governance Readback

Read-only GitHub API responses on 2026-07-13 showed:

- `main.required_status_checks.strict = true`;
- exactly `Branding / ads / Rust / frontend`, `Windows artifacts`, `macOS DMG (x64)`, and `macOS DMG (arm64)` are required;
- every required check is bound to GitHub Actions `app_id = 15368`;
- `upstream-sync` environment secrets are `total_count = 0` with no names;
- repository auto-merge remains enabled and admin enforcement remains enabled.

## Regression Results

| Command | Result |
|---|---|
| `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/test-sync-upstream.ps1` | PASS (`sync-upstream contract tests passed`) |
| `cargo test -p codex-plus-core --test installers --test updater` | PASS (28/28 installers, 53/53 updater) |
| `git diff --check` | PASS; only expected working-tree LF/CRLF conversion warnings were emitted |

## Residual Boundary

The remediated workflow is not yet deployed on remote `main`, so this audit does not claim a live sync execution. Step 15.5 must still prove that the built-in token can push the exact gated SHA, dispatch the four App-bound checks, and enable auto-merge in a real run. That deployment exercise is outside this Step 15.3 workflow-trust contract and is not a blocker for this audit.
