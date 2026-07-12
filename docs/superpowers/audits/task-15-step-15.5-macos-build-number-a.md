# Task 15 / Step 15.5 macOS build number remediation - Audit A

## Scope

Requirement and observable-behavior review after the first public release made the
previous `macos_build_number = 1` part of release history.

## TDD evidence

- Red: after fetching `v1.2.34-chimera.1`, `scripts/generate-branding.ps1 -Check`
  failed because current value 1 was not greater than released value 1.
- Green: incremented the single source of truth to 2 and regenerated Rust/TypeScript
  constants; the same check passed.
- Regression: branding 3/3 and Windows/workflow contracts 44/44 passed.

## Independent conclusion

PASS. The released tag points to `28e46af1bffaba01b391dae244a29b8b702cd3ec`
and contains build number 1; the candidate contains 2. Existing-release resolution
keeps `should_publish=false`, so this maintenance PR cannot overwrite the baseline
release. No remediation remains beyond hosted required checks.
