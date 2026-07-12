# Task 13 Aggregate Audit B - T32

> Status: **FAIL**
> Date: 2026-07-12
> Independence: final-diff audit; did not read or reference aggregate audit A
> Scope: trusted floor monotonicity, offline behavior, update-entry bypasses, concurrent update execution, Windows installer process semantics, macOS mount/confirmation semantics, and regression surface

## Decision

Task 13 cannot pass its aggregate gate yet. The isolated floor/cache and static platform contracts pass, but the composed launcher/Manager/install flow has four unclosed findings. Two allow the mandatory-update invariant to be bypassed or raced, and two can turn repeated startup requests or post-spawn installer failure into repeated installer launches instead of the specified supported-version continuation.

## Findings

### B1 - Critical: the optional-update bypass is a forgeable environment switch and is not one-time or backend-bound

- The Manager checks the cached floor in `launch_after_optional_update_failure`, but then communicates approval only by setting `CHIMERA_SKIP_UPDATE_ONCE=1` on the launcher child (`apps/codex-plus-manager/src-tauri/src/commands.rs:463-486`, `:534-541`).
- The launcher treats that environment value as sufficient to skip `startup_update_status` entirely (`apps/codex-plus-launcher/src/main.rs:207-217`). It does not verify who issued it, consume a one-time token, bind it to the current version/floor, or re-read the cached floor.
- Any persistent user/process environment containing this value bypasses an already cached mandatory floor. There is also a real TOCTOU window: Manager can verify floor F1 as supported, another process can raise the cache to F2, and the child still skips F2 because approval is represented only by the environment bit.
- This contradicts the design requirement that offline startup cannot bypass a known mandatory floor and the aggregate claim that the bypass is backend-verified and one-time.

Required closure: replace the public Boolean environment switch with a launcher-verifiable, single-use authorization bound to the exact current version and observed trusted floor, or make the launcher re-evaluate the latest cached floor before honoring the continuation. Add negative tests for a forged/persistent environment value and a floor increase between Manager approval and launcher evaluation.

### B2 - High: `perform_update` can install a release below a concurrently raised trusted floor

- Manager loads the floor, fetches/binds the requested release, and validates the release against that earlier snapshot (`apps/codex-plus-manager/src-tauri/src/commands.rs:1821-1867`).
- It then records the manifest floor and may receive a higher floor written by another process, but only uses that effective value to compute `mandatory_update`; it does not re-run `validate_release_against_trusted_floor` (`:1880-1903`). If the manifest has no floor, it does not reload the store at all.
- Concrete interleaving: read F1; fetch release L where `L >= F1`; another process records F2 where `F2 > L`; validation against F1 passes; `record_trusted_floor` returns F2; the code still downloads and launches L. The same race remains during the potentially long download before installer launch.
- The cache writer itself is monotonic, but the install authorization is not linearized with that cache, so the rollback/downgrade invariant is incomplete at the composed-flow level.

Required closure: revalidate the selected release against the latest locked floor at the install-launch boundary, with a defined linearization point, and test a deterministic concurrent floor raise both before download and immediately before launch.

### B3 - High: process creation is treated as installation success, so post-spawn failure can create an ordinary-update launch loop

- Windows `launch_installer` returns success immediately after `Command::spawn()` and discards the child handle (`crates/codex-plus-core/src/update.rs:1616-1630`). Manager then reports success and exits solely from `exit_current_process` (`apps/codex-plus-manager/src/App.tsx:1370-1387`).
- The NSIS script can subsequently abort on stale backup, transaction rollback, metadata failure, or rollback failure (`scripts/installer/windows/CodexPlusPlus.nsi:247-253`, `:454-594`), but no exit status/result marker reaches Manager and no supported-version continuation is authorized.
- After such a post-spawn failure, the old version may correctly remain intact, but its next desktop launch sees the same ordinary update and routes back to Manager to retry. Thus preservation of old files alone does not satisfy “ordinary install failure allows a supported version to continue”.
- macOS has the same semantic gap: successful `hdiutil` process creation is reported as “DMG opened” without observing an immediate non-zero exit (`crates/codex-plus-core/src/update.rs:1652-1671`). Cancellation or mount failure after spawn cannot be distinguished from a usable confirmation flow.

