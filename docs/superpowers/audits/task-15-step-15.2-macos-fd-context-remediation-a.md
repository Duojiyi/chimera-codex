# Task 15 Step 15.2 macOS FD Context Remediation Audit A

> Status: **PASS**
> Date: 2026-07-13
> Auditor: independent audit A (requirements and observable behavior)
> Independence: reviewed only the requirement, Run 29200727538 state/evidence, final `update.rs` diff and tests; did not read, request or reference audit B

## Decision

The fix resolves the invalid direct `.context()` call on `std::io::Error` while preserving both the operator-facing context and the underlying I/O error chain. It is confined to `crates/codex-plus-core/src/update.rs`: one small cfg-gated adapter, one call-site replacement and one focused unit test. No blocking finding was identified.

## Failure Evidence

- Run `29200727538` completed the shared Windows gates successfully, then both `macOS DMG (x64)` and `macOS DMG (arm64)` failed in `Rust core unit tests (macOS)`.
- The reported compiler failure was `E0599` at the former `update.rs` restore-error branch: `.context(...)` was invoked directly on `std::io::Error`.
- This failure is compile-time and isolated to the macOS `fcntl` FD-flag restoration path; later packaging steps were skipped.

## Behavior Review

| Requirement | Evidence | Result |
|---|---|---|
| Compile-valid context attachment | Helper first constructs `anyhow::Error::new(error)`, then calls `.context(...)` on the anyhow error | PASS |
| Preserve outer context | Unit test requires exact display text `failed to restore installer FD flags` | PASS |
| Preserve underlying source | `anyhow::Error::new(std::io::Error)` retains the concrete source; alternate chain formatting must include `restore denied` | PASS |
| Preserve branch behavior | Only `(Ok(_), Err(error))` delegates to the helper; spawn errors still take precedence and success behavior is unchanged | PASS |
| Limit platform surface | Helper is compiled only for macOS or tests; non-macOS production code gains no callable path | PASS |
| Minimal diff | `update.rs` only, 17 insertions and one replacement; no unrelated refactor or dependency change | PASS |
| Effective regression test | The focused test compiles the formerly invalid context construction and independently checks outer and inner messages | PASS |

## Verification

| Command or evidence | Result |
|---|---|
| `gh run view 29200727538 --repo Duojiyi/chimera-codex --json status,conclusion,headSha,jobs` | Confirmed both macOS matrix jobs failed at Rust core tests; run was still finishing unrelated Windows work when audited |
| `cargo test -p codex-plus-core --lib --locked update::tests::installer_fd_restore_error_preserves_context_and_source -- --exact` | PASS, 1/1; 156 filtered out |
| `cargo fmt --all -- --check` | PASS |
| targeted `git diff --check` | PASS; line-ending warning only |
| `rustup target list --installed` | Apple target unavailable locally; only `x86_64-pc-windows-msvc` installed |

## Residual Platform Gate

The focused helper test validates the exact type conversion and error chain on the host, but this machine cannot compile an Apple target. A patched macOS x64/arm64 CI rerun must provide the final platform compilation evidence. This is an explicit remote confirmation gate, not a missing local implementation or test.

## Gate

**PASS.** Audit A approves the current macOS FD-context compilation remediation. It may proceed only after independent audit B also passes; remote macOS x64/arm64 success remains required before Step 15.2 is complete.
