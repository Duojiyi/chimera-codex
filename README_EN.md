# Chimera Codex

<p align="center">
  <img src="docs/images/codex-plus-plus.png" alt="Chimera Codex icon" width="160">
</p>

<p align="center">
  <a href="README.md">中文</a> | English
</p>

<p align="center">
  <img alt="Release" src="https://img.shields.io/github/v/release/Duojiyi/chimera-codex">
  <img alt="License" src="https://img.shields.io/badge/license-MIT-blue">
  <img alt="Rust" src="https://img.shields.io/badge/rust-1.85%2B-orange">
  <img alt="Tauri" src="https://img.shields.io/badge/tauri-2.x-24C8DB">
</p>

Chimera Codex is an external enhancement launcher and manager for the Codex App (a public distribution based on upstream [CodexPlusPlus](https://github.com/BigPizzaV3/CodexPlusPlus)). It does not modify the original Codex installation. Instead, it starts Codex externally and injects enhancements through the Chromium DevTools Protocol.

This edition defaults to the **ChimeraHub** relay, removes upstream promotional and sponsor content, and ships updates from the public repository [Duojiyi/chimera-codex](https://github.com/Duojiyi/chimera-codex).

## Quick Start

Download the latest installer from [GitHub Releases](https://github.com/Duojiyi/chimera-codex/releases):

- Windows: `ChimeraCodex-*-windows-x64-setup.exe` (zip portable package also available)
- macOS Intel: `ChimeraCodex-*-macos-x64.dmg`
- macOS Apple Silicon: `ChimeraCodex-*-macos-arm64.dmg`

After installation, two entry points are available:

- `Chimera Codex`: a silent launcher. It does not show the manager UI and only starts Codex with injection.
- `Chimera Codex Manager`: a Tauri control panel for launch, diagnostics, repair, updates, relay injection, enhancements, and user scripts.

## First-run setup (ChimeraHub Key-first)

A fresh install automatically creates and selects the **ChimeraHub** relay profile:

| Field | Value |
|---|---|
| Base URL | `https://api.chimerahub.org/v1` (the `/v1` suffix is required) |
| Protocol | Responses |
| Default model | `gpt-5.5` |

You only need to:

1. Open `Chimera Codex Manager`.
2. Enter your API Key on the ChimeraHub setup page.
3. Click **Save and enable**.

An empty Key does not write live config and does not send business requests. **Do not put real keys** in docs, screenshots, or issues; examples always use placeholders such as `sk-...`.

Upgrading an existing install does not overwrite your current relay profiles or active selection.

## Windows in-place upgrade

The Windows installer supports overlaying an existing Codex++ install root (phase one keeps `$LOCALAPPDATA\Programs\Codex++` to reduce migration cost):

1. Quit running `Codex++` / `Chimera Codex` and manager processes.
2. Run `ChimeraCodex-*-windows-x64-setup.exe`.
3. The installer cleans legacy shortcuts and creates only Chimera entry points.
4. Launch from the new desktop / Start Menu shortcuts.

Update checks anonymously read this repository's public `latest.json` and do not fall back to the upstream update feed.

## macOS install, Gatekeeper, and legacy apps

Releases provide separate `macos-x64` and `macos-arm64` DMGs. Current macOS builds use **ad-hoc codesign** only. They are **not** Developer ID signed and **not** notarized. Gatekeeper may report that the app cannot be opened or is damaged — that is expected, not a corrupt download.

Recommended steps:

1. Quit legacy `Codex++.app` / `Codex++ Manager.app` and any Chimera processes.
2. Open the DMG and drag `Chimera Codex.app` and `Chimera Codex Manager.app` into `/Applications`.
3. If legacy `Codex++*.app` bundles remain, move them to Trash manually (drag-and-drop will not overwrite differently named apps).
4. First launch: **right-click → Open**, or allow the app under System Settings → Privacy & Security.
5. If quarantine still blocks launch:

```bash
xattr -rd com.apple.quarantine "/Applications/Chimera Codex.app"
xattr -rd com.apple.quarantine "/Applications/Chimera Codex Manager.app"
```

## Highlights

- Rust backend and silent launcher with no extra runtime requirement.
- Tauri + React manager with dark/light theme support.
- External CDP injection. No `app.asar` patching and no DLL writes into the Codex installation.
- Relay injection with multiple profiles, a compatible provider id, and a one-click return to official ChatGPT login.
- Traditional enhancements: plugin marketplace unlock, session delete, Markdown export, project move, and more.
- Paste fix, Stepwise suggestions, user scripts, Provider Sync, Zed remote open, and per-model context windows via `model_catalog_json`.
- Public GitHub Release updates for both the manager and the silent launcher.

## Relay Injection

Relay injection is for users who are already logged in with an official ChatGPT account and want model requests to go through a compatible API:

- Official login still owns account features and the plugin entry.
- The relay profile only controls Base URL, key, and model names.
- Clearing API mode returns Codex to official login.

Before applying a relay profile: confirm ChatGPT login works, the Base URL (including `/v1`) is reachable, and probe the key with the smallest useful auth check. **Record only whether the key exists and whether auth passed — never paste real keys into logs, screenshots, or issues.**

A ChimeraHub-shaped config looks like:

```toml
model_provider = "CodexPlusPlus"

[model_providers.CodexPlusPlus]
name = "CodexPlusPlus"
wire_api = "responses"
requires_openai_auth = true
base_url = "https://api.chimerahub.org/v1"
experimental_bearer_token = "sk-..."
```

(`CodexPlusPlus` is a compatibility provider id, not the product display name.)

## Updates

Chimera Codex publishes installers through this repository's GitHub Releases and a public `latest.json` (asset names, sizes, and SHA-256). The manager About page can check and start updates. When the silent launcher finds a new version, it opens the manager on the update prompt.

## Data Locations

- Codex config: `~/.codex/config.toml`
- Codex auth state: `~/.codex/auth.json`
- Codex local database: prefers `~/.codex/sqlite/*.db`, falls back to legacy `~/.codex/state_5.sqlite`
- Tool state and logs: `~/.codex-session-delete/`
- Provider Sync backups: `~/.codex/backups_state/provider-sync`

## FAQ

### The menu does not appear

Launch from the `Chimera Codex` entry instead of stock Codex. You can also inspect Diagnostics and Logs in the manager.

### The plugin says the backend is disconnected

First test the helper endpoint:

```powershell
Invoke-RestMethod -Method Post -Uri http://127.0.0.1:57321/backend/status -Body "{}" -ContentType "application/json"
```

If the endpoint works but the plugin still times out, restart Chimera Codex or check logs for `renderer.script_loaded` / `bridge.request`.

### macOS says the app cannot be opened or is damaged

See **macOS install, Gatekeeper, and legacy apps** above. This distribution does **not** claim trusted or notarized installation.

## Development

```bash
# Frontend checks
cd apps/codex-plus-manager
npm install
npm run check
npm run vite:build

# Rust checks
cd ../..
cargo fmt --check
cargo test
cargo build --release

# Branding and no-promo gates
pwsh -File scripts/generate-branding.ps1 -Check
pwsh -File scripts/verify-no-upstream-ads.ps1
```

Project structure:

```text
apps/
  codex-plus-launcher/          Silent launcher
  codex-plus-manager/           Tauri manager
assets/inject/
  renderer-inject.js            Enhancement script injected into Codex
brand/
  product.toml                  Single source of branding truth
crates/
  codex-plus-core/              Launch, injection, config, update, install, bridge
  codex-plus-data/              Session data, export, Provider Sync
scripts/
  generate-branding.ps1         Branding generation / -Check
  verify-no-upstream-ads.ps1    No-promo scanner gate
```

## Attribution and License

This project is released under the **MIT** license and is based on upstream open-source work:

- [BigPizzaV3/CodexPlusPlus](https://github.com/BigPizzaV3/CodexPlusPlus) (Codex++)
- Related capabilities also benefit from community work such as [cc-switch](https://github.com/farion1231/cc-switch)

Chimera branding, the default ChimeraHub relay, and de-promotion changes belong to this fork and are not pushed upstream. Reusable general bugfixes may be split and contributed upstream.

Public repository: <https://github.com/Duojiyi/chimera-codex>

## Notes

Chimera Codex is an external enhancement tool and does not modify original Codex App files. If a future Codex App update changes page structure, the injection script may need updates.
