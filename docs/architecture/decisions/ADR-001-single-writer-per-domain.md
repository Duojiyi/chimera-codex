# ADR-001: Single Writer Per Domain

**Status:** Accepted  
**Date:** 2026-07-26

## Context

Chimera++ 2.0 manages three distinct configuration domains:

1. **Provider config** — `~/.codex/config.toml` and auth files
2. **Managed Codex runtime** — install directory under `%LOCALAPPDATA%/ChimeraPlusPlus/runtime/`
3. **Skin/enhancement session** — active CDP session state

Multiple tools may co-exist: CC Switch, Codex App Manager, the official Codex CLI, and external editors all can read or write `~/.codex/config.toml`. Without coordination, concurrent writes produce corrupt or inconsistently merged state.

## Decision

Each domain has **exactly one writer at a time**, enforced by:

- **Cross-process operation lock** (file-based advisory mutex) acquired before any write to a domain. Lock includes PID, timestamp, and operation type in its content.
- **CAS (compare-and-swap) before commit**: after acquiring the lock, re-read the file's identity (inode/mtime/size) and content hash. If they differ from the snapshot taken at lock acquisition, the operation enters an explicit **conflict state** — it preserves both candidate and snapshot, never silently overwrites external changes.
- **Persistent write-ahead journal** per domain: every intended change is journaled before execution. On startup, Chimera replays incomplete journals to reach a consistent state.
- **Ownership manifest** for the runtime directory: a JSON file recording install mode, canonical path, Codex version, file-tree digest, Chimera version, and transaction state. All runtime modifications verify ownership before proceeding.
- **Read-only degradation**: if Chimera detects another manager actively operating the same resource (via lock file or ownership mismatch), it enters read-only/conflict UI rather than fighting for the lock.

## Consequences

- Config switching requires acquiring the lock, snapshotting, writing a journal, staging candidates, CAS check, atomic rename, verifying final hashes, and clearing the journal.
- Multi-step operations cannot rely on in-process compensation ("write A, on failure write A back"). They must use the journal and atomic renames at every step.
- Tests must inject a platform mock for the lock and filesystem so fault-injection scenarios (crash between journal write and rename) are reliably exercised.
- Users see a clear conflict UI with three-way merge options when external changes are detected, never silent data loss.
