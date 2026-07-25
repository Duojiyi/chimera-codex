# ADR-006: Independent TUF Trust Roots for App and Payload Updates

**Status:** Accepted  
**Date:** 2026-07-26

## Context

Chimera self-updates and Codex payload updates have different release cadences, different signing authorities (Chimera team vs. official OpenAI/Microsoft signing), and different risk profiles. Sharing a trust root means a compromise of one key jeopardizes both update paths.

## Decision

Two completely separate trust hierarchies:

### Chimera App Update hierarchy
```
chimera-app-offline-root  (air-gapped)
       └── chimera-app-targets key  (CI protected environment)
```
- Manifest: `chimera-app-latest.json`
- Tracks: Chimera app version, minimum supported Codex version, download URL, SHA-256, signatures
- Stored: Chimera's own release channel

### Mirror/Payload hierarchy  
```
chimera-mirror-offline-root  (air-gapped, separate from app root)
       └── chimera-mirror-stable key  (mirror CI protected environment)
```
- Manifest: `codex-stable.json`  
- Tracks: Codex version, official Authenticode/Sparkle identity, raw digest, compatibility gate results, capability manifest
- Stored: `Duojiyi/chimera-codex-mirror` release channel

### Separation properties
- Each hierarchy has its own: offline root key pair, online key pair, version counter, expiry, rotation runbook, revocation procedure
- Client maintains two separate persistent trust state files
- An online key compromise in one hierarchy does NOT affect the other
- Chimera update failure cannot corrupt Codex payload state
- Codex update failure cannot block Chimera's repair UI from loading

## Consequences

- Two separate signing ceremonies for releases
- Two separate key rotation rehearsals required before first public release (Release Gate R4)
- Higher operational overhead is the acceptable price for blast-radius containment
- Rotation of either key requires publishing new root metadata via the offline key, then having clients fetch and verify the new root before accepting further updates
