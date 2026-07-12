# Task 13 Aggregate Audit - T32 Gate

> Status: **PASS**
> Date: 2026-07-12
> Scope: minimum-supported manifest, trusted floor cache, automatic/mandatory startup update, Windows silent launch and unsigned macOS confirmation flow

## Implemented Behavior

- `latest.json` generation and parsing carry a validated `minimum_supported_version`; Node and Rust version bounds agree.
- The trusted floor uses independent `update-state.json`, the existing hardened atomic writer and a shared cross-process lock.
- Trusted floor values only rise. Corrupt state is no-replace quarantined; concurrent quarantine cannot displace a recovery write.
- Offline startup blocks only when a cached floor proves the current version unsupported. Missing/corrupt cache allows startup.
- A server rollback below the cached floor is not exposed as an installable release. A mandatory floor without a complete native asset is not cached.
- Launcher checks update state before settings/login routing. Any valid update opens Manager for automatic handling; mandatory state remains highest priority.
- Manager automatically checks/downloads on startup and on forwarded update routes. Mandatory state uses a blocking retry surface.
- Ordinary install failure can continue only through a version-bound, single-use token that rechecks the maximum trusted floor; cached mandatory state refuses continuation.
- The install path revalidates the selected release under the latest floor lock immediately before launch, and Manager/React single-flight covers the complete check-download-launch flow.
- Windows launches the verified installer with `/S`; NSIS success, section failure and pre-section exit paths use guarded relaunch callbacks so a supported old version can continue once after rollback.
- macOS waits for `hdiutil attach -autoopen` to finish, keeps Manager running and explicitly requires user confirmation.
- Existing NSIS transaction/rollback and verified-file identity protections remain in force.

## Regression Ledger

| Command/area | Result |
|---|---|
| Core updater | 53/53 pass |
| Core lib | 156/156 pass |
| Manager lib | 54/54 pass |
| Manager Windows/static contracts | 43/43 pass |
| Launcher | 8/8 pass |
| Installer/workflow contracts | 24/24 pass |
| TypeScript | pass |
| Vite production build | pass, 1608 modules |
| i18n | plain 568/568, template 36/36, manifest exact |
| Production promotion/branding scanner | pass |
| Branding generation check | pass |
| Release manifest executable self-test | pass |
| Rust format and git diff checks | pass |

## Deferred Real-Platform Gates

- Actual Windows silent replacement, SmartScreen behavior, rollback fault injection and process exit require Task 16 Windows smoke.
- Actual macOS x64/arm64 DMG mount, Finder flow, cancellation and Gatekeeper confirmation require Release CI and Task 16 macOS smoke.
- Remote Actions, auto-merge and public Release publication remain Task 15.

## Aggregate Decision

Task 13 / T32 is complete. The initial independent aggregate audits both found blockers; after TDD remediation, `task-13-remediation-a.md` and `task-13-remediation-b.md` independently passed the final tree. This decision does not claim real-platform or public Release validation, which remain Tasks 15-16.
