# Task 15 Step 15.2 License Hash Remediation Audit A

> Status: **PASS**
> Date: 2026-07-12
> Auditor: independent audit A (requirements and observable behavior)
> Independence: reviewed the failing CI run, final license gate implementation and self-test behavior; did not read or reference the corresponding audit B record

## Decision

The remediation fixes the Windows checkout and encoding-equivalence false failures without weakening the LICENSE content contract. `verify-license.ps1` removes at most one leading `U+FEFF` using an Ordinal comparison, canonicalizes line endings (`CRLF` and lone `CR` to `LF`), hashes the canonical UTF-8 bytes, and still requires the original LF snapshot SHA-256 `8486A10C4393CEE1C25392769DDD3B2D6C242D6EC7928E1414EFFF7DFB2F07EF`. Both production verification and the fail-closed self-test pass.

## Red Evidence

- GitHub Actions run `29198619992`, Windows job `Branding / ads / Rust / frontend`, failed in the active `pwsh -File scripts/verify-license.ps1` step.
- The sole finding was `LICENSE SHA-256 mismatch: 6F1E622C82A380075843BB084A7EC3B1F1D12A4A02526D75E78B0924A860AA75`.
- An independent in-memory probe reproduced the exact hashes from the same LICENSE text: LF produced `8486...07EF`; CRLF produced `6F1E...AA75`. This isolates the Red failure to checkout line-ending conversion rather than a license-content change.

## Behavior Review

| Requirement | Evidence | Result |
|---|---|---|
| Accept the repository LF snapshot | Production gate and explicit LF self-test fixture pass against `8486...07EF` | PASS |
| Accept Windows CRLF checkout | Self-test converts the snapshot to CRLF and runs the full hash-enabled validation; independent probe reproduces the prior `6F1E...AA75` raw hash | PASS |
| Accept equivalent BOM encoding | A single leading `U+FEFF` is removed before hashing and the leading-BOM fixture passes | PASS |
| Preserve the first content character | `StartsWith` uses `StringComparison.Ordinal`; an independent probe returns false for plain `GNU...` and true only for BOM-prefixed text | PASS |
| Preserve content integrity | Canonicalization changes only one leading BOM and `\r\n`/`\r` to `\n`; spaces, text and ordering are not normalized | PASS |
| Reject non-encoding mutations | Appended content and an embedded `U+FEFF` both produce the required SHA-256 mismatch in SelfTest | PASS |
| Use deterministic bytes | Canonical text is encoded with .NET UTF-8 and hashed with SHA-256 | PASS |
| Production and self-test share logic | Both call `Test-LicenseSnapshot -CheckLicenseHash`; there is no separate CI-only hash path | PASS |
| CI continues to execute both gates | PR and Release workflows run the production license gate and `-SelfTest` | PASS |

## Verification

| Command or probe | Result |
|---|---|
| `gh run view 29198619992 --repo Duojiyi/chimera-codex --log-failed` | Confirmed Windows Red hash `6F1E...AA75` |
| `pwsh -NoProfile -File scripts/verify-license.ps1` | PASS |
| `pwsh -NoProfile -File scripts/verify-license.ps1 -SelfTest` | PASS |
| independent LF/CRLF UTF-8 SHA-256 probe | LF `8486...07EF`; CRLF `6F1E...AA75` |
| independent Ordinal BOM probe | plain first character preserved; BOM-prefixed text detected and stripped once |
| targeted `git diff --check` | PASS; line-ending warning only |

## Residual Scope

The remote CI run predates this local remediation, so a new Windows run must still demonstrate the patched script in GitHub Actions. That is a Step 15.2 remote execution gate, not a missing local behavior or test.

## Gate

**PASS.** Audit A approves the CI license hash remediation. It may be closed after independent audit B also passes and the patched Windows workflow is rerun in the remote verification step.
