# Task 13 Step 13.1 Audit Remediation Evidence

> Status: **PASS - remediation and final focused A/B complete**
> Date: 2026-07-12
> Findings: audit A1 Manager payload propagation; audit A2 executable workflow generation and smoke validation

## Red

All three remediation checks failed for the intended missing behavior:

1. `cargo test -p codex-plus-manager --test windows_subsystem --locked manager_update_install_keeps_visible_progress_bar -- --exact`
   - **FAIL as expected**: `App.tsx` did not declare `minimumSupportedVersion?: string | null`; backend assertions were not reached.
2. `cargo test -p codex-plus-core --test updater --locked release_workflow_emits_minimum_supported_version -- --exact`
   - **FAIL as expected**: release workflow did not call the executable manifest self-test/generator/validator contract.
3. `node scripts/release-manifest.mjs --self-test`
   - **FAIL as expected**: the executable manifest helper did not exist.

The original Step 13.1 audit records remain unchanged. A complete independent A/B re-audit is required after remediation.

## Green

- Manager `UpdateResult` now declares `minimumSupportedVersion`; check success/failure and install success/failure payloads preserve the nullable floor.
- `scripts/release-manifest.mjs` is the executable source for manifest generation and floor validation.
- Its self-test executes default/latest, cross-upstream, foreign/invalid, above-latest, missing and non-string floor cases.
- Release generation and both existing/new Release anonymous smoke paths call the same validator.
- PR and Release gates run the executable self-test.

Green results:

- `node scripts/release-manifest.mjs --self-test`: **PASS**.
- Targeted Manager contract: **PASS - 1/1**.
- Targeted updater workflow contract: **PASS - 1/1**.
- PR self-test contract first failed as expected, then passed after the PR gate was connected.

## Regression

- updater: **42/42 PASS**.
- Manager backend lib: **53/53 PASS**.
- Manager Windows/static contracts: **42/42 PASS**. The first full run exposed two stale inline-YAML assertions; they were moved to inspect the executable generator and the full suite then passed.
- installer/workflow contracts: **23/23 PASS**.
- TypeScript: **PASS**.
- Vite production build: **PASS**, 1608 modules transformed.
- `node --check scripts/release-manifest.mjs`: **PASS**.
- `pwsh -NoProfile -File scripts/verify-no-upstream-ads.ps1`: **PASS**.
- `pwsh -NoProfile -File scripts/generate-branding.ps1 -Check`: **PASS**.
- `cargo fmt --all -- --check`: **PASS**.
- `git diff --check`: **PASS**; only existing line-ending conversion warnings were emitted.

The original audit A FAIL and audit B PASS remain immutable historical records. Final Step 13.1 closure requires a fresh independent remediation A/B pair.

## Numeric Range Remediation

The first remediation re-audit produced A=PASS and B=FAIL. Audit B found that the Node generator used unbounded `BigInt` components while Rust `semver::Version` accepts only `u64` components.

Red:

- Added a self-test floor `0.18446744073709551616.0-chimera.1`, which is ordered below latest but contains a component above `u64::MAX`.
- `node scripts/release-manifest.mjs --self-test`: **FAIL as expected** with `Missing expected exception`.

Green:

- `normalizeChimeraVersion` now rejects every component above `18_446_744_073_709_551_615`.
- manifest self-test: **PASS**.
- `node --check scripts/release-manifest.mjs`: **PASS**.
- updater: **42/42 PASS**.
- targeted diff check: **PASS**, with only the existing line-ending warning.

Final focused verification:

- `task-13-step-13.1-range-remediation-a.md`: **PASS**.
- `task-13-step-13.1-range-remediation-b.md`: **PASS**.

Step 13.1 is closed. The earlier FAIL records remain preserved as the audit trail that led to both remediation cycles.