Required closure: define and implement a post-spawn completion protocol. For Windows this can be an installer-owned success/failure marker plus relaunch/one-time supported continuation on failure; for macOS, observe the mount command result before claiming the confirmation surface is ready. Cover post-spawn non-zero exit, NSIS rollback, DMG mount failure, and user cancellation separately from spawn failure.

### B4 - Medium: update execution has no single-flight guard and duplicate routes can launch the installer more than once

- The Manager route state intentionally preserves every pending route and emits every `update` request (`apps/codex-plus-manager/src-tauri/src/lib.rs:28-49`, `:388-435`).
- Each event starts `updateAutomatically` (`apps/codex-plus-manager/src/App.tsx:1846-1858`), while `performUpdate` uses React state as its only guard (`:1348-1351`). The listener effect has an empty dependency list, so its callback retains the initial closure; two events before a render can both observe inactive state.
- The backend `perform_update` command has no process-wide single-flight lock (`apps/codex-plus-manager/src-tauri/src/commands.rs:1810-1903`). The download-directory lock serializes file publication, but it is released before installer launch, allowing concurrent calls to reuse the verified asset and both spawn it.
- This is reachable through rapid duplicate desktop starts/forwarded update routes, not only a hypothetical hostile caller. On Windows the NSIS mutex may reject the second process, but that is already too late and feeds B3; on macOS it can open the DMG more than once.

Required closure: add a backend-owned update-operation single-flight guard covering check/download/launch and coalesce duplicate `update` routes while one operation is active. Test two concurrent `perform_update` requests and multiple forwarded update events.

## Verified Behavior

- Missing cache and corrupt quarantined cache fail open as specified; a valid cached floor below which the current version falls produces `Mandatory` offline.
- Floor writers use a shared file lock and preserve the maximum floor in the existing concurrency test.
- Manifest rollback below a static cached floor is hidden from installation, and release/request version binding rejects same-version, downgrade, and changed-version requests.
- Native assets remain constrained by platform/architecture, versioned branded URL, exact name, size, SHA-256 and launch-time file identity.
- Windows policy includes `/S`; macOS policy keeps Manager alive and labels user confirmation as required. Real Windows replacement/SmartScreen and macOS x64/arm64 Gatekeeper smoke remain correctly deferred to Tasks 15/16, but those deferred tests do not close the process-semantic defects above.

## Commands

```text
cargo test -p codex-plus-core --test updater --locked concurrent_trusted_floor_writers_cannot_roll_back_the_cache -- --exact
PASS - 1/1

cargo test -p codex-plus-core --test updater --locked update_decision_covers_none_automatic_mandatory_and_offline_paths -- --exact
PASS - 1/1

cargo test -p codex-plus-manager --test windows_subsystem --locked manager_update_install_keeps_visible_progress_bar -- --exact
PASS - 1/1

git diff --check -- apps/codex-plus-launcher/src/main.rs apps/codex-plus-manager/src-tauri/src/commands.rs apps/codex-plus-manager/src-tauri/src/lib.rs apps/codex-plus-manager/src/App.tsx crates/codex-plus-core/src/update.rs crates/codex-plus-core/tests/updater.rs scripts/installer/windows/CodexPlusPlus.nsi scripts/installer/macos/package-dmg.sh .github/workflows/release-assets.yml
PASS - no whitespace errors; only existing line-ending conversion warnings
```

## Residual Risk

- The cached floor is explicitly not designed to resist an attacker with local file-write access; this audit does not treat deletion/editing of `update-state.json` as a separate defect.
- `startup_update_status` currently fails open on state-lock/read errors as well as on corrupt/missing state. The documented corrupt-cache behavior is covered, but permission/lock I/O fault behavior should be made explicit in the final threat model.
- Actual unsigned Windows/macOS installer UX and replacement behavior still require the declared real-platform smoke gates after the four composed-flow findings are closed.

## Gate

**FAIL.** Do not check T32 or mark the Task 13 aggregate record PASS until B1-B4 have Red/Green regression evidence and fresh independent aggregate A/B audits both pass.
