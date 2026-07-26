# Chimera++ 2.0 Threat Model

**Step 9.4** · Last updated 2026-07-26

## What makes this app worth attacking

Chimera++ is not primarily a place where secrets sit. It is a program that
**downloads and runs another program on the user's behalf**, holds the API keys
that program authenticates with, and writes the config file that decides where
that program sends every request.

That shapes the whole document. The highest-value outcome for an attacker is
not reading a key — it is getting Chimera to install and launch a binary of
their choosing, or to point Codex at an endpoint of their choosing. Key theft is
second. Both rank above anything involving the UI.

Two decisions narrow the surface before any control does:

- **D6** — no official Codex payload ships in our package. It is fetched at
  runtime against a digest from a signed manifest. This moves the trust decision
  from build time to `crates/chimera-runtime/src/download.rs`.
- **ADR-008** — nothing is OS code-signed. There is no Authenticode or
  notarization boundary to lean on, so every guarantee has to come from content
  signatures we verify ourselves.

## Assets, in the order an attacker would want them

| # | Asset | Why it is worth taking | Where it lives |
|---|---|---|---|
| 1 | Ability to install a chosen binary | Full code execution as the user | `chimera-runtime::download`, `chimera-update::trust` |
| 2 | Provider API keys | Directly monetisable | OS credential store, via `chimera-provider::keychain` |
| 3 | The live Codex `config.toml` | Redirects every request; a quiet MITM | `chimera-provider::projection` |
| 4 | Official Codex login token | Account access | Owned by Codex; Chimera never reads it |
| 5 | Local diagnostic data | Paths, usernames, sometimes keys | `chimera-update::diagnostics` |

## Trust boundaries

```
 user input ──► webview ──IPC──► Tauri commands ──► service crates ──► OS
                                                          │
 our mirror ──HTTPS──► download/update ────────────────────┘
 (GitHub Releases)          │
                            └──► signature + digest verification
```

Four boundaries, each with its own section below:

- **B1** webview → Rust (IPC)
- **B2** network → client (mirror and update)
- **B3** Chimera → managed Codex process
- **B4** skin package → CDP session → Codex UI

---

## B1 — Webview to Rust

### Untrusted-input assumption

The webview is our own code, but it is also the layer most exposed to content
we do not control (provider error strings, model lists, skin metadata). Every
command therefore treats its arguments as hostile.

### Threats and controls

**T1.1 — A command trusts a plan the frontend hands back.**
The cleanup feature originally suggested passing a `CleanupPlan` from the UI to
an execute call. That would have made "delete these paths" attacker-controllable
from any XSS-equivalent in the webview.
*Control:* `portable_cmds::execute_cleanup` re-derives the plan server-side and
ignores anything from the frontend. Documented at the call site as the reason.

**T1.2 — Secrets crossing the IPC boundary in the wrong direction.**
*Control:* `ProviderDto` deliberately drops `secret_ref`; only whether a key
exists crosses. An API key crosses exactly once, inward, on add. `dto.rs` tests
pin the wire shape.

**T1.3 — A snake_case DTO field reads as `undefined` and a screen silently
renders "everything is fine".**
This is a safety issue, not a cosmetic one: the first-run screen would report a
passing preflight on a machine that cannot run the app.
*Control:* camelCase tests in `dto.rs`, `bootstrap_cmds.rs`, `portable_cmds.rs`.

**T1.4 — Error strings leaking paths, usernames or key material into
screenshots and support tickets.**
*Control:* every user-facing error is a fixed, actionable sentence. Tests assert
absence of `os error`, of the URL, and of `sk-`. `CleanupEntry::display_label`
is a bare name, never a path.

**Residual:** the webview has no CSP (`tauri.conf.json` sets `csp: null`). Our
UI loads no remote resources, so there is nothing to isolate today, but this
should become a real CSP before any screen renders provider-supplied HTML.

---

## B2 — Network to client

This is the boundary that matters most, because crossing it successfully means
choosing what code runs.

### The two independent trust domains (G8, G15)

