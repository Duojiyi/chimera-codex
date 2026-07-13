# Task 15 Step 15.5 Evidence - Upstream Sync Conflict

> Date: 2026-07-13
> Upstream formal release: `v1.2.35`

## Red and remediation

- Initial workflow run `29211323039` detected the expected upstream conflict, but its
  report job could not create an Issue because `gh issue` attempted repository
  discovery in a job without checkout.
- Red contract `upstream_sync_issue_commands_bind_repository_explicitly` failed 0/1.
- PR #4 added explicit `--repo "$env:GITHUB_REPOSITORY"` to every Issue list,
  create, edit and close command. The focused contract passed 1/1 and the complete
  Windows/workflow contract passed 44/44.
- The first post-release PR run correctly exposed the unchanged macOS build number;
  fetching the release tag reproduced the branding Red locally. The build number was
  incremented from 1 to 2 and generated outputs were refreshed; branding and workflow
  regressions passed. Both remediations received independent A/B audits.
- PR #4 passed all five hosted checks and was normally squash-merged without an admin
  bypass as `591d5f361aa0081fb20cb850cf69ab01739db14d`.

## Final Green

- Manual workflow run `29213865141` ran on that immutable main SHA and detected the
  expected `v1.2.35` conflict.
- `Prepare and gate latest formal upstream Release` succeeded.
- `Upsert blocked Issue (conflict or gate failure)` succeeded and created/opened Issue
  #5, `[sync:v1.2.35] upstream sync blocked`.
- `Fail job on conflict or gate failure` then failed deliberately, making the overall
  run fail visibly as designed.
- `Push gated branch and open PR` was skipped. No remote
  `sync/upstream-v1.2.35` branch and no corresponding PR exists, so conflicted content
  was neither pushed nor merged. No Release was created for this upstream conflict.

This validates the conflict path. A future non-conflicting upstream formal release is
still expected to exercise the successful sync-PR path; that does not weaken the
verified fail-closed behavior recorded here.

## Processed-tag idempotency Green

- PR #6 passed the complete hosted gate and three platform builds, then was normally
  squash-merged as `eeb62316a1421813d4c6e4c5b98bf6048f40c361`.
- Manual run `29215957730` executed on that main SHA with
  `UPSTREAM_TAG: v1.2.34`, an upstream formal Release already represented by
  `v1.2.34-chimera.1`.
- The prepare job and overall run succeeded. Both `Report blocked upstream sync` and
  `Push gated branch and open PR` were skipped.
- After the run there were no open PRs, no remote `sync/*` branch, no new or modified
  Issue, and no additional Release. Existing conflict Issue #5 retained its previous
  update time.

This closes the processed-tag remote idempotency requirement without weakening the
default scheduled behavior, which continues to select the latest formal Release.
