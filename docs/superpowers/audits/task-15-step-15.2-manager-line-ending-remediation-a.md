# Task 15 Step 15.2 Manager Line-Ending Remediation Audit A

> Status: **PASS**
> Date: 2026-07-13
> Auditor: independent audit A (requirements and observable behavior)
> Independence: reviewed the failing CI run, final source-normalization fixture and full exact test; did not read or reference the corresponding audit B record

## Decision

The remediation makes the manager diagnostics source contract independent of checkout line endings without relaxing its production assertions. The test canonicalizes CRLF and lone CR to LF before locating the test-module boundary, and a synthetic CRLF round trip proves that the canonical source is identical. No blocking finding remains.

## Red Evidence

- GitHub Actions run `29199987420` failed `commands::tests::manager_diagnostics_do_not_submit_raw_errors_or_write_logs_in_unit_tests` on Windows.
- The assertion expected two production `append_diagnostic_log` references but observed three (`left: 3`, `right: 2`).
- The prior LF-only `\n#[cfg(test)]\nmod tests` split did not match the CRLF checkout, so test-module source was incorrectly counted as production.

## Behavior Review

| Requirement | Evidence | Result |
|---|---|---|
| Normalize Windows checkout | Source replaces CRLF with LF, then lone CR with LF before parsing | PASS |
| Preserve source content | A shared normalization closure converts synthetic CRLF and synthetic CR back to the same canonical source | PASS |
| Correctly isolate production | Existing test-module marker is applied only after canonicalization | PASS |
| Preserve diagnostics security checks | Raw `error.to_string()` remains forbidden for all three events | PASS |
| Preserve test-isolated logging contract | Production still must contain exactly two `append_diagnostic_log` references | PASS |
| Production code unchanged | Remediation is confined to the unit-test source fixture | PASS |

## Verification

| Command or evidence | Result |
|---|---|
| `gh run view 29199987420 --repo Duojiyi/chimera-codex --log-failed` | Confirmed Windows Red `3 != 2` |
| `cargo test -p codex-plus-manager --lib --locked commands::tests::manager_diagnostics_do_not_submit_raw_errors_or_write_logs_in_unit_tests -- --exact` | PASS, 1/1; 53 filtered out |
| `cargo fmt --all -- --check` | PASS |
| targeted `git diff --check` | PASS; line-ending warning only |

## Gate

**PASS.** Audit A approves the manager diagnostics line-ending remediation. The fix may proceed after independent audit B also passes; the patched Windows CI run remains the remote confirmation gate.

Final focused recheck: the synthetic line-ending loop now covers both CRLF and CR-only inputs through the exact production normalization closure. The full targeted test remains 1/1 PASS; Audit A remains PASS.
