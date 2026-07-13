# Windows legacy cleanup idempotency hotfix evidence

Date: 2026-07-13

Scope: fix the `v1.2.35-chimera.1` installer false failure when the legacy
`Codex++` uninstall registry key is already absent, and advance the release to
`1.2.35-chimera.2`.

## TDD evidence

- Red baseline: the ordered cleanup contract rejects the `HEAD` NSIS script
  because it has no `install_legacy_cleanup_probe` branch.
- Green: the current script initializes the enumeration index, scans the HKCU
  uninstall subkeys, treats end-of-enumeration as an idempotent success, deletes
  the legacy key only when found, preserves the fatal deletion-error branch,
  and jumps to `install_complete` after successful deletion.
- Regression contract: `crates/codex-plus-core/tests/installers.rs` locks the
  ordered initialization, enumeration, match, increment, loop, delete, failure,
  and successful-completion steps.

Lightweight local checks:

```text
red_baseline_rejects_old=True
green_current_contract=True
generate-branding -Check: PASS (PowerShell 7.6.3)
cargo fmt --all -- --check: PASS
git diff --check: PASS
Cargo.lock: 4 insertions / 4 deletions, workspace versions only
```

Windows PowerShell 5.1 reports a false generated-file drift because it decodes
the no-BOM UTF-8 generator differently. The repository workflows use `pwsh`,
and PowerShell 7 is the release-gate baseline.

Per user direction, no local Cargo compilation, NSIS build, or platform build
was run. GitHub Actions must provide the complete Rust, frontend, Windows x64,
macOS x64, and macOS arm64 build evidence. This record does not claim Task 16
real-machine install or upgrade acceptance.

## First cloud Red and remediation

PR #9 run `29230762055` passed branding generation but failed the fail-closed
upstream-ad scanner with 16 findings: eight current legacy shortcut references
were not matched and the corresponding eight old allowlist entries were unused.
The NSIS probe added ten lines before those references.

The remediation changes only the eight affected `lineNumber` values by `+10`;
the paths, patterns, exact source lines, and compatibility-only reasons remain
unchanged. The PowerShell 7 scanner then returned:

```text
verify-no-upstream-ads: OK
```

Independent remediation audits A and B both pass. The complete cloud build must
still be rerun on the new commit.
