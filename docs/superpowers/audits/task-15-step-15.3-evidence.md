# Task 15 Step 15.3 Evidence - Repository Governance

> Date: 2026-07-13
> Repository: `Duojiyi/chimera-codex`

## Red

- `GET /repos/Duojiyi/chimera-codex/environments` returned only `upstream-sync`; the required first-release `public-release` environment was absent.
- `GET /repos/Duojiyi/chimera-codex/environments/upstream-sync/secrets` returned `total_count: 0`; `CHIMERA_AUTOMATION_TOKEN` was not configured.
- `main` required one approval and `require_last_push_approval: true`. With one repository operator, the final pusher could not satisfy both rules, so a fully green PR could not merge.
- The first attempted fallback stored the existing broad OAuth credential as `CHIMERA_AUTOMATION_TOKEN`, and one required check had `app_id: null`. Independent audit B rejected both the over-privileged long-lived secret and the unbound check source.
- The first short-lived-token remediation dispatched `pr-build.yml` from the candidate branch without proving that its workflow tree matched trusted main. Independent remediation audit B rejected this because a candidate-controlled workflow could emit the same App-bound required check names. The audit also demonstrated that the missing-`actions: write` mutation did not alter a pure CRLF fixture.

## Green

- Created `public-release` with `wait_timer: 0`, required reviewer `Duojiyi`, `prevent_self_review: false`, and deployments limited to protected branches.
- Changed only the unsatisfiable review requirements to `required_approving_review_count: 0` and `require_last_push_approval: false`.
- Bound all four required check contexts to the GitHub Actions App ID `15368`, preserving `strict: true`.
- Replaced the external-token design with the short-lived job-scoped `github.token`. Only `publish-sync-pr` receives `contents`, `pull-requests`, `actions`, and `issues` write access; all other workflow jobs retain their existing least privileges.
- Because a PR created by `GITHUB_TOKEN` does not recursively emit a `pull_request` workflow, `publish-sync-pr` explicitly dispatches `pr-build.yml` for the gated branch before enabling auto-merge.
- Deleted `CHIMERA_AUTOMATION_TOKEN` from `upstream-sync`. No long-lived automation credential remains.
- Candidate construction restores the complete `.github/workflows` tree from the immutable trusted main SHA with `git restore --staged --worktree`, removing upstream modifications and additions. The restored index is checked with `git diff --cached`; the committed new candidate and any resumed candidate are checked again by ref before gates.
- `publish-sync-pr` independently compares the imported gated branch workflow tree with `${{ github.sha }}` before push, dispatch or auto-merge. A difference or verification error fails closed.

## Readback

- `public-release` reports both `required_reviewers` and `branch_policy`; the branch policy is `protected_branches: true`, `custom_branch_policies: false`.
- `upstream-sync` reports `total_count: 0` environment secrets.
- `main` still requires the four exact checks with `strict: true`: `Branding / ads / Rust / frontend`, `Windows artifacts`, `macOS DMG (x64)`, and `macOS DMG (arm64)`. Each check reports `app_id: 15368`.
- `enforce_admins`, linear history and conversation resolution remain enabled. Force pushes and branch deletion remain disabled.
- Default Actions workflow permission remains `read`, and Actions cannot approve pull-request reviews.
- TDD workflow contract: the initial test failed against the external-token workflow; Green passes and fail-closed mutations reject a secret-token substitution, missing `actions: write`, and a commented dispatch command.
- The second Red required protected workflow restoration and pre-dispatch verification. Green enforces verification before dispatch before auto-merge, and the missing-permission mutation now changes and fails for both LF and CRLF fixtures.
- Follow-up mutations require `verify < push < dispatch < auto-merge` and independently remove the resume, committed-candidate and cached-index checks. A separate alternate Git index fixture proves that added, modified and deleted workflow entries are rejected while a clean HEAD index passes.

## Gate

The final workflow-trust remediation audits A and B both pass. Step 15.3 is complete. The first release still requires a `public-release` deployment approval in Step 15.4.
