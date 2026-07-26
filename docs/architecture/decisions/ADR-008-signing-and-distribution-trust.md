# ADR-008: Signing and Distribution Trust

**Status:** Accepted
**Date:** 2026-07-26

> **Decision (product owner, 2026-07-26):** match 1.x exactly on distribution —
> NSIS installer on Windows with no Authenticode, ad-hoc signature only on
> macOS with no notarization, and GitHub Releases as the sole host for both the
> app and the Codex mirror. No certificate purchases, no Apple Developer
> Program, no owned domain or object storage.
>
> **Content signing is kept.** It was not part of what was being decided: it
> costs nothing, `services/mirror-contract/src/signature.rs` already implements
> it, and it is the only layer that governs which binary the app downloads and
> executes. See "What this decision does and does not change" at the end.

## Context

R2, R3, R4 and V2-R9 require signed releases and an owned mirror before 2.0.0
stable. Before choosing an approach, we looked at what 1.x actually does, since
it has been shipping to real users.

**1.x signs nothing.** This is not an inference; the workflow states it:

- Windows — `.github/workflows/release-assets.yml` builds an NSIS installer
  (`choco install nsis`). There is no `signtool` invocation, no certificate, no
  Authenticode step, and no signing secret anywhere in the repository. The
  shipped `ChimeraPlusPlus-<v>-windows-x64-setup.exe` is unsigned.
- macOS — the verification step runs `codesign --verify --deep --strict` and
  then prints, in its own words, `verified (ad-hoc only, not notarized)`. An
  ad-hoc signature carries no identity; Gatekeeper treats it as unsigned.
- Updates — `latest.json` is fetched from
  `github.com/<repo>/releases/latest/download/latest.json`. No Tauri updater
  key, no minisign, no content signature of any kind. There is no `pubkey` in
  any `tauri.conf.json`.
- Domain — none. Distribution is entirely GitHub Releases.

So 1.x's root of trust is *"whoever can push to the GitHub repository."* TLS to
github.com protects the transport and nothing protects the content.

That is a legitimate posture for a small tool, and it is why 1.x users see a
SmartScreen warning on install and macOS users have to right-click → Open. It
is not a posture v2 can inherit, because v2 downloads and executes a *second*
program (the managed Codex runtime) on the user's behalf. A compromised release
in 1.x replaces the manager; in v2 it also chooses what Codex binary the user
runs.

## Decision

Separate two things that are usually conflated, because only one of them costs
money and only one of them is on the critical path.

### 1. Content signing — our own trust roots. Free, and already underway.

The TUF-style roots required by G8/G15 are ed25519 keypairs we generate
ourselves. They cost nothing, need no vendor, and are what actually protects
the update path. `services/mirror-contract/src/signature.rs` already implements
verification; `crates/chimera-update` implements the app-side domain, kept
deliberately separate so the mirror root and the app root rotate and revoke
independently.

**This is the part that matters and it has no external blocker.** It is a
strict improvement over 1.x regardless of what we decide about the two items
below.

### 2. OS code signing — costs money and identity verification.

This buys exactly one thing: the operating system stops warning the user. It
does not make the update path safer — item 1 does that. Treating it as a
prerequisite for internal alpha and beta would block work for no security gain.

**Windows.** Since June 2023 all publicly-trusted code signing certificates
must have their private key in hardware (HSM or token), so the old "put a .pfx
in a CI secret" approach is no longer available from any CA. The practical
options for signing from GitHub Actions are cloud-HSM services:

| Option | Shape | Main catch |
|---|---|---|
| Azure Trusted Signing | Microsoft-run, cheapest, signs via an Action | Eligibility rules on org age/verification; check current region and entity requirements |
| SSL.com eSigner / DigiCert KeyLocker / Certum cloud | Commercial cloud HSM, CI-friendly | Annual cert + per-service fee |
| Physical token | Cheapest cert | Cannot be used from cloud CI at all — rules out our "build only in protected Actions" rule (G10) |

Two things worth knowing before choosing: SmartScreen reputation accrues *per
certificate*, so an OV certificate still shows warnings until enough people
have installed it, while an EV certificate starts with reputation. And an
individual (non-organisation) certificate publishes the individual's legal
name.

**Prices move; verify current figures before committing.** The structural
constraints above are what to design around.

**macOS.** Apple Developer Program membership, a Developer ID Application
certificate, and notarization via `notarytool`. There is no alternative — an
unsigned or ad-hoc app is blocked by Gatekeeper, not merely warned about. This
is a hard prerequisite for shipping macOS at all, which is a further reason D8
ships Windows first.

### 3. Domain and mirror — already owned.

`chimerahub.org` exists and serves the live API (verified against
`https://api.chimerahub.org/v1/models`). No new domain is needed. The mirror
becomes a subdomain plus object storage. R4's remaining substance is the two
*isolated signing roots* — which is item 1, and free.

## Consequences

- Internal alpha and public beta ship **unsigned on Windows**, exactly as 1.x
  does today, with content signing already enforced. Users see the same
  SmartScreen prompt they see for 1.x. This is not a regression.
- macOS does not ship until the Apple Developer Program membership exists.
  That is the one purchase with no workaround.
- R1 is already resolved by the D6 revision: no official Codex binary is
  distributed with our package, so the legal review that blocked it is moot.
- R2 moves off the critical path for beta and back onto it for 2.0.0 stable.
- The release workflow is built with the signing step present but skipped when
  the credential secret is absent, so turning it on later is configuration
  rather than rework — and so the unsigned path is a visible, deliberate branch
  rather than an omission nobody notices.

## What this decision does and does not change

**Adopted, matching 1.x:**

| Area | v2 does |
|---|---|
| Windows packaging | NSIS installer, unsigned. Reuse `scripts/installer/windows/`, which already exists and ships |
| macOS packaging | Ad-hoc `codesign` only, not notarized. Same DMG/zip flow as 1.x |
| Hosting | GitHub Releases for the app **and** for the Codex mirror. No domain, no object storage |
| Update discovery | `chimera-app-latest.json` and the mirror manifests are release assets, same as 1.x's `latest.json` |

**Kept, because it is free and load-bearing:**

- ed25519 content signatures on every manifest, verified client-side before
  anything is written to disk (V2-R9).
- Two separate trust roots — mirror vs app — that rotate and revoke
  independently (G8, G15). Hosting them both on GitHub does not merge them; a
  root is a keypair, not a URL.
- Client-side anti-rollback, expiry and freeze-attack checks. These matter
  *more* under this decision, not less: with GitHub as the only host, "withdraw
  a bad stable" means replacing a release asset, and a client that accepts a
  replayed older manifest would keep installing the withdrawn version.

**Dropped from the release gates:**

- R2 (Windows Authenticode) — no longer a gate. Users see the same SmartScreen
  prompt they already see for 1.x.
- R3 (Developer ID + notarization) — no longer a gate. macOS users open the app
  via right-click → Open, as they do for 1.x today.
- R4's domain and object-storage half — no longer a gate. The two isolated
  signing roots half stays.

**Unchanged:** G10 still requires builds and stable promotion to happen only in
protected GitHub Actions. That constraint was never about certificates; it is
about no one being able to publish from a laptop.

The release workflow still carries the signing step behind a
"skip when the secret is absent" branch, so this stays a visible, deliberate
choice rather than an omission — and so reversing it later is configuration
rather than rework.
