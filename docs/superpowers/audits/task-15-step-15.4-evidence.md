# Task 15 Step 15.4 Evidence - First Public Release

> Date: 2026-07-13
> Target: `v1.2.34-chimera.1`

## Red

- `gh release view v1.2.34-chimera.1 --repo Duojiyi/chimera-codex` returned `release not found`.
- `GET /repos/Duojiyi/chimera-codex/git/ref/tags/v1.2.34-chimera.1` returned HTTP 404.
- `GET /repos/Duojiyi/chimera-codex/releases/latest` returned HTTP 404, so no public `latest.json` or installer asset can exist yet.
- PR #1 remains open at `d3c4552145aeb3156c4d29e3bb85f71c1acbd9c3`; its final required checks are still running.
- The protected `public-release` environment exists, requires reviewer `Duojiyi`, allows the single operator to approve, and accepts protected branches only.

## Pending Green

Do not create the tag or Release until PR #1 passes all four App-bound required checks and merges normally into protected `main`. The release workflow must build first, pause only the publish job for `public-release` approval, publish the immutable tag, and expose all expected Windows/macOS/source/manifest assets anonymously. Checksums, sizes, tag target and manifest URLs must be verified before Step 15.4 can pass A/B audit.

## First Release Run Red

- PR #1 merged normally as `2acdf8999f16b436b81d2f6939c86122428d5e25` after all four required checks passed.
- Release run `29204323955` resolved `1.2.34-chimera.1` and passed branding, no-promo, license, manifest, icon and TypeScript gates.
- The gate then failed before platform builds: `cargo test --workspace --locked` compiled the Tauri manager, whose `generate_context!` macro rejected missing `apps/codex-plus-manager/dist` (`frontendDist` is `../dist`).
- Windows, both macOS builds and publish were skipped. No tag, draft or published Release was created.

## Release Gate Remediation

- Red: `release_gate_builds_frontend_before_rust_tests` failed 0/1 because the release gate had no active frontend build step.
- Green: the release gate now runs `npm run vite:build` after TypeScript check and before Rust formatting/tests. It does not invoke the full Tauri `npm run build` packaging command.
- The focused contract passes 1/1, installer workflow tests pass 29/29, the Vite production build succeeds, and formatting/diff checks pass.
- Independent remediation audits A and B both pass after the step-boundary and mutation-test remediation. Hosted CI Green remains required before the Release workflow is retried.

## Final Green

- PR #3 merged normally after required checks as
  `28e46af1bffaba01b391dae244a29b8b702cd3ec`.
- Release run `29210400288` completed successfully for Windows x64, macOS x64 and
  macOS arm64, then published through the protected `public-release` environment.
- Tag and Release `v1.2.34-chimera.1` both resolve to the merge SHA above; the Release
  is neither draft nor prerelease.
- The Release contains exactly the expected eight assets: Windows setup/zip, two
  macOS DMGs, two macOS zips, corresponding source tarball and `latest.json`.
- Every asset was fetched through its anonymous public download URL. GitHub size and
  digest, downloaded size and locally calculated SHA-256 matched for all eight assets.
- `latest.json` declares version and minimum supported version
  `v1.2.34-chimera.1`, and its six platform download entries match the published
  asset URLs, sizes and SHA-256 values.
- The reviewer required only for the first publish was removed after verification.
  The environment remains restricted to protected branches, while `main` still
  enforces the four App-bound checks, admin enforcement, no force pushes and no
  deletions.

Task 16 Windows/macOS installation and upgrade smoke testing is intentionally not
claimed by this evidence.
