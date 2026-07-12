# Task 15 Step 15.3 Audit A - Remote Repository Governance

> Status: **PASS**
> Date: 2026-07-13
> Auditor: independent audit A (requirements and observable remote behavior)
> Independence: reviewed only the Step 15.3 requirement, `task-15-step-15.3-evidence.md` and current read-only GitHub API responses; did not read, request or reference audit B
> Mutation boundary: no remote configuration was modified during this audit

## Decision

The current remote governance state satisfies the observable Step 15.3 requirements. The first public Release is protected by an explicit human deployment approval; the upstream automation secret exists by name without exposing its value; the review policy is satisfiable by the repository's single operator; and the four required checks plus the other recorded `main` protections remain enforced. No blocking finding was identified.

## Readback Results

| Requirement | Current GitHub API readback | Result |
|---|---|---|
| First-release human gate | `public-release` has required reviewer `Duojiyi` (`User`), `prevent_self_review: false`, and deployments limited to protected branches | PASS |
| Publish job uses the gate | `release-assets.yml` binds only `publish-release` to `environment: public-release` | PASS |
| Gate applies to the actual first Release | Repository Releases API currently returns zero releases | PASS |
| Upstream secret exists | `upstream-sync` reports exactly one environment secret named `CHIMERA_AUTOMATION_TOKEN` | PASS |
| Secret value is not exposed | GitHub API returned only secret metadata; workflow uses `${{ secrets.CHIMERA_AUTOMATION_TOKEN }}` and repository scan found no PAT-shaped value | PASS |
| Single-person merge is satisfiable | Direct collaborators contain only admin `Duojiyi`; required approvals are `0`, last-push approval is false, self deployment review is allowed, and repository auto-merge is enabled | PASS |
| Required checks retained | `strict: true` with exactly `Branding / ads / Rust / frontend`, `Windows artifacts`, `macOS DMG (x64)`, `macOS DMG (arm64)` | PASS |
| Other main protections retained | Admin enforcement, stale-review dismissal, linear history and conversation resolution are enabled; force pushes and deletions are disabled | PASS |
| Actions remains least privileged | Default workflow permission is `read`; Actions cannot approve pull-request reviews | PASS |

## Detailed Observations

- Environments API lists exactly `public-release` and `upstream-sync`.
- `public-release` has a required-reviewer rule and protected-branch deployment policy (`protected_branches: true`, `custom_branch_policies: false`). A wait-timer rule is absent, equivalent to no delay; the human reviewer rule remains the approval gate.
- `upstream-sync` is also limited to protected branches. Its secret endpoint returned `total_count: 1` and only the authorized secret name.
- Branch protection retains a pull-request review object even though the deliberately unsatisfiable approval count and last-push requirement were reduced. `dismiss_stale_reviews` remains true.
- `required_signatures` is false and branch restrictions are null, but neither was identified in the Red evidence as a protection removed by this Step. The explicitly preserved controls all match the evidence readback.

## Verification

Read-only endpoints used:

- `GET /repos/Duojiyi/chimera-codex/environments`
- `GET /repos/Duojiyi/chimera-codex/environments/public-release`
- `GET /repos/Duojiyi/chimera-codex/environments/upstream-sync`
- `GET /repos/Duojiyi/chimera-codex/environments/upstream-sync/secrets`
- `GET /repos/Duojiyi/chimera-codex/branches/main/protection`
- `GET /repos/Duojiyi/chimera-codex/actions/permissions/workflow`
- `GET /repos/Duojiyi/chimera-codex/collaborators?affiliation=direct`
- `GET /repos/Duojiyi/chimera-codex/releases?per_page=1`

Local read-only scans confirmed the secret appears only as a name/configuration reference and found no `github_pat_...` or `ghp_...` credential-shaped value outside ignored dependency/build metadata.

## Residual Verification

GitHub intentionally does not expose an Actions secret value or its fine-grained token scopes through the environment-secrets read API. This audit therefore does not claim to have inspected either. The real Step 15.5 upstream-sync run must verify that the credential has sufficient effective permissions without broadening `main` protection, and logs must remain free of token material.

## Gate

**PASS.** Independent audit A approves the observable Step 15.3 remote-governance state. Step 15.3 may close only after independent audit B also passes.
