# Chimera++

<p align="center">
  <img src="docs/images/codex-plus-plus.png" alt="Chimera++ icon" width="160">
</p>

<p align="center">
  <a href="README.md">中文</a> | English
</p>

Chimera++ is an external Codex App launcher and manager for ChimeraHub customers. It does not modify the original Codex installation; it starts Codex externally and injects the required enhancements.

This edition defaults to the **ChimeraHub** relay and removes promotion, sponsorship, and community-group entry points. Regular users need no knowledge of source repositories or update infrastructure.

> Release readiness: This development snapshot has not completed single-desktop-entry or automatic-update enforcement acceptance and must not be delivered as a customer release. The single-entry and automatic-update text below describes the target behavior after acceptance.

## Quick Start

Get the installer for your computer from ChimeraHub support or your customer delivery page:

- Windows: `ChimeraPlusPlus-*-windows-x64-setup.exe` (zip portable package also available)
- macOS Intel: `ChimeraPlusPlus-*-macos-x64.dmg`
- macOS Apple Silicon: `ChimeraPlusPlus-*-macos-arm64.dmg`

On the target Windows release, the installer creates only one `Chimera++` desktop shortcut. The desktop `Chimera++` shortcut opens the manager, and you can launch Codex from the manager. `Chimera++ 管理工具` also remains available from the Start Menu for setup, diagnostics, and maintenance. The macOS package contains both apps; use `Chimera++.app` for daily launches.

- `Chimera++`: the everyday manager and launch entry on Windows, and the daily launch entry on macOS.
- `Chimera++ 管理工具`: the Key setup, diagnostics, repair, and advanced settings entry.

## First-run setup (ChimeraHub Key-first)

A fresh install automatically creates and selects the **ChimeraHub** relay profile:

| Field | Value |
|---|---|
| Base URL | `https://api.chimerahub.org/v1` (the `/v1` suffix is required) |
| Protocol | Responses |
| Default model | `gpt-5.5` |

You only need to enter your API Key:

1. Open `Chimera++ 管理工具`.
2. Enter your API Key on the ChimeraHub setup page.
3. Click **Save and enable**.

An empty Key does not write live config and does not send business requests. **Do not put real keys** in docs, screenshots, or issues; examples always use placeholders such as `sk-...`.

Upgrading an existing install does not overwrite your current relay profiles or active selection.

## Windows in-place upgrade

The Windows installer supports overlaying an existing Codex++ install root (phase one keeps `$LOCALAPPDATA\Programs\Codex++` to reduce migration cost):

1. Quit running `Codex++` / `Chimera++` and manager processes.
2. Run `ChimeraPlusPlus-*-windows-x64-setup.exe`.
3. The installer cleans legacy shortcuts and creates only Chimera entry points.
4. Launch from the new desktop / Start Menu shortcuts.

The updater anonymously reads the Chimera++ public update manifest and never falls back to another update source.

## macOS install, Gatekeeper, and legacy apps

Releases provide separate `macos-x64` and `macos-arm64` DMGs. Current macOS builds use **ad-hoc codesign** only. They are **not** Developer ID signed and **not** notarized. Gatekeeper may report that the app cannot be opened or is damaged — that is expected, not a corrupt download.

Recommended steps:

1. Quit legacy `Codex++.app` / `Codex++ 管理工具.app` and any Chimera processes.
2. Open the DMG and drag `Chimera++.app` and `Chimera++ 管理工具.app` into `/Applications`.
3. If legacy `Codex++*.app` bundles remain, move them to Trash manually (drag-and-drop will not overwrite differently named apps).
4. First launch: **right-click → Open**, or allow the app under System Settings → Privacy & Security.
5. If quarantine still blocks launch:

```bash
xattr -rd com.apple.quarantine "/Applications/Chimera++.app"
xattr -rd com.apple.quarantine "/Applications/Chimera++ 管理工具.app"
```

## Highlights

- Rust backend and silent launcher with no extra runtime requirement.
- Tauri + React manager with dark/light theme support.
- External CDP injection. No `app.asar` patching and no DLL writes into the Codex installation.
- Relay injection with multiple profiles, a compatible provider id, and a one-click return to official ChatGPT login.
- Traditional enhancements: plugin marketplace unlock, session delete, Markdown export, project move, and more.
- Paste fix, Stepwise suggestions, user scripts, Provider Sync, Zed remote open, and per-model context windows via `model_catalog_json`.
- Automatic startup updates with integrity checks and preservation of the current working version on failure.

## Relay Injection

Relay injection is for users who are already logged in with an official ChatGPT account and want model requests to go through a compatible API:

- Official login still owns account features and the plugin entry.
- The relay profile only controls Base URL, key, and model names.
- Clearing API mode returns Codex to official login.

Before applying a relay profile: confirm ChatGPT login works, the Base URL (including `/v1`) is reachable, and probe the key with the smallest useful auth check. **Record only whether the key exists and whether auth passed — never paste real keys into logs, screenshots, or issues.**

ChimeraHub Key-first uses the Pure API path. Its generated `config.toml` shape is:

```toml
model = "gpt-5.5"
model_provider = "custom"

[model_providers.custom]
name = "custom"
wire_api = "responses"
requires_openai_auth = true
base_url = "https://api.chimerahub.org/v1"
```

The key is stored separately in `auth.json`:

```json
{
  "OPENAI_API_KEY": "sk-..."
}
```

`custom` is the provider id in Codex configuration, not the product display name. Key-first does not use the mixed-mode `experimental_bearer_token`.

## Updates

The target Chimera++ release checks for and installs updates automatically at startup. A supported version can still launch when a normal update or network request fails. Only a version below the minimum supported version must update before continuing. Updates require no user action.

## Data Locations

- Codex config: `~/.codex/config.toml`
- Codex auth state: `~/.codex/auth.json`
- Codex local database: prefers `~/.codex/sqlite/*.db`, falls back to legacy `~/.codex/state_5.sqlite`
- Tool state and logs: `~/.codex-session-delete/`
- Provider Sync backups: `~/.codex/backups_state/provider-sync`

## FAQ

### The menu does not appear

Launch from the `Chimera++` entry instead of stock Codex. You can also inspect Diagnostics and Logs in the manager.

### The plugin says the backend is disconnected

First test the helper endpoint:

```powershell
Invoke-RestMethod -Method Post -Uri http://127.0.0.1:57321/backend/status -Body "{}" -ContentType "application/json"
```

If the endpoint works but the plugin still times out, restart Chimera++ or check logs for `renderer.script_loaded` / `bridge.request`.

### macOS says the app cannot be opened or is damaged

See **macOS install, Gatekeeper, and legacy apps** above. This distribution does **not** claim trusted or notarized installation.

## Development and Open-source Attribution

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

Chimera++ is distributed as a modified combined work under [AGPL-3.0-only](LICENSE). See [NOTICE](NOTICE) for third-party licenses, the upstream baseline, and the license-change timeline. Corresponding source is published in the public repository: <https://github.com/Duojiyi/chimera-codex>.

This project is based on the following open-source and community work:

- [BigPizzaV3/CodexPlusPlus](https://github.com/BigPizzaV3/CodexPlusPlus) (Codex++)
- Related capabilities also benefit from community work such as [cc-switch](https://github.com/farion1231/cc-switch)

Chimera branding, the default ChimeraHub relay, and de-promotion changes belong to this fork and are not pushed upstream. Reusable general bugfixes may be split and contributed upstream.

## Notes

Chimera++ is an external enhancement tool and does not modify original Codex App files. If a future Codex App update changes page structure, the injection script may need updates.