`chimera-update` (the app) and `services/mirror-contract` (the Codex payload)
have separate roots, separate caches, and **deliberately duplicated signature
code**. Sharing a crate between them is exactly what G15 forbids: one
compromised key must not reach across.

`chimera-update::metadata::APP_TRUST_DOMAIN` is checked at parse time, before a
single signature is inspected, so a mirror document pointed at the app chain is
refused as a domain error rather than surfacing later as a signature mismatch
somebody could misread as "the mirror rotated a key".

### Threats and controls

**T2.1 — Rollback.** Serve an older, validly-signed release to undo a security
fix.
*Control:* `trust::check_rollback`, per role, with the floor persisted between
runs. Equal versions are accepted; only a decrease is refused.

**T2.2 — Freeze.** Keep serving yesterday's timestamp so the client never
learns a newer snapshot exists.
*Control:* expiry on all four roles, with an injectable clock so the test can
exist. Enforced on every role, not just timestamp — checking one would let a
stale targets list through behind a fresh timestamp.

**T2.3 — Mix and match.** Pair a targets list with a snapshot that vouched for a
different one.
*Control:* `trust::check_pin` verifies both the digest **and** the version the
layer above published. Digest alone would admit a document whose version field
lies, and the rollback check reads exactly that field.

**T2.4 — Online key compromise.** A timestamp key signing a targets list.
*Control:* `check_signatures` only ever offers a role its own keys as
candidates, so a cryptographically valid signature from the wrong role cannot
count.

**T2.5 — Root key compromise / malicious rotation.**
*Control:* `accept_root_rotation` requires consecutive versions and signatures
from **both** the outgoing and incoming root. One compromised historical key
cannot jump a client to an attacker's root, because every intermediate rotation
must also verify.

**T2.6 — A hostile or misconfigured payload host.** Serving a body larger than
declared to exhaust the disk, or a truncated one.
*Control:* `download::stream_to_file` checks size **as bytes arrive**, and
truncation gets its own check because a short body never trips the "too big"
one. The digest is verified before the file is renamed into place.

**T2.7 — SSRF via a user-supplied provider URL.** A custom provider pointing at
`http://169.254.169.254/` or an internal host.
*Control (partial):* `probe::validate_provider_url` requires HTTPS except on
explicit loopback in dev mode, and bans userinfo and fragments.
**Residual — tracked:** there is no IP-literal or private-range block. A user
can still enter `https://10.0.0.5/v1`. This is arguably intended (self-hosted
gateways are a real use case) but it is a decision, not an oversight, and the
probe should be moved off any privileged network context before that changes.

**T2.8 — Credential exfiltration via redirect.** The probe sends the API key in
an `Authorization` header, and HTTP clients follow redirects by default.
*Control:* `probe::redirect_verdict` — same scheme, host and port, failing
closed on anything unparseable. Deliberately our own rule rather than trusting a
dependency's header-stripping defaults, so key safety is a property of code we
test rather than of a transitive crate's configuration.

**T2.9 — Transport-only trust.** Under ADR-008 GitHub is the sole host, so
"whoever can push to the repository" would otherwise be the root of trust.
*Control:* content signatures verified client-side. This matters **more** under
a single host, not less: withdrawing a bad release means replacing an asset, so
a client that accepted a replayed manifest would keep installing the withdrawn
build.

---

## B3 — Chimera to the managed Codex process

**T3.1 — Killing or updating something that is not ours.** Matching on
`Codex.exe` by name would reach an unrelated MSIX install or another manager's
portable copy (G5).
*Control:* `health::is_process_owned_by_runtime` compares canonicalised path
**segments**, not string prefixes. The earlier `starts_with` implementation
judged `C:/rt-evil/x.exe` as owned by root `C:/rt`.

**T3.2 — TOCTOU between the ownership check and the spawn.** A path verified as
owned, then replaced before execution.
*Control (partial):* `process::launch_managed_codex` canonicalises both sides
immediately before spawning, collapsing `..` to a real on-disk location.
**Residual:** the window between `canonicalize` and `Command::spawn` remains. On
Windows, closing it properly needs an open handle held across both. Accepted for
now because the runtime root is under the user's own profile — an attacker who
can write there can already replace the binary outright.

