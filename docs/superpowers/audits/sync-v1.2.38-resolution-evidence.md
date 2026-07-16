# Upstream v1.2.38 Resolution Evidence

Date: 2026-07-16

## Scope

- Synced formal upstream `v1.2.38` into Chimera `1.2.38-chimera.1` with macOS build `8`.
- Recorded `v1.2.36` ancestry before the candidate merge so Git compared the actual `v1.2.36..v1.2.38` upstream range.
- Preserved Chimera branding, repository/update source, ChimeraHub defaults, promotion removal, protected workflows, hardened atomic writes, and injected backup paths.
- Adopted upstream GPT-5.6 metadata, VLM support, provider/App state synchronization, local-session pagination, audio transcription proxying, and related tests.

## Conflict Resolution

- Version metadata resolves to `1.2.38-chimera.1`; `brand/product.toml` uses `macos_build_number = 8`.
- Manager package tests use the upstream `src/*.test.ts` glob while keeping Chimera entrypoint tests.
- `commands.rs` combines bounded pagination with explicit home/backup injection and retains scoped process-environment guards.
- `relay_switch.rs` keeps settings/live-file rollback and runs upstream nonfatal App state synchronization only after success.
- `settings.rs` exposes `atomic_write` publicly for new upstream callers while preserving Chimera's unique temp file, sync, identity, and cleanup protections.
- `relay_config.rs` retains live-profile and aggregate matching, adds responses-proxy detection, and creates model catalogs for either user windows or bundled metadata.
- Upstream recommendation copy was excluded; compatibility-only `jojocode.com` fixtures remain covered by exact allowlist entries.
- `.github/workflows` was restored from trusted Chimera HEAD before staging.

## Local Non-Build Evidence

- `scripts/test-sync-upstream.ps1`: PASS.
- `scripts/generate-branding.ps1 -Check`: PASS.
- `scripts/verify-brand-icons.ps1 -SelfTest`: PASS.
- `scripts/verify-no-upstream-ads.ps1`: PASS.
- `scripts/test-verify-allowlist.ps1`: PASS.
- `scripts/verify-license.ps1`: PASS.
- JSON parse for package, lock, Tauri config, and i18n key files: PASS.
- Unmerged path scan, conflict-marker scan, protected-workflow diff, and `git diff --check`: PASS.

No local dependency installation, Cargo/npm build, Rust test, frontend build, or packaging build ran. Rustfmt was not available without rustup downloading the pinned toolchain, and the TypeScript i18n verifier requires uninstalled node modules; both remain cloud gates.

Cloud run `29509860794` passed TypeScript, frontend behavior tests, and the frontend build, then reported one Rustfmt-only import ordering diff in `tests/protocol_proxy.rs`. The candidate was updated to the exact cloud formatter output with no behavior change.

Cloud run `29510126822` passed Rustfmt and reached Rust compilation. It exposed one merged API mismatch: the external-model-catalog branch returned the upstream `String` instead of Chimera's `PreparedModelCatalogConfig`. The branch now wraps the unchanged config text with `catalog: None`, preserving the external pointer without scheduling a generated catalog write.

## Cloud Gates

- Branding / ads / Rust / frontend required check.
- Windows x64 artifacts.
- macOS x64 DMG and zip.
- macOS arm64 DMG and zip.
- Release tag, target SHA, asset set, manifest, and SHA-256 verification.
