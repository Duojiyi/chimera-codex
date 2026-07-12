# Task 13 Step 13.1 Numeric Range Remediation Audit A

> Result: **PASS**
> Date: 2026-07-12
> Scope: focused independent review of the release-manifest numeric range remediation
> Independence: this audit did not read, reference, wait for, or communicate with any range remediation audit B record.

## Findings

No blocking findings.

- `scripts/release-manifest.mjs` defines `U64_MAX` as the exact decimal value `18_446_744_073_709_551_615n` and rejects a parsed numeric component before generation, validation or version ordering can accept it.
- The checked Rust dependency is `semver 1.0.28`; its `Version.major`, `minor` and `patch` fields are `u64`, and its parser uses checked `u64` arithmetic. The Node publisher can therefore no longer accept an oversized core component that the Rust updater rejects.
- The regression fixture `0.18446744073709551616.0-chimera.1` is well chosen: its major component `0` orders below latest `1.2.35-chimera.1`, while its minor component is exactly `u64::MAX + 1`. Its rejection proves that the range guard, rather than the existing floor-above-latest check, closes the defect.
- The same `normalizeChimeraVersion` path is used for release version, configured floor and manifest validation, so generation and both release smoke validators inherit the corrected boundary.
- A direct boundary probe accepted `u64::MAX` and rejected `u64::MAX + 1` with the expected range error.

## TDD Evidence

The appended numeric-range section in `task-13-step-13.1-remediation-evidence.md` records a valid isolated Red/Green sequence:

- Red: the below-latest overflow fixture was added first and `node scripts/release-manifest.mjs --self-test` failed with `Missing expected exception`.
- Green: the u64 guard was added, after which the same self-test passed; the recorded updater regression passed 42/42 and the targeted diff check passed.

## Commands

```text
node scripts/release-manifest.mjs --self-test
PASS: release-manifest self-test: PASS

node --check scripts/release-manifest.mjs
PASS

node --input-type=module -e <u64 boundary probe>
PASS: u64::MAX accepted; u64::MAX + 1 rejected with "outside u64 range"

cargo test -p codex-plus-core --test updater --locked chimera_semver_rejects_illegal_and_foreign_channels -- --exact
PASS: 1 passed; 0 failed

git diff --check -- scripts/release-manifest.mjs docs/superpowers/audits/task-13-step-13.1-remediation-evidence.md
PASS
```

Static inspection of the locally resolved `semver-1.0.28` source confirmed `major`, `minor` and `patch` are `u64` and parsing fails on checked-arithmetic overflow.

## Residual Risk

- The publisher also applies the u64 cap to the numeric Chimera revision. Rust stores prerelease identifiers textually and can represent a larger revision, so the publisher is intentionally stricter in that direction. This cannot create an emitted manifest that Rust rejects and does not reopen the reported failure mode.
- The real GitHub Actions publication path was not executed in this focused local review; it remains covered by the Release gate and later online smoke testing.

## Decision

**PASS.** The numeric-range defect is closed with an isolated Red fixture, an exact fail-closed boundary and passing focused regressions.
