# Task 15 Step 15.1 Audit A - Release Workflow Contract

> Status: **PASS**
> Date: 2026-07-12
> Auditor: independent audit A (requirements and observable behavior)
> Independence: reviewed the Step 15.1 requirement, workflows, release-manifest generator and relevant tests; did not read or reference any audit B record

## Decision

The current `release-assets.yml` behavior is build-gated, environment-gated and least-privileged, and its manifest generator emits a validated `minimum_supported_version`. The final regression contracts structurally lock the required PR check names, reject a disguised name mutation, require both macOS matrix architectures, confine the only `contents: write` grant to `publish-release`, and reject known release/tag writes before that job. No blocking requirement or TDD gap remains in Step 15.1's local scope.

## Requirements Review

| Requirement | Observable evidence | Result |
|---|---|---|
| Build-first release | `publish-release` needs `resolve-version`, `gates`, `windows-installer` and `macos-dmg`; it downloads all three platform artifact groups before creating the draft | PASS (current behavior) |
| Minimum supported version | `MINIMUM_SUPPORTED_VERSION` is passed to `release-manifest.mjs`; generation always writes `minimum_supported_version`, validates it against the release version and the anonymous smoke validates the published manifest | PASS |
| First-release approval | Only `publish-release` has `environment: public-release` | PASS |
| Least privilege | Workflow-level permission is `contents: read`; only `publish-release` overrides it with `contents: write` | PASS |
| Required-check names remain stable | Job-scoped assertions lock `Branding / ads / Rust / frontend`, `Windows artifacts` and matrix `macOS DMG (${{ matrix.arch }})`; x64/arm64 matrix values and a rename-with-comment mutation are covered | PASS |
| Main protection is not weakened | Step 15.1 makes no repository-policy write; remote protection remains deferred to the explicit governance verification step | PASS for local scope |

## Resolved Findings

### A1 - Required check names are now covered by a scoped regression test

Resolved by `required_check_names_are_stable_and_release_side_effects_are_publish_only`. Its `job_section` helper extracts each top-level job, requires exactly one matching top-level `name`, checks both macOS matrix architectures, and proves that renaming the actual Windows job while retaining the old text in a comment is rejected.

### A2 - Build-first now excludes early publication side effects

Resolved by the same contract. It rejects known Release creation/upload, `gh api` and tag-push commands before `publish-release`; actual pre-publish YAML `run:` mutations for both `gh release create` and a Release API POST are rejected. It also requires workflow-level `contents: read` and requires the sole `contents: write` occurrence to be inside `publish-release`. Combined with the final job's explicit platform `needs`, earlier jobs cannot publish with the workflow token.

### A3 - Approval and manifest contracts now require active scoped lines

`public-release` must be the unique active environment line in the publish job, and commenting it out fails the mutation check. Manifest self-test, generation and the exact `MINIMUM_SUPPORTED_VERSION` environment binding must all be active inside the create/upload/publish step; both active floor-validation commands are counted exactly across the workflow. Commenting out either generation or the publish-step floor binding fails the contract before a Release can be built.

## Verification

| Command | Result |
|---|---|
| `cargo test -p codex-plus-core --test installers --locked first_release_publish_job_is_build_first_and_environment_gated -- --exact` | PASS, 1/1 |
| `cargo test -p codex-plus-core --test installers --locked required_check_names_are_stable_and_release_side_effects_are_publish_only -- --exact` | PASS, 1/1 |
| `cargo test -p codex-plus-core --test updater --locked release_workflow_emits_minimum_supported_version -- --exact` | PASS, 1/1 |
| `node scripts/release-manifest.mjs --self-test` | PASS |
| targeted `git diff --check` | PASS; line-ending warnings only |
| required-name, commented approval/floor, early Release and early API mutation fixtures | PASS; all prohibited mutations are rejected |

## Gate

**PASS.** Audit A approves Step 15.1's requirements and observable workflow behavior. The Step may be checked only after independent audit B also passes.
