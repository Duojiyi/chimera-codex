# Sync v1.2.38 Automation Audit B

Date: 2026-07-16
Perspective: implementation diff, failure boundaries, and regression surface
Result: PASS

## Evidence

- `merge-base --is-ancestor` exit 0 maps to `present`, exit 1 maps to `stitch`, and every other exit throws with the original Git diagnostic.
- The stitch uses `git merge --no-ff --no-edit -s ours` with the existing GitHub Actions bot identity helper; the candidate tree is not replaced by the baseline tree.
- The baseline tag is derived from the validated formal workspace baseline and fetched explicitly from the read-only `upstream` remote.
- Source-contract mutations prove that removing the stitch or moving it after the candidate merge fails the PowerShell contract suite.
- `pwsh -NoProfile -File scripts/test-sync-upstream.ps1` and `git diff --check` pass. No Cargo, npm, frontend, Rust, or packaging build ran locally.

## Conclusion

The patch is minimal and ordered correctly. Cloud required checks remain responsible for compilation, tests, and platform packaging. No open boundary or regression finding remains.

## Cloud Red Follow-up

The test fixture no longer assumes upstream tag refs exist in the Chimera origin. It still exercises the real Git graph and fails closed if the baseline release commit cannot be resolved from candidate history. This makes the contract portable to both developer clones and GitHub PR merge checkouts.
