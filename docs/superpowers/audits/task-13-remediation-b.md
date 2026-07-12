# Task 13 Remediation Audit B

> Status: **PASS**
> Date: 2026-07-12
> Independence: reviewed the frozen remediation diff without reading or referencing remediation audit A
> Scope: continuation forgery/replay, trusted-floor races, duplicate update requests, Windows post-spawn failure, macOS mount failure, and version boundary parity

## Decision

PASS. The remediation closes all four findings from the earlier aggregate B audit. No new blocking defect was found in the composed launcher, Manager, updater and installer flow.

## Verification

### Continuation authorization

- The public Boolean skip switch is gone. Manager/installer continuation uses a random persisted token bound to the exact current Chimera version and the observed trusted floor.
- A matching token is removed before validation completes, so it is single-use even when expired, version-mismatched or rejected by a newer floor. A forged token neither authorizes startup nor consumes the valid record.
- Consumption re-reads the current trusted floor and evaluates the maximum of the observed and latest floors. A raised floor at or below the current version remains available, while a floor above the current version still forces the update.
- The two-hour TTL is issued after the potentially long download, not before it. This preserves enough time for Windows transaction failure/relaunch without leaving an unbounded bypass.
- Launcher always evaluates startup update state; a continuation can suppress only `Automatic`, never `Mandatory`.

### Floor and install races

- Asset download and launch-time file identity validation remain intact.
- Immediately before launch, `authorize_release_install` takes the shared update-state lock, reloads the latest floor, rejects a release below it and keeps the lock through installer/mount launch. This supplies a clear linearization point for concurrent floor writers.
- If the floor rises after the optional continuation was issued, consumption independently rechecks the newer effective floor. The install and continuation paths therefore fail conservatively in both interleavings.

### Duplicate and forged requests

- The frontend ref now covers the entire automatic check/download/launch chain and is released in `finally`; the nested install call explicitly reuses ownership instead of deadlocking itself.
- Backend `perform_update` also has an atomic process-wide single-flight guard. A duplicate IPC request gets `updateInProgress=true` and `mandatoryUpdate=true`, so frontend failure handling cannot misinterpret the busy response as an optional install failure and issue a continuation.
- Version-only IPC input is still rebound to a freshly fetched trusted release, and same-version, downgrade or changed-version requests remain rejected.

### Platform process semantics

- Windows still launches the verified installer with `/S`, now adding only a UUID-shaped continuation argument for supported optional updates.
- NSIS has mutually exclusive success, transaction-failure and GUI-end fallback relaunch paths guarded by `UpdateRelaunchHandled`. Successful install starts the new entry; failure with a retained executable starts it with the single-use continuation; initialization/early-exit fallback cannot duplicate an already handled relaunch.
- macOS now waits for `hdiutil attach -autoopen` and requires a successful exit status before reporting that the confirmation flow is ready. Manager remains running and the result still says user confirmation is required; closing Finder or declining to copy the app is not represented as a completed installation.

### Version boundaries

- Rust rejects surrounding whitespace, double prefixes, foreign channels, build metadata and revision values above `u64::MAX`; it accepts `u64::MAX` and normalizes one `v`/`V` prefix.
- The executable Node manifest helper applies matching numeric grammar and `u64` bounds. Default, cross-upstream, invalid/foreign, above-latest and numeric overflow cases remain covered.

## Commands

```text
cargo test -p codex-plus-core --test updater --locked
PASS - 53/53

cargo test -p codex-plus-manager --test windows_subsystem --locked manager_update_install_keeps_visible_progress_bar -- --exact
PASS - 1/1

cargo test -p codex-plus-core --test installers --locked windows_update_installer_reports_completion_and_relaunches_exactly_once -- --exact
PASS - 1/1

cargo test -p codex-plus-manager --lib --locked
PASS - 54/54

node scripts/release-manifest.mjs --self-test
PASS

git diff --check -- <Task 13 remediation files>
PASS - no whitespace errors; only existing line-ending conversion warnings
```

## Residual Risk

- Static NSIS contracts cannot prove real Windows callback ordering, SmartScreen behavior, process replacement or rollback under injected filesystem faults. Those remain mandatory Task 16 smoke gates.
- This Windows host cannot execute the macOS `hdiutil`, Finder copy/cancel and Gatekeeper flows. Release CI and real x64/arm64 macOS smoke remain required.
- The trusted-floor cache is still not intended to resist a local user who can edit application state files; this is the documented threat-model boundary, not a remediation regression.

## Gate

**PASS.** Remediation B has no open finding. Task 13 may proceed to its final aggregate gate only if the other independent remediation audit also passes.
