# Sync v1.2.38 Automation Audit A

Date: 2026-07-16
Perspective: requirements, regression evidence, and observable automation behavior
Result: PASS

## Evidence

- `origin/main` records workspace baseline `1.2.36-chimera.1` but `git merge-base --is-ancestor v1.2.36 origin/main` is false because PR #14 was squash-merged.
- The RED run failed because `Get-UpstreamBaselineAncestryDisposition` did not exist.
- The GREEN run passed against the real repository graph: `origin/main` requires `stitch`, while upstream `v1.2.38` reports `present` for baseline `v1.2.36`.
- Candidate construction fetches the formal baseline tag, records it with an `ours` merge only when absent, and then merges the newer formal tag.
- Existing ancestry remains a no-op, so repositories that preserve merge parents do not gain redundant commits.

## Conclusion

The change addresses the reason v1.2.37 and v1.2.38 repeatedly stopped at the merge gate. It does not weaken conflict handling, bypass required checks, push to upstream, or publish directly. No open requirements finding remains.

