# Task 15 Step 15.2 Installer Fixture Remediation Audit A

> Status: **PASS**
> Date: 2026-07-13
> Auditor: independent audit A (requirements and observable behavior)
> Independence: reviewed the failing CI run, final mutation fixture and target parser; did not read or reference the corresponding audit B record

## Decision

The remediation removes the fixture's dependency on checkout line endings without weakening the fail-closed installer contract. The mutation now replaces the first exact single line `  IfErrors uninstall_failed`, which is the target `UninstallFile` macro's error jump, while the contract parser continues to use `lines()`, trim active statements and exclude semicolon comments. No blocking finding remains.

## Red Evidence

- GitHub Actions run `29199419608` failed only the installer contract `windows_legal_files_share_the_binary_transaction_and_uninstall_mutex` in the Windows gate.
- The panic was `assertion left != right failed: mutation fixture must change macro`.
- The logged NSIS source used CRLF. The former LF-delimited multi-line replacement therefore left the fixture byte-for-byte unchanged and never exercised the intended negative behavior.

## Behavior Review

| Requirement | Evidence | Result |
|---|---|---|
| Mutation is line-ending independent | `replacen` matches one NSIS statement and contains no newline token | PASS |
| Correct macro is targeted | The first exact `  IfErrors uninstall_failed` occurs inside `!macro UninstallFile PATH SLOT`; earlier error lines use different labels | PASS |
| Mutation is observable | `assert_ne!(commented_error_jump, nsi)` proves the fixture changed | PASS |
| Commented jump fails closed | Parser extracts only the target macro, uses `lines()`, trims lines, excludes `;` comments and requires the error jump after delete | PASS |
| Production behavior is unchanged | Only the Rust mutation fixture changed; the NSIS implementation was not relaxed | PASS |

## Verification

| Command or evidence | Result |
|---|---|
| `gh run view 29199419608 --repo Duojiyi/chimera-codex --log-failed` | Confirmed CRLF Red and unchanged mutation fixture |
| `cargo test -p codex-plus-core --test installers --locked windows_legal_files_share_the_binary_transaction_and_uninstall_mutex -- --exact` | PASS, 1/1 |
| `cargo fmt --all -- --check` | PASS |
| targeted `git diff --check` | PASS; line-ending warning only |

## Gate

**PASS.** Audit A approves the installer fixture remediation. The fix may proceed after independent audit B also passes; the patched Windows CI rerun remains the remote confirmation gate.
