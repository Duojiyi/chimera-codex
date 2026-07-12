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

## Third Remote Red

- Run `29199987420` passed the previous installer mutation and reached the manager library tests.
- `manager_diagnostics_do_not_submit_raw_errors_or_write_logs_in_unit_tests` failed because an LF-only `#[cfg(test)]` split did not cut the CRLF source; the test counted its own assertion as a production diagnostic call (`3` instead of `2`).
- Green normalizes CRLF and CR to LF before source inspection and requires synthetic CRLF and CR inputs to normalize identically. Removing CR-only normalization is a failing mutation.
- The fully qualified targeted test runs 1/1 and passes; formatting and diff checks pass. Independent manager line-ending remediation audits A and B pass.

## Fourth Remote Red

- Run `29200727538` passed `Branding / ads / Rust / frontend` and produced the Windows artifacts successfully.
- Both `macOS DMG (x64)` and `macOS DMG (arm64)` failed in `Rust core unit tests (macOS)` with `E0599`: the macOS-only FD restore branch called `.context(...)` directly on `std::io::Error`.
- Green converts the I/O error to `anyhow::Error` before adding the unchanged operator context. The call order, spawn-error priority and underlying I/O source are preserved.
- The focused regression runs 1/1 and verifies both the outer context and inner error text. The complete core suite, including core unit `157/157`, launcher `66/66`, installer `28/28` and updater `53/53`, passes locally.
- `cargo fmt --all -- --check` and `git diff --check` pass. Independent macOS FD-context remediation audits A and B both pass.

## Replacement Remote Green

- Commit `f23ab828499a25df77035d245577b6656adfbd79` triggered run `29201732498`.
- `Branding / ads / Rust / frontend`: SUCCESS.
- `Windows artifacts`: SUCCESS; Windows zip and installer artifacts uploaded.
- `macOS DMG (x64)`: SUCCESS; the formerly failing macOS core test step passed and DMG/zip artifacts uploaded.
- `macOS DMG (arm64)`: SUCCESS; the formerly failing macOS core test step passed and DMG/zip artifacts uploaded.
- Run conclusion: SUCCESS. All four exact required checks passed at the same PR head SHA.

Step 15.2 has complete Red/Green platform evidence. Its checkbox remains gated only on the final independent Step 15.2 audits.
