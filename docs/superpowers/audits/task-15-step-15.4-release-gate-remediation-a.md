# Task 15 Step 15.4 Release Gate Remediation - Audit A

> Date: 2026-07-13
> Scope: requirements, real CI Red, tests, and observable behavior only
> Conclusion: PASS

## Requirement

Release run `29204323955` must not reach workspace Rust tests before the Tauri manager's `frontendDist` exists. The remediation must build only the frontend in the `gates` job after dependency installation and TypeScript validation, before Rust tests, without broadening the first-release workflow or invoking Tauri packaging.

## Independent Red Check

- GitHub run `29204323955` is a completed failure at commit `2acdf8999f16b436b81d2f6939c86122428d5e25`.
- `Resolve version / idempotent gate` passed. In `Branding / ads / Rust / frontend`, branding, no-promo, license, manifest, icon, dependency installation, TypeScript check, and Rust formatting all passed; `Rust tests` alone failed.
- The hosted log reports a proc-macro panic at `tauri::generate_context!()` with: `The frontendDist configuration is set to "../dist" but this path doesn't exist`. The following Windows, macOS, and publish jobs were skipped.
- The failing workflow's `gates` step list had no frontend build between TypeScript check and Rust tests. This independently matches the recorded focused Red (`0/1`) for `release_gate_builds_frontend_before_rust_tests`.

The Red is therefore a real release-gate ordering defect, not a synthetic failure or a platform installer failure.

## Final Diff And Observable Behavior

- `.github/workflows/release-assets.yml` adds one four-line `Build frontend` step in `gates`, immediately after `TypeScript check` and before Rust formatting/tests.
- The step runs `npm run vite:build` from `apps/codex-plus-manager`. That script is `vite build`; it is not the full `npm run build` command, which also performs launcher and Tauri packaging.
- `tauri.conf.json` declares `frontendDist: "../dist"` and the same `npm run vite:build` as its normal `beforeBuildCommand`, so the new gate step creates the exact input rejected by the hosted run.
- `release_gate_builds_frontend_before_rust_tests` is scoped to the `gates` job and requires the named step, exact working directory, exact command, and ordering before `Rust tests`.
- Its fail-closed mutations reject a commented/missing step, `npm run build`, a build placed after Rust tests, and an unrelated decoy command in another step.

No release versioning, installer construction, environment approval, artifact, tag, or publish behavior is changed. The implementation is the minimum workflow change that supplies the missing Tauri compile input; the accompanying contract is proportionate to the regression.

## Verification

| Check | Result |
| --- | --- |
| `cargo test -p codex-plus-core --test installers release_gate_builds_frontend_before_rust_tests -- --exact` | PASS, 1/1 |
| `cargo test -p codex-plus-core --test installers` | PASS, 29/29 |
| `npm run vite:build` in `apps/codex-plus-manager` | PASS, 1608 modules transformed, `dist/index.html` emitted |
| `cargo fmt --all -- --check` | PASS, exit 0 |
| `git diff --check` | PASS, exit 0; only line-ending conversion warnings, no whitespace error |

## Audit A Conclusion

No blocking requirement, test, or observable-behavior finding remains. The remediation is correctly ordered, narrowly scoped, fail-closed under the relevant mutations, and directly addresses the real hosted CI failure. Audit A passes.

The remaining release evidence is operational rather than a defect in this diff: after push, the hosted release gate must produce a real Green before Step 15.4 proceeds to platform builds, protected publish approval, and public Release verification.
