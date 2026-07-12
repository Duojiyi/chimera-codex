# Task 15 Step 15.5 Sync Issue Repository - Audit A

> Date: 2026-07-13
> Branch: `codex/sync-issue-repo`
> Scope: real hosted failure, requirements, tests, and observable behavior only
> Conclusion: PASS

## Real Hosted Failure

Sync run `29211323039` at commit `28e46af1bffaba01b391dae244a29b8b702cd3ec` completed with failure after a real upstream merge conflict.

- `Prepare and gate latest formal upstream Release` completed successfully and exported conflict exit code `2` as designed.
- `Push gated branch and open PR` was skipped.
- `Report blocked upstream sync` downloaded the result artifact but has no checkout step and therefore no local `.git` repository.
- Its first `gh issue list` omitted `--repo`. GitHub CLI attempted repository inference and failed with `fatal: not a git repository`.
- The Issue was not created or updated, and the later explicit `Fail job on conflict or gate failure` step was skipped because the upsert step had already failed.

The run proves a repository-context defect in Issue reporting. It does not indicate that conflict detection, the gated branch, or the intended blocked state should be relaxed.

## Final Diff

All five active `gh issue` commands in `.github/workflows/sync-upstream.yml` now include:

```text
--repo "$env:GITHUB_REPOSITORY"
```

The complete Issue surface is covered:

1. Success-path lookup for a prior blocked Issue.
2. Success-path close of that Issue.
3. Blocked-path lookup for exact-title deduplication.
4. Blocked-path edit of an existing Issue.
5. Blocked-path creation of a new Issue.

This is appropriate even for the success job, which currently has a checkout, because every Issue operation is now independent of ambient Git state. `GITHUB_REPOSITORY` is a GitHub Actions default variable in `owner/repository` form and is accepted by the CLI's `--repo` option.

The implementation adds only explicit repository arguments. It does not add checkout to `report-blocked`, change Issue titles or bodies, alter deduplication, or touch sync artifacts and trusted outputs.

## Conflict And Failure Semantics

The original fail-closed flow remains intact:

- `prepare` still records sync exit code `2` for conflicts and `3` for gate failures while allowing the reporting jobs to evaluate the result.
- `report-blocked` still runs only when the trusted prepare output is `2` or `3`.
- The upsert still validates tag and Chimera version formats, maps `2` to `merge conflict`, deduplicates by exact title, and creates or updates the blocked Issue.
- After a successful upsert, `Fail job on conflict or gate failure` still throws unconditionally, so the workflow remains failed and cannot appear green on a blocked sync.
- `publish-sync-pr` remains skipped on the conflict path and is not made reachable by this change.

The fix therefore restores the intended sequence `detect conflict -> report Issue -> fail job`; it does not convert a conflict into success.

## Permissions

No permission line changes in the diff.

- Workflow default remains `contents: read`.
- `prepare` remains `contents: read`.
- `report-blocked` remains limited to `contents: read` and `issues: write`.
- No token, checkout credential, pull-request, action, or contents-write permission is added to the reporting job.

The explicit `--repo` argument selects the target repository but grants no authority beyond the existing job-scoped token.

## Test And Observable Checks

The focused test enumerates active, non-comment `gh issue` lines, requires exactly five commands, and requires the explicit repository argument on every command. The exact count prevents an unreviewed Issue command from silently escaping the contract.

| Check | Result |
| --- | --- |
| `cargo test -p codex-plus-manager --test windows_subsystem upstream_sync_issue_commands_bind_repository_explicitly -- --exact` | PASS, 1/1 |
| `cargo test -p codex-plus-manager --test windows_subsystem` | PASS, 44/44 |
| `gh issue list --repo Duojiyi/chimera-codex ...` from `C:\Windows\Temp` without a repository checkout | PASS, exit 0 |
| `cargo fmt --all -- --check` | PASS, exit 0 |
| `git diff --check` | PASS, exit 0; line-ending conversion warnings only |

## Audit A Conclusion

No requirement, permission, conflict-handling, Issue-reporting, or observable-behavior problem remains. Every Issue command is explicitly repository-bound, the no-checkout reporting job no longer depends on Git inference, and blocked syncs still create/update an Issue before deliberately failing. **Independent Audit A: PASS.**

A hosted conflict/gate-failure rerun is still required for real CI Green of the Issue-upsert step followed by the expected deliberate job failure; that operational evidence is not a defect in this candidate diff.
