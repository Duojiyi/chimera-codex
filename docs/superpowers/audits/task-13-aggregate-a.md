# Task 13 Aggregate Audit A - T32 Gate

> Status: **FAIL**
> Date: 2026-07-12
> Auditor: independent audit A
> Scope: manifest floor, trusted cache, startup routing, Manager automatic/mandatory flow, Windows/macOS install policy, customer-facing terminology

## Decision

Task 13 cannot pass its aggregate gate yet. The focused suites pass and most required behavior is present, but the Manager install path can persist a mandatory floor before proving that the freshly fetched release has a complete native asset. A same-version manifest change can therefore leave an old client blocked by a cached floor with no installable asset. The Node and Rust Chimera version validators also still disagree on accepted boundaries.

## Findings

### A1 - High - Manager can cache a mandatory floor before native-asset validation

Evidence:

- `apps/codex-plus-manager/src-tauri/src/commands.rs:1841-1879` binds the requested version to a newly fetched trusted release, but `validate_update_request` only checks version equality/newness.
- `apps/codex-plus-manager/src-tauri/src/commands.rs:1880-1895` calls `record_trusted_floor` immediately after the fetch.
- The first operation that requires complete native asset metadata is the later `perform_update` call at `apps/codex-plus-manager/src-tauri/src/commands.rs:1903`.
- `crates/codex-plus-core/src/update.rs:644-650` explicitly avoids caching a newly received floor when startup sees no complete native asset, so the Manager path does not preserve the same invariant.

Reproduction of the unsafe ordering:

```powershell
$source = Get-Content -Raw apps/codex-plus-manager/src-tauri/src/commands.rs
$body = ($source -split 'pub async fn perform_update\(version: Option<String>\)', 2)[1] `
  -split '\#\[tauri::command\]\r?\npub fn load_watcher_state', 2 | Select-Object -First 1
$record = $body.IndexOf('update_state_store.record_trusted_floor(floor)')
$install = $body.IndexOf('codex_plus_core::update::perform_update(&release, &download_dir).await')
$assetGuard = $body.IndexOf('release_has_complete_native_asset')
[pscustomobject]@{ RecordFloorOffset=$record; PerformInstallOffset=$install; CompleteNativeAssetGuardOffset=$assetGuard }
```

Observed: `RecordFloorOffset=2587`, `PerformInstallOffset=3463`, `CompleteNativeAssetGuardOffset=-1`.

Observable failure sequence:

1. Startup check sees version `1.2.35-chimera.1`, a floor above the current client, and a valid native asset.
2. Before Manager installs, `latest.json` for the same version is replaced or temporarily published without the current platform/architecture asset.
3. Manager accepts the same requested version, records the higher floor, then fails in `perform_update` because asset metadata is incomplete.
4. The monotonic cache cannot lower the floor. Subsequent offline startup, or online startup while the asset remains missing, is mandatory-blocked without an installer.

Required remediation: validate a complete current-platform asset before `record_trusted_floor`, preferably through one core operation shared by startup and Manager. Add a regression using an isolated `UpdateStateStore` proving a mandatory release without a native asset leaves the prior cache bytes unchanged and still permits a supported client to continue.

### A2 - Medium - Node and Rust version validators still accept different Chimera domains

Evidence:

- `scripts/release-manifest.mjs:20-23` converts all four numeric components, including `chimera.N`, to `BigInt` and rejects any component above `u64::MAX`.
- Rust `semver::Version` stores major/minor/patch as `u64`, but numeric prerelease identifiers are arbitrary-length digit strings. `crates/codex-plus-core/src/update.rs:273-280` only checks that the `chimera.` suffix is non-empty ASCII digits, so Rust accepts `1.2.35-chimera.18446744073709551616` while Node rejects it.
- `crates/codex-plus-core/src/update.rs:264` trims surrounding whitespace and removes every leading `v`/`V`; Node at `scripts/release-manifest.mjs:15-16` removes exactly one prefix and otherwise enforces its anchored grammar. Rust therefore also accepts values such as `vv1.2.35-chimera.1` or whitespace-wrapped tags that Node rejects.
- The executable self-test covers an oversized upstream component, not an oversized Chimera revision, so it does not expose this mismatch.

Impact: release CI and the client do not implement the claimed identical bounds. A repository variable/tag can be rejected by release generation even though the client considers it a valid channel version, while separately a client can accept non-canonical manifest spellings that release automation will never generate.

Required remediation: define one canonical accepted grammar and apply it in both implementations. If `chimera.N` is intended to be `u64`, enforce that in Rust; otherwise limit only the first three Node components. Add the same boundary corpus to Rust tests and the executable Node self-test, including oversized revision, repeated prefix, whitespace, leading zero, foreign channel and build metadata.

## Verified Areas

- `minimum_supported_version` missing/default, same-upstream boundary, cross-upstream floor, foreign/invalid value and floor-above-latest behavior are covered and passing.
- Trusted cache separation, atomic write, cross-process monotonic writes, rollback rejection, corrupt quarantine and offline decision tests pass.
- Launcher checks update state before settings/login routing; automatic and mandatory updates route to Manager.
- Manager exposes automatic startup checking, blocking mandatory retry UI, backend-verified ordinary-failure continuation and Windows exit-after-spawn wiring.
- Windows policy uses `/S`; installer transaction/mutex/rollback contracts pass.
- macOS policy opens a verified DMG with `hdiutil attach -autoopen`, keeps the process running and reports that user confirmation is required.
- The production promotion/branding scanner passes; no customer-facing GitHub/Release/asset/SHA terminology was found outside the explicit background infrastructure allowlist.

## Focused Verification

| Command | Result |
|---|---|
| `cargo test -p codex-plus-core --test updater --locked` | PASS, 50/50 |
| `cargo test -p codex-plus-core --test launcher --locked` | PASS, 66/66 |
| `cargo test -p codex-plus-launcher --locked` | PASS, 8/8 |
| `cargo test -p codex-plus-core --test installers --locked` | PASS, 23/23 |
| targeted Manager update static contract | PASS, 1/1 |
| `node scripts/release-manifest.mjs --self-test` | PASS |
| `pwsh -NoProfile -File scripts/verify-no-upstream-ads.ps1` | PASS |

Passing current tests do not close A1 or A2 because neither unsafe floor-persistence ordering nor the missing cross-language boundary cases are asserted.

## Residual Platform Risk

- Actual Windows silent replacement, installer failure injection, old-version rollback, SmartScreen behavior and process-exit timing still require the Task 16 Windows smoke gate.
- Actual macOS x64/arm64 DMG mount, Finder replacement, cancellation and unsigned Gatekeeper confirmation still require Release CI plus the Task 16 macOS smoke gate.
- Remote Actions and public Release publication remain Task 15 and are not claimed by this audit.

## Gate

**FAIL.** Remediate A1 and A2 with failing tests first, rerun focused/full regressions, then repeat independent aggregate audits A and B. Do not check T32 from this result.
