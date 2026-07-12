# Task 13 Step 13.1 Evidence

> Status: Red/Green/regression captured; independent A/B pending
> Date: 2026-07-12
> Scope: `minimum_supported_version` manifest parsing, propagation and release workflow contract

## Red

Command:

```text
cargo test -p codex-plus-core --test updater --locked
```

Result: **FAIL as expected** - 41 tests executed, 36 existing tests passed and all 5 new contract tests failed.

Expected failures:

- `latest_json_defaults_missing_minimum_supported_version`: `Release`/`UpdateCheck` serialization did not contain the new nullable field.
- `latest_json_accepts_minimum_supported_version_boundaries`: a valid same-upstream/cross-upstream floor was ignored instead of propagated.
- `latest_json_rejects_foreign_or_invalid_minimum_supported_version`: foreign/invalid floor values were accepted because the field was ignored.
- `latest_json_rejects_minimum_supported_version_above_latest`: a floor above latest was accepted because no ordering validation existed.
- `release_workflow_emits_minimum_supported_version`: the release workflow did not define or emit the field.

No pre-existing updater test failed. This isolates the Red state to the Step 13.1 behavior.

## Green

Implementation:

- Added nullable `minimum_supported_version` to `Release` and `UpdateCheck`.
- Missing fields remain backward compatible as `None`.
- Present values must be strings and valid `X.Y.Z-chimera.N` versions; accepted values are normalized.
- A floor above the manifest latest version is rejected; cross-upstream floors remain valid.
- The release workflow emits `minimum_supported_version`, using repository variable `MINIMUM_SUPPORTED_VERSION` when configured and the current release version otherwise.
- Workflow generation validates the configured floor and rejects a floor above the release version before publishing.

Green command:

```text
cargo test -p codex-plus-core --test updater --locked
```

Result: **PASS - 41/41**.

## Targeted Regression

- `cargo test -p codex-plus-core --lib --locked`: **PASS - 155/155**.
- `cargo test -p codex-plus-manager --lib --locked`: **PASS - 53/53**.
- `cargo test -p codex-plus-manager --test windows_subsystem --locked`: **PASS - 42/42**.
- `cargo test -p codex-plus-core --test installers --locked`: **PASS - 23/23**.
- `cargo fmt --all -- --check`: **PASS**.
- `pwsh -NoProfile -File scripts/generate-branding.ps1 -Check`: **PASS**.
- `git diff --check`: **PASS**; only existing line-ending conversion warnings were emitted.

## Independent Audits

Pending audit A and audit B.
