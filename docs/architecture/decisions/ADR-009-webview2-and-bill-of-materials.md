# ADR-009: WebView2 Strategy and Delivery Bill of Materials

**Status:** Accepted
**Date:** 2026-07-26

## Context

Step 6.1 requires a decided WebView2 strategy and a written bill of materials
for each delivery shape, before anything is built.

The Spec states the constraint plainly: Tauri's dependency on WebView2 must not
become a hidden condition that makes an "offline" download still need the
network on first run. That is a promise about honesty as much as about
engineering — a package labelled offline that silently downloads a runtime is
worse than one that never claimed to be offline.

Two things changed the shape of this decision after it was first written:

- **D6 (revised 2026-07-26)** removed the Codex payload from our packages. The
  client downloads it on first run. So *every* delivery shape needs the network
  eventually, and "offline" can only ever mean "installs without the network",
  never "works without it".
- **ADR-008** decided nothing is code-signed. That removes the option of
  shipping a runtime installer we vouch for with our own signature; whatever we
  redistribute has to stand on Microsoft's signature alone.

## Decision

### WebView2: preflight and direct the user, do not redistribute

Windows 11 ships the WebView2 Evergreen Runtime as part of the OS, and it has
been distributed to supported Windows 10 installations for years. The realistic
gap is a small number of older or heavily-managed Windows 10 machines.

We do **not** bundle the Evergreen Standalone Installer and we do **not** carry
a fixed-version runtime. Instead the app preflights for WebView2 before it
touches any configuration or runtime state, and when it is missing it says so
and links to Microsoft's official download.

Reasons, in the order they mattered:

1. **Redistribution we cannot vouch for.** Under ADR-008 our installer is
   unsigned. Carrying a bundled Microsoft installer inside an unsigned package
   asks the user to trust that we did not modify it, with nothing to check that
   against. Sending them to Microsoft's own signed download removes us from
   that trust chain entirely.
2. **A fixed-version runtime is a security liability we would own.** It does
   not auto-update. Pinning a browser engine means shipping known-vulnerable
   rendering code to users until we notice and re-release — for an app that
   already fetches and executes a second program, that is the wrong thing to
   take responsibility for.
3. **The size is not free.** The Evergreen Standalone Installer is roughly the
   size of our entire app; the fixed-version runtime is several times larger.
   Paying that on every download to serve a shrinking minority is a poor trade
   when the fallback is a two-click download from Microsoft.
4. **D6 already ended the pretence of a network-free first run.** Since the
   Codex payload is fetched on first run regardless, a bundled WebView2 would
   not buy an offline install — it would only move which download happens.

The failure must stay honest and recoverable. Preflight runs **before** any
state is written, so a machine without WebView2 is left exactly as it was, with
a plain explanation and a link — never a half-configured install.

### What "offline installer" means, and what it is renamed to

Because it can no longer mean "installs and runs with no network", the second
delivery shape is not called an offline package. There is one Windows delivery:

| | Windows installer (NSIS) |
|---|---|
| Contains | Chimera++ only |
| Does not contain | Any official Codex binary (D6), any WebView2 runtime, any key or token |
| Needs at install time | Nothing |
| Needs at first run | Network — to fetch the Codex payload, and to fetch WebView2 if the OS lacks it |
| Signing | None (ADR-008); SmartScreen will warn |

If a genuinely air-gapped shape is ever required, it is a new decision with its
own ADR, not a variant of this one — it would need a payload distribution
review (the R1 that D6 was changed to avoid) and a WebView2 redistribution
review, both of which this decision deliberately steps around.

## Bill of materials

Every delivered artifact contains exactly:

- `Chimera++.exe` — the app
- `NOTICE`, `LICENSE` — AGPL-3.0-only plus third-party notices (R8, G11)
- `checksums.txt` — SHA-256 of every other file

and nothing else. `scripts/test-bundle-contract.mjs` enforces the absence side
against the built artifact, with self-tests proving each rejection fires.

## Version naming

`Chimera++_<semver>_<platform>-<arch>-setup.exe`, matching what
`scripts/v2-release-manifest.mjs` parses. It refuses an asset whose platform or
arch it cannot read rather than silently omitting it from what users are
offered, so the naming rule is enforced rather than documented.

## Resource requirements

| | Value | Why |
|---|---|---|
| Disk, install | ~60 MB | The app itself |
| Disk, first run | 2× the payload size, free | The payload is downloaded **and** unpacked. Requiring only its size would move the failure into extraction, which leaves a half-written version directory instead of a clean refusal |
| Install location | Per-user, under `%LOCALAPPDATA%` | No administrator rights (R15) |
| Data location | Per-user app data | Never `Program Files`: a per-user install cannot rely on writing there |

`chimera_runtime::download::preflight` enforces the free-space rule, probing
with a real write rather than a metadata check — read-only mounts and
restrictive ACLs report as fine to the latter. Unmeasurable free space does not
block the install, because the write already fails safely and refusing because
we could not measure a disk would be worse than letting it try.

## Consequences

- No Microsoft redistributable ships with Chimera, so no redistribution terms
  apply to us and R10 is resolved by not needing it.
- Users on the small set of Windows 10 machines without WebView2 get one extra
  step, before anything on their machine has changed.
- There is exactly one Windows artifact to build, test and support, rather than
  an "online" and "offline" pair that differ in a way users misread.
- The word "offline" does not appear on the download page, because after D6 it
  would not be true of anything we ship.
