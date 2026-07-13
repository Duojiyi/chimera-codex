# Installer transaction idempotency hotfix evidence

Date: 2026-07-13

Scope: remediate the `v1.2.35-chimera.2` Windows installer hang after shortcut
creation and the uninstaller metadata-cleanup failure. Target release:
`v1.2.35-chimera.3`.

## Field evidence and root cause

- The installer had already replaced program files, registered uninstall data,
  and created shortcuts when its UI stopped progressing.
- The legacy uninstall-key enumeration handled the NSIS error flag but not the
  empty string returned at end-of-enumeration, allowing an infinite loop.
- The uninstall empty-key macro treated every `DeleteRegKey /ifempty` error as
  fatal. A missing key or a non-empty URL protocol key containing unknown data
  is an expected best-effort outcome, so this caused a false metadata failure.
- The installed desktop `Chimera++.lnk` targets the manager. The Start Menu
  already exposes launcher, manager, and uninstall entries, so no second desktop
  shortcut is required.

## TDD evidence

Before implementation:

```text
RED install_empty_termination=False
RED uninstall_avoids_fatal_branch=False
RED uninstall_clears_error=False
```

After the minimal NSIS change:

```text
GREEN install_empty_termination=True
GREEN uninstall_avoids_fatal_branch=True
GREEN uninstall_clears_error=True
```

The Rust installer contract now requires the empty-string exit in the ordered
enumeration flow and requires `/ifempty` cleanup to avoid the fatal metadata
branch while clearing its error flag. Real legacy whole-key deletion failures,
program-file failures, shortcut failures, and owned registry-value failures
remain fatal.

## Lightweight regression

```text
generate-branding -Check: PASS
verify-no-upstream-ads: OK
cargo fmt --all -- --check: PASS
git diff --check: PASS
Cargo.lock: 4 insertions / 4 deletions, workspace versions only
```

Per user direction, no local Cargo compilation, NSIS build, or platform build
was run. Required GitHub Actions checks and Windows real-machine install and
uninstall tests remain mandatory.
