# ADR-005: Restricted Skin Architecture

**Status:** Accepted  
**Date:** 2026-07-26

## Context

Chimera++ 1.x used CDP injection via `dream_skin` with arbitrary JavaScript. This is a significant security and stability risk: arbitrary JS can exfiltrate API keys, break Codex startup, and modify official app files in ways that are hard to audit or reverse.

Codex App Manager (`codex-theme-engine`) demonstrates a safer approach: a declarative `.codexskin` package format with restricted content types and a loopback-only CDP session.

## Decision

Adopt and adapt the `codex-theme-engine` approach with the following constraints:

### Package format (`.codexskin`)
- **Allowed**: CSS files, image assets (PNG/JPEG/WebP/SVG), font files, a `theme.json` manifest
- **Banned**: JavaScript files, executable files, external protocol handlers, remote resource URLs, path traversal in filenames
- Package validation: schema check, path traversal scan, size limits, MIME type verification, CSS property allowlist

### CDP session
- CDP is enabled only when a skin or future explicitly-approved enhancement requires it
- Port: random loopback (`127.0.0.1:<random>`) — never `0.0.0.0`
- Process: owned child process of Chimera; killed on session exit or Chimera shutdown
- Target discovery: Chimera-launched Codex only, never attaches to externally started processes

### Compatibility gating
- Each stable Codex release produces a `skin-compat-<codex_version>.json` capability manifest (signed by mirror stable key, bound to exact raw digest)
- Theme Engine reads this manifest at session start; if the Codex version's fingerprint/selectors no longer match, the skin is auto-disabled and Codex starts without it
- User-visible error: "Skin incompatible with this Codex version — restored default"

### Kill switch
- Server-side kill switch can disable Theme Engine entirely; it cannot execute remote code or alter provider config/keys
- Kill switch is a signed message from the Chimera app update channel

## Consequences

- Skins cannot break Codex startup — worst case is Codex starts unskinned
- Skin authors cannot inject arbitrary JavaScript
- CDP exposure is minimal: random port, loopback only, owned child, session-scoped
- Compatibility manifest generation is the mirror gate's responsibility (ADR-003)
- 2.0.0 ships basic skin trial/apply/restore only; session export and model catalog remain future candidates
