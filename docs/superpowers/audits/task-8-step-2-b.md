# Task 8 Step 2 Audit B — Build jobs (diff / regression)

> Status: **PASS**
> Date: 2026-07-10
> Scope: Regression vs Task 6 installer paths; lockfile; fail-fast

## Diff / regression review

| Area | Observation | Result |
|---|---|---|
| Task 6 Chimera stage paths retained | Same app names and plist integer/`X.Y.Z` checks | PASS |
| `npm install --package-lock=false` removed | Replaced with `npm ci` | PASS |
| softprops upload-from-build removed | Artifacts only; publish deferred | PASS |
| Checkout ref | Default SHA (push/dispatch), not `github.event.release.tag_name` | PASS |
| Missing setup/zip fails job | Explicit `Test-Path` / `test -f` before upload | PASS |

## Decision

Step 2 regression surface acceptable.
