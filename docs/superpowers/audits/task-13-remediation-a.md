# Task 13 Remediation Audit A - T32 Gate

> Status: **PASS**
> Date: 2026-07-12
> Auditor: independent remediation audit A
> Independence: reviewed the original aggregate A/B findings and the final worktree; did not read or reference any remediation audit B record

## Decision

All findings from aggregate audit A and the observable requirements from aggregate audit B are closed in the final worktree. No new blocking finding remains in the remediation scope.

## Finding Closure

### A1 - Missing native asset before floor persistence: closed

- Manager calls `validate_release_for_install(&release)` before `record_trusted_floor`.
- The validator requires complete current-platform asset metadata before the floor can rise.
- Core also validates again before download/launch. An incomplete native release therefore cannot create a cached mandatory floor without an installable asset.

### A2 - Node/Rust Chimera version domain: closed

- Rust now accepts at most one `v`/`V`, rejects surrounding whitespace, requires the exact `chimera.N` channel and constrains the revision to `u64`.
- Node applies the same canonical grammar and `u64` bounds to all four numeric components.
- Boundary coverage includes `u64::MAX`, overflow, repeated prefix, whitespace, leading zero, foreign channel and build metadata.

### B1 - Optional continuation authorization: closed

- The forgeable Boolean environment bypass is gone.
- Continuations are random UUID tokens persisted atomically, bound to the current application version, single-use and time-limited.
- Launcher always evaluates startup update state and consumes a token only for an automatic update; mandatory state remains unconditionally blocking.
- Consumption re-reads the latest trusted floor and evaluates the maximum of the observed and latest floor. A floor rise above the current version refuses continuation, while a rise to the supported boundary still permits it.
- Invalid, missing, expired, reused and version-mismatched tokens fail closed.

### B2 - Floor race at installer launch: closed

- Download and launch-time file identity validation complete before installer authorization.
- `authorize_release_install` acquires the update-state lock, reloads the latest floor, rejects a release below it and keeps the lock through process launch.
- Tests prove a concurrently raised floor prevents the launch closure from running.

### B3 - Platform completion semantics: closed for static/unit scope

- Windows receives `/S` plus an optional-update continuation token. NSIS reports failure, restores the old transaction state and relaunches the surviving launcher with the one-time continuation; success relaunches exactly once without a bypass token.
- The token is issued after the potentially long download, and its two-hour TTL covers the installer handoff instead of expiring during the configured 30-minute download window.
- macOS uses blocking `hdiutil ... .status()`, rejects a non-zero mount result, keeps Manager alive and reports only that the DMG is open and user confirmation is required.

### B4 - Duplicate update execution: closed

- Frontend `updateInstallInFlightRef` now covers the entire check-to-install flow, so duplicate forwarded routes are coalesced before either check or install starts.
- Direct/manual `performUpdate` uses the same guard without prematurely clearing a flow-owned guard.
- Backend `UpdateOperationGuard` permits only one fetch/download/launch operation at a time.
- A busy backend response is marked `mandatoryUpdate: true` and `updateInProgress: true`, preventing the rejected duplicate request from triggering ordinary-failure continuation.

## Focused Verification

| Command or area | Result |
|---|---|
| `cargo test -p codex-plus-core --test updater --locked` | PASS, 53/53 |
| `cargo test -p codex-plus-launcher --locked` | PASS, 8/8 |
| targeted Manager update/static contract | PASS, 1/1 |
| targeted Manager backend single-flight guard | PASS, 1/1 |
| `cargo test -p codex-plus-core --test installers --locked` | PASS, 24/24 |
| `npm run check` in Manager | PASS |
| `node scripts/release-manifest.mjs --self-test` | PASS |
| `pwsh -NoProfile -File scripts/verify-no-upstream-ads.ps1` | PASS |
| `pwsh -NoProfile -File scripts/test-verify-allowlist.ps1` | PASS, all fail-closed fixture classes |
| `cargo fmt --all -- --check` | PASS |
| targeted `git diff --check` | PASS; line-ending warnings only |

The first production scanner run exposed stale exact allowlist line numbers after the NSIS callback insertion. The allowlist was updated only for the existing legacy-shortcut cleanup references; the real scan and fail-closed self-test then both passed.

## Residual Platform Risk

- Actual Windows silent replacement, forced rollback fault injection, SmartScreen behavior and process-exit timing still require the declared Task 16 Windows smoke gate.
- Actual macOS x64/arm64 DMG mount, Finder replacement, cancellation and unsigned Gatekeeper confirmation still require Release CI and the Task 16 macOS smoke gate.
- Remote Actions and public Release publication remain Task 15. This audit does not claim those deferred gates.

## Gate

**PASS.** Remediation audit A approves Task 13. T32 may be checked only after the independent remediation audit B also passes.
