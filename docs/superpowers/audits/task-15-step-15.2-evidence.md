# Task 15 Step 15.2 Evidence - First Remote CI

> Date: 2026-07-12
> Target: PR #1, `codex/docs-initialization` to `main`

## Remote Red

- Commit `f92178cd50816ee83a18ae395e803aae15249e19` was pushed to `origin` and PR #1 was marked ready.
- GitHub Actions run `29198619992` resolved version `1.2.34-chimera.1` successfully, then failed the Windows gates job before platform builds.
- Failure: `verify-license.ps1` hashed physical checkout bytes. The repository LF hash was `8486A10C...`, while the Windows runner CRLF checkout produced `6F1E622C...`.

## Remediation Green

- LICENSE hash input now canonicalizes LF, CRLF and CR to LF and treats one leading UTF-8 BOM as an encoding marker.
- Content changes, embedded/repeated BOM, disabled hashing, removed CR normalization and culture-sensitive BOM matching are fail-closed self-test mutations.
- Local normal gate, self-test and diff check pass.
- Independent remediation audit A: PASS.
- Independent remediation audit B: PASS AFTER REMEDIATION.

## Pending Remote Gate

The remediation must be committed and pushed. Step 15.2 remains open until the replacement PR run passes gates, Windows artifacts and macOS x64/arm64 builds at the pushed commit.

## Second Remote Red

- Replacement run `29199419608` passed license, branding, ads, icon, frontend and format gates.
- Workspace Rust then failed `windows_legal_files_share_the_binary_transaction_and_uninstall_mutex`: its negative mutation used an LF-only multi-line replacement, so the Windows CRLF checkout was unchanged.
- Green replaces only the first exact `IfErrors uninstall_failed` line. That first occurrence is inside `UninstallFile`; LF and CRLF probes both change exactly one line, make the helper fail, and leave the later uninstall section unchanged.
- Targeted test, formatting and diff gates pass. Independent fixture-remediation audits A and B both pass.
