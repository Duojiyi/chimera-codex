# ADR-002: Managed Portable as Primary Runtime Mode

**Status:** Accepted  
**Date:** 2026-07-26

## Context

Codex is distributed as a Windows Store MSIX and as a portable executable. Chimera needs to install, update, and roll back Codex without requiring administrator rights, and without conflicting with user-owned or externally managed installations.

## Decision

Chimera recognises three runtime modes:

| Mode | Description | Chimera action |
|---|---|---|
| `ManagedPortable` | Chimera-owned directory under `%LOCALAPPDATA%/ChimeraPlusPlus/runtime/` | Full lifecycle: install, update, repair, rollback |
| `ExternalMsix` | System-registered Store package | Detect and launch only; never modify |
| `ExternalPortable` | User or other-manager-owned directory | Detect and optionally import with explicit user confirmation; never modify without confirmation |

The default for new installations is `ManagedPortable`. The client offline bundle ships with an unmodified official MSIX payload; first run extracts it into the managed directory after verifying the official Authenticode signature.

**Ownership manifest** (`ownership.json`) is written to the managed runtime root and contains:
- `install_mode`, `canonical_path`, `codex_version`
- `source_manifest_digest`, `file_tree_digest`
- `created_by_chimera_version`, `transaction_state`
- `last_health_result`, `created_at`, `updated_at`

Every destructive operation (update, repair, rollback, delete) first verifies the manifest exists, the canonical path matches, and the file-tree digest is consistent before proceeding.

## Consequences

- No administrator rights needed for install, update, or rollback.
- Chimera cannot manage an ExternalMsix; if the user only has an MSIX install, Chimera offers to set up a parallel ManagedPortable.
- Two managers cannot own the same directory simultaneously — the second to acquire the ownership lock enters read-only mode.
- Portable installations lack MSIX package identity, Store auto-update, `codex://` URI registration, and Apps & Features uninstall record. Chimera's UI discloses these limitations honestly and provides its own update/cleanup paths.
