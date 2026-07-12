# Task 15 Step 15.4 Release Source Unicode - Audit A

> Date: 2026-07-13
> Scope: real Release run Red, requirements, focused test, and observable behavior only
> Conclusion: PASS

## Requirement

The source-tree equality checks must compare the same path representation for tracked Chinese and other Unicode filenames. The fix must cover both the pre-publish source archive check and the idempotent verification of an already published source archive, while preserving fail-closed detection of missing, extra, or changed source content.

## Real Release Red

GitHub Release run `29206388036` at commit `00df56089d9f498f7f25045a680884175d7772aa` completed with failure.

- The release gate, Windows build, macOS x64 build, and macOS arm64 build all passed.
- `Draft -> upload -> publish` failed in `Create draft, upload assets, publish`, before draft creation and upload.
- The log's `diff -u` showed the same eight Chinese paths twice: the expected `git ls-tree` side used quoted octal escapes such as `\350\201...`, while the actual `tar -tzf` side used raw UTF-8 names such as `docs/plans/三阶段计划.md`.
- No non-Unicode path or archive-content difference was reported. The process exited 1 at the source-tree comparison.

This is a path-rendering false positive in the equality gate, not a missing source file or platform artifact failure.

## Final Diff

The implementation changes exactly the two expected-tree listings:

1. `Verify existing corresponding source`, which validates the downloaded source archive for an already published Release.
2. `Create draft, upload assets, publish`, which validates the locally generated source archive before any draft is created or asset is uploaded.

Both now use:

```text
git -c core.quotePath=false ls-tree -r --name-only
```

No archive creation, prefix stripping, sorting, release state, asset, tag, approval, or publish behavior changes.

## Observable Behavior And Fail-Closed Check

At the failing commit, the default `git ls-tree -r --name-only` emits all eight tracked Chinese paths as quoted octal escapes. Running the same listing with `-c core.quotePath=false` emits their raw UTF-8 filenames, matching the representation emitted by `tar -tzf` in the hosted failure log.

The integrity gates remain fail-closed:

- Both jobs retain `set -euo pipefail`.
- Both retain `LC_ALL=C sort` on the expected and actual lists and an unconditional `diff -u`; a missing or extra path still exits nonzero.
- The pre-publish path still checks required source entries after whole-tree equality.
- The published path still performs byte-for-byte `cmp` between a regenerated deterministic archive and the downloaded asset before whole-tree equality.
- Only quoting of bytes at or above `0x80` is disabled. The compared path set is not filtered or reduced.

The focused contract requires exactly two Unicode-safe listings and rejects any remaining `git ls-tree -r --name-only` without the configuration. Reverting either production occurrence makes the contract fail by reducing the safe count and reintroducing the unsafe command.

## Verification

| Check | Result |
| --- | --- |
| `cargo test -p codex-plus-core --test installers release_source_tree_checks_preserve_unicode_paths -- --exact` | PASS, 1/1 |
| `cargo test -p codex-plus-core --test installers` | PASS, 30/30 |
| Default listing of Unicode paths at `HEAD` | Reproduces quoted octal paths from the hosted Red |
| Listing with `core.quotePath=false` at `HEAD` | Emits raw Chinese paths |
| `cargo fmt --all -- --check` | PASS, exit 0 |
| `git diff --check` | PASS, exit 0; line-ending conversion warnings only |

## Audit A Conclusion

No blocking requirement, test, or observable-behavior finding remains. The two-line implementation covers both source archive verification paths, removes only the Unicode representation mismatch, and preserves stronger fail-closed archive and tree checks. Audit A passes.

A hosted rerun is still required to supply the real CI Green and continue the protected first-release process; that operational evidence is not a defect in this candidate diff.

## Second Independent Audit A - NUL-Safe Remediation

> Date: 2026-07-13
> Final conclusion: PASS; this section supersedes the earlier two-line `core.quotePath=false` assessment.

### Updated Requirement Check

The real run `29206388036` demonstrated a false mismatch between Git's quoted path listing and tar's raw UTF-8 listing. A complete repair must also avoid treating a valid newline inside a Git pathname as a record separator. The final candidate now uses NUL-delimited records end to end in both affected jobs.

For both `verify-published-release` and `publish-release`, the workflow now:

1. Extracts the trusted source archive into a dedicated temporary root.
2. Produces the expected list with `git ls-tree -rz --name-only` and `LC_ALL=C sort -z`.
3. Produces the actual extracted list with `find ... -print0`, `sed -z` to remove only the leading `./`, and `LC_ALL=C sort -z`.
4. Compares the two NUL-delimited byte streams with `cmp`.

This removes Git quoting differences for Unicode paths and preserves spaces, tabs, and embedded newlines without converting any pathname into a text line.

### Fail-Closed Semantics

- Both scripts remain under `set -euo pipefail`; failed extraction, `cd`, enumeration, sorting, or comparison terminates the job.
- Missing and extra non-directory entries still change the sorted NUL stream and make `cmp` fail.
- Symlinks remain entries because `find` does not follow them by default and `! -type d` includes the symlink itself.
- The published verification still performs the stronger byte-for-byte archive `cmp` before extraction, so an untrusted or altered published archive cannot reach the tree comparison as an accepted equivalent.
- The pre-publish archive is still produced directly by `git archive`, and its existing required-entry checks remain after whole-tree equality.
- No release state, target SHA, asset-set, approval, tag, or publish command is relaxed.

### Mutation Review

The focused contract scopes its checks independently to `verify-published-release` and `publish-release`. Each job must contain exactly one active NUL Git listing, one active `find -print0`, one active `sed -z`, at least two `sort -z` occurrences, and its exact `.z` stream `cmp`.

- Replacing the first `git ls-tree -rz` with non-NUL `git ls-tree -r` makes the predicate false.
- Commenting the first required `find -print0` makes the predicate false because commented lines are removed before matching.
- Appended commented command decoys do not affect the predicate, demonstrating that comments cannot count as active safeguards.
- The same all-jobs predicate is applied to both job slices, so a missing required stage in either production path fails the contract.

### Second-Round Verification

| Check | Result |
| --- | --- |
| `cargo test -p codex-plus-core --test installers release_source_tree_checks_preserve_unicode_paths -- --exact` | PASS, 1/1 |
| `cargo test -p codex-plus-core --test installers` | PASS, 30/30 |
| `cargo fmt --all -- --check` | PASS, exit 0 |
| `git diff --check` | PASS, exit 0; line-ending conversion warnings only |

No requirement, fail-closed, mutation, or observable release-semantics issue remains in the final candidate. **Second-round independent Audit A: PASS.** A hosted rerun remains necessary for real CI Green before the first release proceeds.
