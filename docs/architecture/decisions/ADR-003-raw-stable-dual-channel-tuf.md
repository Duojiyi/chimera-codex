# ADR-003: Raw/Stable Dual Channel with TUF-Style Trust

**Status:** Accepted  
**Date:** 2026-07-26

## Context

Chimera distributes official Codex builds verbatim. A naive mirror that pushes every new upstream release directly to end users would expose them to untested builds. A single long-lived online signing key is a persistent compromise target.

## Decision

### Dual channel

- **`raw` channel**: every newly probed official package is stored as-is with its provenance (source URL, ETag, SHA-256, official signature metadata). `raw` is never directly consumable by end-user clients.
- **`stable` channel**: only packages that passed the compatibility gate are promoted. The gate requires: official identity + Authenticode/Sparkle verification, clean VM cold boot, provider projection test, optional skin compatibility. Promotion uses a compare-and-swap against the current stable pointer to prevent older parallel workflows from overwriting a newer stable.

### TUF-style key hierarchy

```
offline root key  (air-gapped, never online)
       │ trusts
  online targets/stable key  (CI environment secret)
```

Root metadata carries: version, threshold, expiry, consecutive signatures from previous + new key (for rotation). Clients persist the highest seen root version and reject any metadata with a lower version (anti-rollback). Clients also reject expired metadata (anti-freeze).

Online key compromise: offline root publishes a new root metadata document revoking the old online key and designating a replacement. Clients fetch and verify the new root before accepting any new stable pointer.

### Capability manifest

Every stable promotion also produces a `skin-compat-<codex_version>.json` capability manifest, signed by the stable key, bound to the exact raw digest. This is the only authoritative source for skin compatibility data; Theme Engine consumers may not self-generate it.

## Consequences

- Two separate signing ceremonies: one for mirror (offline root + online stable key), one for Chimera app updates (separate hierarchy per ADR-006).
- Rotation and revocation runbooks must be tested before first public release (Release Gate R4).
- Client must persist trusted root metadata across restarts; cold-start state must be seeded from a bundled root document at build time.
- Rollback test: feeding an older stable manifest to the client must be rejected.
