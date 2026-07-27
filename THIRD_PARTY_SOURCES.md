# THIRD_PARTY_SOURCES.md

This document registers all third-party code sources referenced or adopted by the Chimera++ v2 project. Each entry records the source repository, baseline version/commit, license, the scope of what is adopted, and what is explicitly excluded. File-level imports are registered individually when code is brought in.

---

## Table of Contents

1. [cc-switch (farion1231)](#1-cc-switch-farion1231)
2. [CodexPlusPlus (BigPizzaV3)](#2-codexplusplus-bigpizzav3)
3. [Codex-App-Manager (Wangnov)](#3-codex-app-manager-wangnov)
4. [codex-app-mirror (Wangnov)](#4-codex-app-mirror-wangnov)
5. [File-Level Import Registration Process](#file-level-import-registration-process)

---

## 1. cc-switch (farion1231)

| Field | Value |
|---|---|
| Repository | https://github.com/farion1231/cc-switch |
| Baseline version | v3.18.0 |
| Baseline commit | `606e7bb` (tag ref to be confirmed at import time) |
| License | MIT |

### Adopted scope

- Codex config parsing patterns
- Provider URL and API key data model
- Atomic config write patterns
- Backup logic
- Tray quick-switch UX concepts
- `web_search` field handling

### Explicitly NOT adopted

- MCP/Skills all-in-one abstraction
- Provider marketplace ads
- Protocol proxy (Chat Completions to Responses API)
- Cloud sync
- Usage billing panel
- Failover/rotation logic

### Notes

File-level entries will be added to this document when specific files are imported. The baseline commit (`606e7bb`) should be verified against the v3.18.0 tag at import time and updated here if they diverge.

---

## 2. CodexPlusPlus (BigPizzaV3)

| Field | Value |
|---|---|
| Repository | https://github.com/BigPizzaV3/CodexPlusPlus |
| Baseline version | v1.2.42 |
| Baseline commit | `657cd33e009ad02515d30db6492cd4e669b06318` |
| License | AGPL-3.0 |

### Adopted scope

- Legacy 1.x migration knowledge
- Official login protection patterns
- Model catalog reference
- CDP (Chrome DevTools Protocol) launch experience reference
- Read-only session export reference

### Explicitly NOT adopted

- Existing monolithic core and UI
- Upstream sync workflow
- Injection feature set
- Watcher subsystem
- User scripts
- Any features with high coupling to the 1.x monolith

### Notes

This repository IS the current codebase from which Chimera++ v2 is derived. v2 is a clean-slate rewrite and does not inherit the monolithic architecture. Patterns listed under the adopted scope are used as reference only; no code block is carried forward wholesale. Because this project is AGPL-3.0 licensed, any file-level imports must be tracked carefully and the AGPL obligations reviewed before inclusion.

---

## 3. Codex-App-Manager (Wangnov)

| Field | Value |
|---|---|
| Repository | https://github.com/Wangnov/Codex-App-Manager |
| Baseline version | v0.5.0 |
| Baseline commit | `89b542b9299453dcd833757b10cdb15f6d14d527` |
| License | MIT |

### Adopted scope

- `codex-win-engine`: Windows runtime install, update, and rollback logic
- `codex-mac-engine`: macOS Sparkle integration and codesign patterns
- `codex-theme-engine`: restricted CDP skin approach

### Explicitly NOT adopted

- Second manager UI shell
- Duplicate self-updater outer shell

### Notes

The three engine components above are the primary adoption targets. The outer manager shell and self-updater are redundant with Chimera++ v2's own update infrastructure and are excluded to avoid duplication and maintenance burden.

### Windows install engine dependency — from Codex-App-Manager

| Field | Value |
|---|---|
| Source repo | https://github.com/Wangnov/Codex-App-Manager |
| Source commit | `89b542b9299453dcd833757b10cdb15f6d14d527` |
| Source path | `crates/codex-win-engine` |
| Local path | `Cargo.toml` pinned dependency; adapter in `crates/chimera-runtime/src/manager.rs` |
| License | MIT |
| Modifications | None to dependency; Chimera adapter narrows release discovery and install contracts |

---

## 4. codex-app-mirror (Wangnov)

| Field | Value |
|---|---|
| Repository | https://github.com/Wangnov/codex-app-mirror |
| Baseline version | n/a (no version tag) |
| Baseline commit | `84c625145ad2e2cfc6f06439250c1fddce0eff14` |
| License | MIT |

### Adopted scope

- Official source probing design
- Raw mirror pipeline concepts
- Checksums and manifest schema reference
- Sparkle appcast reference
- Region routing design

### Explicitly NOT adopted

- Original brand and sponsor content
- Direct latest-push behavior without a Chimera++ compatibility gate

### Notes

All content derived from this repository must pass through the Chimera++ compatibility gate before being surfaced to users. Brand and sponsor content from the upstream mirror is stripped at import; only structural and schema patterns are carried forward.

---

## File-Level Import Registration Process

When a specific file (or a portion of a file) is imported from any of the sources above, it must be registered here before or at the time the import is merged into the Chimera++ v2 tree. Use the following template.

### Template

```
### <short descriptor> — from <repo name>

| Field         | Value                                      |
|---------------|--------------------------------------------|
| Source repo   | <repository URL>                           |
| Source commit | <exact full commit SHA at time of import>  |
| Source path   | <path within the source repository>        |
| Local path    | <absolute or repo-relative path in v2>     |
| License       | <SPDX identifier, e.g. MIT, AGPL-3.0>     |
| Modifications | <description of changes made, or "none">   |
```

### Rules

1. Commit SHA must be the full 40-character SHA, not a short hash or tag alias. Resolve the tag to a commit at import time if the baseline entry above uses a short hash.
2. Source path is the path as it exists in the source repository at the recorded commit.
3. Local path is the path in the Chimera++ v2 repository where the file (or derived file) lives.
4. Modifications must describe any changes made relative to the upstream file. If the file is taken verbatim, write "none". If it is substantially reworked, write "derived — <summary>".
5. AGPL-3.0 imports (source 2, CodexPlusPlus) require an additional legal review note confirming the AGPL obligation is understood and addressed before the entry is considered complete.
6. No file may be imported from the "Explicitly NOT adopted" scope of its source entry without a separate review and an updated entry in this document.
7. This document is append-only for completed imports. Entries are never deleted; if an import is later removed from the codebase, annotate the entry with a removal note and the commit at which it was removed.

### Example entry

```
### Atomic config write — from cc-switch

| Field         | Value        |
|---------------|--------------------------------------------------------------|
| Source repo   | https://github.com/farion1231/cc-switch                      |
| Source commit | 606e7bb<full SHA to be filled at import>                     |
| Source path   | src/config/atomicWrite.ts                                    |
| Local path    | packages/config/src/atomicWrite.ts                           |
| License       | MIT                |
| Modifications | Removed provider-marketplace dependency; adapted to v2 types |
```

---

*Last updated: 2026-07-26. Maintainer: Chimera++ v2 core team.*