**T3.3 — A concurrent update corrupting the version chain.**
*Control:* `commit_version` and `rollback_to_last_known` both take
`OperationLock`, a real cross-process file lock.

**T3.4 — A crash mid-update leaving an unbootable runtime.**
*Control:* the write-ahead journal in `update.rs`. Every destructive step is
preceded by a phase record; `recover_if_interrupted` runs at startup. Before the
pointer is written the new version has been verified by nothing, so recovery
rolls back; after it, recovery completes forward.

---

## B4 — Skin package to the Codex UI

The most hostile input in the product: an attacker-supplied archive parsed
locally.

**T4.1 — Archive extraction escaping its destination.** Absolute paths, `..`,
symlinks, names that normalise differently on Windows (trailing dots, reserved
device names, alternate data streams).
*Control:* `chimera-theme::package` — `safe_join`, `validate_entry_name`, each
with its own negative test built in-test rather than committed as a malicious
archive.

**T4.2 — Decompression bomb.**
*Control:* `check_decompression_ratio`, plus declared-vs-actual size comparison.

**T4.3 — Code execution via skin content.**
*Control:* `css_allowlist` is deny-by-default over properties, and refuses
`@import`, `expression()`, `javascript:`, script-bearing `data:` URIs, and
`url()` pointing anywhere but a bundled asset. No JavaScript is permitted at all
(G9).

**T4.4 — CDP as a local attack surface.** A predictable debugging port is
reachable by any local process and gives full control of the browser context.
*Control (in progress, Step 8.2):* random free loopback port, never a fixed one,
never bound off-loopback, owned child process, cleanup on drop and abnormal exit.

**T4.5 — Modification of official Codex files.**
*Control (in progress, Step 8.3):* the required test records every official
file's bytes before apply and asserts byte equality after. Skin state lives in
Chimera's own data directory.

**T4.6 — A hostile kill switch.** A server-supplied "disable enhancement"
message is a denial-of-service primitive if unauthenticated.
*Control (in progress, Step 8.4):* an unsigned or badly-signed kill switch must
be **ignored**, not obeyed. A fingerprint mismatch disables the skin, never the
app — stock Codex must still launch with the fuse tripped.

---

## Least privilege

| Component | Has | Deliberately does not have |
|---|---|---|
| Frontend | IPC calls only | No filesystem, no network, no crate imports (V15 check 5) |
| `chimera-domain` | Pure types | No I/O at all |
| `chimera-theme` | Skin parsing, CDP | No provider access, no runtime access (adapter isolation, V15 check 1) |
| `chimera-migration` | Read-only source access | Cannot write 1.x files; no adapter-crate dependency |
| `chimera-update` | App trust domain | Cannot see the mirror's root or cache (G15) |
| Installer | Per-user `%LOCALAPPDATA%` | No administrator rights, no service, no registry outside HKCU (R15) |

## What is not defended, and why

- **An attacker who already runs code as the user.** They can read the
  credential store through the same OS API we do. Nothing here changes that.
- **A malicious upstream Codex build.** We verify the payload is the one our
  mirror promoted; we do not audit its behaviour.
- **SmartScreen and Gatekeeper warnings.** ADR-008 accepts them. Users see the
  same prompt 1.x users see today.
- **A compromised GitHub account with release permission.** They can publish an
  asset, but not a manifest that verifies — the signing key is not in the
  repository. Anti-rollback then limits replay of an old good release.

## Open items

| Item | Where | Status |
|---|---|---|
| Private-range / IP-literal policy for custom provider URLs | `probe::validate_provider_url` | Decision needed (T2.7) |
| CSP for the webview | `tauri.conf.json` | Required before any provider-supplied markup renders |
| TOCTOU window at spawn | `process::launch_managed_codex` | Accepted; revisit if the runtime root ever moves outside the user profile |
| CDP session hardening | Step 8.2 | In progress |
| Secret canary in diagnostics | Step 9.3 | In progress |
