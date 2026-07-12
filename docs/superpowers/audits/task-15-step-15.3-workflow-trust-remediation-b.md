# Task 15 Step 15.3 Workflow Trust Remediation Independent Audit B

> Date: 2026-07-13
> Scope: protected workflow restoration, new/resume/pre-publish equality, immutable trusted ref, operation ordering, candidate false-Green bypass and LF/CRLF mutations
> Method: independent final-diff inspection, executable contract, synthetic source mutations and read-only GitHub governance readback; no implementation or remote mutation

## Findings

No blocking finding remains in the final remediation diff.

## Implementation Review

The workflow-trust implementation itself closes the previously reported candidate-controlled workflow bypass:

- `Restore-ProtectedWorkflowTree` uses `git restore --source=<immutable SHA> --staged --worktree -- .github/workflows`. The complete directory path covers modifications, deletions and newly tracked workflow files rather than restoring only known filenames.
- The immediate `-Cached` comparison checks the restored index against the trusted tree, so a restore that leaves an added, modified or deleted workflow fails before the candidate commit.
- A new candidate is compared again by committed `HEAD` before candidate gates.
- A resumed remote candidate is compared with freshly fetched `origin/main` before its worktree and gates are used.
- `publish-sync-pr` independently imports/fetches the exact `gated_sha`, verifies the trusted `${{ github.sha }}` object exists, then compares the candidate workflow tree against that immutable commit.
- The pre-publish comparison is textually before `gh auth setup-git`, branch push, PR creation/update, explicit workflow dispatch and auto-merge enabling. Candidate workflow content cannot reach a push-triggered or manually dispatched execution first.
- The dispatched branch therefore contains the same `.github/workflows` tree as trusted main. A candidate cannot preserve the required context names while replacing their workflow programs, so GitHub Actions `app_id=15368` plus context checks no longer admit the reported self-certified false Green.
- The candidate branch can still change ordinary source and test inputs, as required for an upstream sync, but the trusted `pr-build.yml` keeps top-level `contents: read`; candidate content cannot elevate that token because it no longer controls the workflow definition.

The trusted refs are commit identities, not mutable branch-name comparisons at the final boundary. New-candidate construction resolves main to a SHA; publication validates `${{ github.sha }}` as lowercase 40-hex and as an existing commit before tree comparison. A change to main during the run can cause a conservative mismatch, but cannot make an untrusted workflow tree equal to a different immutable trusted object.

## Regression Review

Verified Green coverage:

- The workflow contract now requires `verify < push < dispatch < auto-merge`.
- Moving verification after push changes the normalized fixture and fails the contract.
- Secret-token substitution, missing `actions: write`, commented dispatch and post-push verification all fail.
- The original permission mutation changes and fails for both independently constructed LF and CRLF workflow inputs.
- The protected-script contract now names the cached restore check, resume check, restore call and committed-candidate check.
- `Test-ProtectedScriptContract` canonicalizes CRLF and CR to LF, and raw script source is canonicalized before line-based mutations. Removing `-Cached`, changing the protected path, and removing the resume/candidate assertions therefore fail independently of the checkout line ending.
- A synthetic CRLF protected-path mutation is also rejected. An independent pure-CRLF probe confirmed canonicalization followed by resume, candidate, cached and protected-path mutations gives `changed=true` and `contract=false` for all four cases.
- A separate `GIT_INDEX_FILE` fixture uses real Git index operations to prove added, modified and deleted `.github/workflows` entries are rejected while a clean `HEAD` tree passes. It does not alter the working index.
- `pwsh -NoProfile -File scripts/test-sync-upstream.ps1` passes with the final canonicalized mutations and real Git index fixture.
- `cargo test -p codex-plus-manager --test windows_subsystem --locked upstream_sync_semver_fixture_contract_passes -- --exact` passes `1/1`, confirming the repository integration entry invokes the same contract successfully.
- `git diff --check` passes for the three remediation files, with only expected checkout conversion warnings.

## Remote Readback

Read-only GitHub API checks still report:

- `main` required checks `strict: true`.
- All four exact required contexts have `app_id: 15368`.
- Admin enforcement remains enabled; force push and deletion remain disabled.
- `upstream-sync` has `total_count: 0` secrets.

No secret values were requested or read, and no remote state was changed.

## Conclusion

**PASS.** The final workflow-trust remediation closes the candidate-controlled `pr-build.yml` false-Green path, restores and compares the complete workflow tree at the required new/resume/pre-publish boundaries, uses an immutable final trusted ref, and verifies before push, dispatch and auto-merge. The permission and protected-script mutations now remain effective across LF and CRLF inputs, and real Git index add/modify/delete cases fail closed. Step 15.3 may proceed to its remaining independent and aggregate gates.
