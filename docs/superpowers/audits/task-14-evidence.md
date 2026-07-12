# Task 14 Evidence - Original Chimera++ Icon

> Date: 2026-07-12
> Scope: concept generation, source provenance, SVG-to-platform export, resource replacement, and small-size review

## Design Red

- Before Task 14, `brand/icon/logo.svg`, `brand/icon/PROVENANCE.md`, and `scripts/verify-brand-icons.ps1` did not exist.
- All six distributed icon files still had the previous hashes. The PNG was only `300 x 300`.
- After adding the fail-closed gate but before replacing assets, `pwsh -NoProfile -File scripts/verify-brand-icons.ps1` failed with 10 findings: missing source/provenance, six legacy hashes, and the undersized PNG.

## Concepts And Selection

- Three self-contained SVG concepts were generated without external images, fonts, paths, gradients, masks, or filters.
- `brand/icon/preview.html` renders every concept on light/dark backgrounds and at 64/32/16 pixels.
- Concept 1 was selected because its C silhouette remained strongest at 16 pixels and its complete C+ reading remained clear from 32 pixels upward.
- Three plus-size refinements were rendered at the actual 16-pixel output. Making the plus large enough to preserve four arms at 16 pixels would merge it with the C, so iteration 1 was retained and the tradeoff was recorded in `brand/icon/PROVENANCE.md`.

## Export Green

- Source: `brand/icon/logo.svg` SHA-256 `0BA434F6D39AD3A55EE3756BF0553FB839B22CA3D2A14A3C18EAF8D4A53DB048`.
- Export tool: repository-pinned Tauri CLI, directly from the final SVG.
- Primary PNG: `1024 x 1024`, transparent outer corners, SHA-256 `F881F5F1FE76449D7C11ACF61581DFA39DB0A0DD1940129ADF3860A9DBBE53E3`.
- Primary ICO: includes 16/32/48/256 pixel entries, SHA-256 `74723AA52B081D84970E3FE5358CA4F1B58F4C3BA1F94AB930BD54E96EC71715`.
- All three PNG destinations are byte-identical; all three ICO destinations are byte-identical.

## Audit Remediation Red / Green

- Audit B first proved that the initial SVG gate accepted active `<script>` / `<style>` elements, event attributes and CSS `url(...)` values, and that binary copies were not machine-bound to the master SVG.
- Red: `cargo test -p codex-plus-core --test installers --locked brand_icon_gate_self_test_is_fail_closed_and_runs_in_ci -- --exact` failed because `verify-brand-icons.ps1` had no `-SelfTest` parameter.
- Green: the gate now uses an `XmlReader` with DTD and external resolution disabled, an SVG element/attribute allowlist, active-value rejection, and safe/unsafe fixtures. Both PR and Release workflows run the self-test and real gate only after `npm ci` installs the locked Tauri CLI.
- Audit B then constructed an ICO whose directory declared valid dimensions while every payload contained only the eight-byte PNG signature; the first payload-range implementation accepted it.
- Red: `pwsh -NoProfile -File scripts/verify-brand-icons.ps1 -SelfTest` failed with `truncated PNG ICO fixture was accepted`.
- Green: every ICO payload is now range-checked, decoded as a PNG, and matched to its directory dimensions. The real gate also re-exports PNG and ICO from `brand/icon/logo.svg` using `node_modules/.bin/tauri.cmd` and requires exact SHA-256 equality with the distributed assets.
- Audit A independently found that workflow command presence did not lock execution order. The Rust contract now asserts `npm ci < icon SelfTest < real icon gate` for both workflows.

## Verification

```text
pwsh -NoProfile -File scripts/verify-brand-icons.ps1
PASS

pwsh -NoProfile -File scripts/verify-brand-icons.ps1 -SelfTest
PASS

cargo test -p codex-plus-core --test installers --locked
PASS, 26/26

pwsh -NoProfile -File scripts/generate-branding.ps1 -Check
PASS

pwsh -NoProfile -File scripts/verify-no-upstream-ads.ps1
PASS

cargo test -p codex-plus-manager --test windows_subsystem --locked launcher_binary_embeds_codex_icon_resource -- --exact
PASS, 1/1

cargo test -p codex-plus-manager --test windows_subsystem --locked manager_main_window_uses_default_window_icon_explicitly -- --exact
PASS, 1/1

git diff --check -- brand/icon scripts/verify-brand-icons.ps1 apps/codex-plus-manager/src-tauri/icons assets/images docs/images
PASS
```

## Deferred Platform Evidence

- The Windows runner must still compile the final ICO into both executables and the NSIS installer.
- The macOS x64/arm64 runners must still generate `.icns` from the final 1024 PNG and build both app bundles.
- Installed shortcut, Finder, Dock, Start Menu and Gatekeeper appearance remain Task 16 real-platform checks.
