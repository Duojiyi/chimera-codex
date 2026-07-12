# Task 13 Step 13.1 Remediation Audit A

> Result: **PASS**
> Date: 2026-07-12
> Scope: requirements/behavior re-audit of the A1/A2 remediation
> Independence: this audit did not read, reference, wait for, or communicate with remediation audit B.

## Findings

No blocking findings.

The remediation closes both original audit A findings:

- Manager's `UpdateResult` TypeScript contract declares `minimumSupportedVersion?: string | null`.
- Manager check-update success and failure payloads expose the trusted floor or explicit `null`; install success and failure payloads preserve the floor from the freshly fetched trusted `Release`. A failed trusted re-fetch cannot supply a floor and returns explicit `null`.
- `scripts/release-manifest.mjs` is now the executable source for Chimera manifest floor normalization, generation and validation. Generation defaults an unset/empty repository variable to latest, accepts same-version and lower cross-upstream floors, and rejects foreign channels, malformed versions and a floor above latest.
- Manifest validation fails closed for a missing or non-string floor, rejects invalid/foreign versions and a floor above latest, and can require an exact expected floor.
- The helper self-test exercises default/latest, cross-upstream, foreign, invalid, above-latest, missing and non-string cases against the actual exported generator/validator rather than workflow source text alone.
- The existing-Release anonymous smoke calls `--validate-floor` and verifies presence, type, channel and ordering without assuming that today's repository variable equals a historical release floor. The newly published Release smoke supplies the configured floor (defaulting to the release version) and therefore also verifies exact equality.
- Both PR and Release gates run `node scripts/release-manifest.mjs --self-test`; the publish path reruns the self-test and invokes `--generate`, so CI and publication use the executable contract under test.
- Rust parsing remains backward-compatible when the field is absent, rejects a present non-string value, validates the Chimera channel and ordering, normalizes accepted values, and propagates the field through core `Release`/`UpdateCheck` into Manager payloads.

## Commands

```text
node scripts/release-manifest.mjs --self-test
PASS: release-manifest self-test: PASS

node --check scripts/release-manifest.mjs
PASS

cargo test -p codex-plus-core --test updater --locked
PASS: 42 passed; 0 failed

cargo test -p codex-plus-core --test updater --locked release_workflow_emits_minimum_supported_version -- --exact
PASS: 1 passed; 0 failed

cargo test -p codex-plus-manager --test windows_subsystem --locked manager_update_install_keeps_visible_progress_bar -- --exact
PASS: 1 passed; 0 failed

npm run check
PASS: tsc --noEmit -p tsconfig.json

git diff --check -- scripts/release-manifest.mjs .github/workflows/pr-build.yml .github/workflows/release-assets.yml crates/codex-plus-core/src/update.rs crates/codex-plus-core/tests/updater.rs apps/codex-plus-manager/src/App.tsx apps/codex-plus-manager/src-tauri/src/commands.rs apps/codex-plus-manager/src-tauri/tests/windows_subsystem.rs
PASS; only existing LF-to-CRLF working-copy warnings were emitted.
```

Read-only source inspection also confirmed both `validate_latest_manifest` smoke implementations call the helper, the new-release smoke passes the expected repository-variable floor, and the PR/Release workflows invoke the helper self-test.

## TDD Evidence Review

`task-13-step-13.1-remediation-evidence.md` records the intended Red states independently for the missing Manager type/payload contract, missing executable manifest helper, missing workflow generator/validator wiring and missing PR self-test gate. The Green commands exercise those same behaviors, and the recorded broader regression set covers updater, Manager backend/static contracts, installers/workflows, TypeScript, Vite, branding, advertising scanner, formatting and diff hygiene. The evidence therefore supplies a traceable Red -> Green -> regression chain for both A1 and A2.

## Residual Risks

- GitHub Actions and an actual public GitHub Release were not executed during this local audit. YAML expression evaluation, repository-variable delivery, anonymous GitHub propagation timing and hosted-runner behavior remain release-gate risks.
- Existing Release smoke deliberately validates a historical floor structurally and by ordering rather than against the current repository variable; this avoids rejecting an older release after the variable changes, but cannot prove the historical operator's intended floor.
- The Manager payload assertions are static production-boundary contracts. The targeted Rust test and TypeScript compiler cover drift, but there is no full Tauri IPC end-to-end test in this step.
- Trusted-floor caching, monotonicity and force-update decisions belong to Steps 13.2/13.3 and are outside this audit's closure decision.

## Decision

**PASS.** The remediation supplies the missing Manager propagation and executable release-manifest generation/validation coverage. Step 13.1 may proceed to its independent remediation B gate; it should be checked only if both remediation audits pass.
