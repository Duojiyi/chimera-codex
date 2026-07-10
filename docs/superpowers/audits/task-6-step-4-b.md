# Task 6 Step 4 — Audit B (Diff / Boundary)

> Status: **PASS**
> Date: 2026-07-10
> Auditor: B (diff / boundary / regression)
> Scope: macOS version fields, detection roots, CI path sync

## Evidence

| Check | Result |
|-------|--------|
| `CFBundleShortVersionString` rejects `-chimera` suffix in verify_app | enforced |
| `CFBundleVersion` requires positive integer from product.toml | enforced |
| `detect_legacy_apps` never deletes | only reports paths + message |
| Overview searches `/Applications`, sibling of current app, `~/Applications` | wired |
| `open_applications_folder` macOS-only | non-macOS returns failed |
| CI no longer asserts `Codex++.app` | release-assets + pr-build updated |

## Findings

- Uninstall of macOS bundles only removes Chimera-named apps (legacy left for user), matching “do not auto-delete” policy.
- Zip asset names in release-assets still CodexPlusPlus-* (Task 8 rename scope).

## Open issues

- None blocking Task 6; zip rename deferred to Task 8 by plan.
