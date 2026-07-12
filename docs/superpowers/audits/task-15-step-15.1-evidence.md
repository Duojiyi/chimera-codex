# Task 15 Step 15.1 Evidence - Release Workflow Contract

> Date: 2026-07-12
> Scope: build-first release, stable required checks, minimum supported version, public release environment and token permissions

## Red / Green

- Red: `first_release_publish_job_is_build_first_and_environment_gated` failed because `publish-release` had no `public-release` environment.
- Green: only `publish-release` uses `environment: public-release`; it depends on resolve, gates, Windows and both macOS matrix builds. Repository permissions remain `contents: read`, with the only `contents: write` scoped to publish.
- Audit mutations exposed comment, scope and equivalent-command false positives. The contracts now parse top-level job names, active step commands and the publish step's direct `env:` mapping rather than accepting arbitrary matching text.
- Mutations cover renamed jobs hidden by comments, commented environment/generator/floor bindings, floor text in a block scalar, pre-publish `gh release create`, pre-publish `gh api`, and extra pre-publish write permission.

## Verification

| Check | Result |
|---|---|
| installer workflow contracts | PASS, 28/28 |
| updater contracts | PASS, 53/53 |
| release manifest self-test | PASS |
| independent audit A | PASS |
| independent audit B | PASS AFTER SECOND REMEDIATION |

## Gate

**PASS.** Step 15.1 is complete. Remote environment, required checks and live runs remain Step 15.2-15.4.
