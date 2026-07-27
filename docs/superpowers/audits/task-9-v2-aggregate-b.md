# Task 9 (T49) Audit B — Architecture and Failure Boundaries (v2)

**Date:** 2026-07-26
**Scope:** Steps 9.1–9.4, `crates/chimera-update/` (all), `docs/architecture/threat-model.md`
**Auditor:** Independent B — stop conditions, failure paths, concurrent access, wiring; not spec-coverage checklisting

> **Status: FAIL**

## Method

Read every source file in `crates/chimera-update/src/` and every test file in
`crates/chimera-update/tests/`, plus `docs/architecture/threat-model.md` in
full. Cross-checked every load-bearing claim in the threat model against the
code it cites, including two files outside my nominal scope
(`crates/chimera-runtime/src/update.rs`, `apps/chimera-desktop/src-tauri/src/{runtime_cmds,state}.rs`)
where the threat model or the atomic-store's own doc comments made a direct,
checkable claim about them.

Ran, for real:

```
cargo test -p chimera-update --locked        # 98 passed, 0 failed
cargo test -p chimera-runtime --test step6_3_download --locked   # 15 passed (context check only)
```

For every protection reported below as "verified by breaking," I edited the
source, ran the exact test(s) that name it, confirmed red, then reverted the
edit and re-ran to confirm green and `git diff --stat` empty for that file.
Evidence table at the end.

**Note on repo concurrency:** this is a live, shared repository. During this
session `git status --short` showed in-flight edits to
`crates/chimera-runtime/src/download.rs` and `crates/chimera-update/src/trust.rs`
that I did not make (including a transient `if false &&` mutation of the
size-check in `download.rs`, and a transient diff in `trust.rs`), which
resolved themselves between consecutive `git status` calls — almost certainly
another auditor session independently running the same "delete the protection,
confirm red" exercise this brief requires. I did not attribute any of that
transient state to a finding; every finding below is confirmed against
`git show HEAD:<path>` and a fresh `Read` immediately before being written up,
and independently reproduced by my own edit/test/revert cycle where marked
"verified by breaking." `git status --short` is clean of my own changes as of
the end of this audit (checked below).

---

## Verdict rationale

Task 9's own stop condition (Plan §Task 9) is: **app/payload state
cross-use, update downgrade, diagnostic leak, PR obtaining a production
secret, or a security check bypassable from the UI → stop.** Two of those
have a concrete, reproduced instance in the current code, and a third
(the release-policy gate on the placeholder root) is not exercised by any
test through the code path a real caller actually uses. Combined with the
fact that **no other crate in the workspace depends on `chimera-update` at
all**, I cannot sign off Step 9.5 as satisfied. This is not a "some tests are
missing" gap; it is that the feature Task 9 exists to build has no caller
anywhere in the shipped binary, and one of the two live defects
(the redaction blind spot) sits exactly on the seam that would matter the
moment someone does wire it up.

---

## Finding 1 — `chimera-update` has no caller anywhere in the workspace

**Severity: blocking.** `crates/chimera-update` is a workspace member with 98
green tests, but grepping every `Cargo.toml` in the repository for
`chimera-update` finds it only in the workspace member list and in its own
package manifest:

```
$ grep -rn "chimera-update" --include=Cargo.toml .
./Cargo.toml:14:  "crates/chimera-update",
./crates/chimera-update/Cargo.toml:2:name = "chimera-update"
```

`apps/chimera-desktop/src-tauri/Cargo.toml` — the only binary this workspace
ships — depends on `chimera-domain`, `chimera-provider`, `chimera-migration`,
`chimera-theme`, `chimera-runtime`, `chimera-platform`. Not `chimera-update`.
`grep -rln "chimera_update" --include=*.rs .` matches only files under
`crates/chimera-update/tests/` itself.

Consequence: `trust::verify_chain`, `atomic::AtomicStore`, `redact::redact`,
and `diagnostics::build_bundle` execute today **only inside this crate's own
test binaries.** `apps/chimera-desktop/src-tauri/src/runtime_cmds.rs:179`
(`apply_codex_update`) is an explicit stub that unconditionally returns
`Err("Updating is not enabled in this build...")` — there is no live call
site this crate could even be reached from yet. The mirror side is the same
shape: `services/mirror-contract` (which owns the Codex-payload
`TrustAnchor`/`SignedManifest`) has zero consumers outside its own crate
either — `chimera-runtime::download` (the module the threat model names as
where "the trust decision" for the Codex payload moves to) does not depend on
`mirror-contract` and does not construct a `TrustAnchor`.

This matters for every one of my assigned stop conditions:

- **"App state and Codex payload state never share a key, path, cache or
  trust root"** — true as coded (see Finding 6), but it is currently true of
  two trust chains that are both dormant, not two chains that are both live
  and mutually isolated in production.
- **"No security check bypassable from the UI"** — vacuously true: there is
  no UI path that reaches any check in this crate at all, in either
  direction. The self-update feature that Task 9 (T49) is titled for does not
  exist in the running app yet.
- The TODO's own status line ("T49. 代码完成，缺正式信任根与安全审计" — code
  complete, missing a production trust root and security audit)
  undersells this: it is not "missing a formal root," it is "missing being
  called by anything."

## Finding 2 — the one function a real caller would use is untested, and I proved it can regress silently

**Severity: blocking.** `trust::verify_chain` (the public, documented entry
point — `verify_chain_with_policy` is explicitly commented "the only reason
to call this directly is to assert release behaviour from a test") selects
its policy via:

```rust
// crates/chimera-update/src/trust.rs:120-124
pub fn for_this_build() -> Self {
    Self {
        allow_development_root: cfg!(debug_assertions),
    }
}
```

I grepped every test in `tests/trust.rs` for calls to bare `verify_chain(`
(13 call sites) versus `verify_chain_with_policy(` (4 call sites, all in the
"development root must never ship" test block at the bottom of the file).
**Every single test that exercises the development-root refusal calls
`verify_chain_with_policy` with a hardcoded `TrustPolicy::RELEASE` struct
literal — none of them go through `for_this_build()`'s `cfg!(debug_assertions)`
selection at all.**

Verified by breaking: I replaced `for_this_build()`'s body with a hardcoded
`allow_development_root: true` (i.e. simulated the selection logic itself
regressing — as if a future edit inverted or dropped the `cfg!` check) and
ran the full crate test suite.

```
cargo test -p chimera-update --locked
# 98 passed, 0 failed
```

**All 98 tests still pass.** This is exactly the pattern the brief asked me
to hunt for: a protection whose removal breaks nothing is not a protection.
The module's own doc comment argues correctly that a bare
`cfg!(debug_assertions)` branch *inside the verifier* would be untestable —
and then puts an equivalent, equally untestable `cfg!` branch one layer up,
inside the function real code is supposed to call instead. Reverted; `git
diff --stat` empty, `cargo test -p chimera-update --test trust` back to 18/18.

Separately, and more mundanely: no `[profile.release]` section exists
anywhere in the workspace `Cargo.toml`, so today `cargo build --release`
does produce `debug_assertions == false` and the gate does hold by Cargo's
defaults. But that is an unstated, unenforced assumption — a
`debug-assertions = true` override in `[profile.release]` (a real practice,
sometimes added to keep `debug_assert!` diagnostics live in production) would
silently reopen this with zero test signal, because no test can observe a
`cfg!(debug_assertions)` branch flip under an actual release compile from
inside `cargo test`.

## Finding 3 — the redaction scanner has a real, reproduced blind spot for the product's own primary secret shape

**Severity: blocking** (this is the "diagnostic leak" stop condition,
directly). `redact.rs` recognises exactly four things: URL userinfo, email
local-parts, four fixed token prefixes (`sk-`, `ghp_`, `gho_`,
`github_pat_`) above a minimum length, and JWTs (three base64url segments,
first starting `eyJ`). Every test in `tests/redact.rs` and the canary test in
`tests/diagnostics.rs` uses one of exactly these shapes.

Chimera's own spec (§3, product description) makes **custom providers with
an arbitrary `URL + Key`** — no required prefix, no required shape — the
primary way a user adds a credential; ChimeraHub is the only *templated*
provider. A generic custom-provider key (e.g. a 64-char hex string, a
base64 blob without `+`/`/`, a vendor's own opaque token format) matches none
of `redact.rs`'s four patterns.

Verified by breaking (added, ran, reverted — no lasting diff):

```rust
#[test]
fn zzz_probe_generic_custom_provider_key_is_not_a_known_shape() {
    let generic_key = "a1b2c3d4e5f67890a1b2c3d4e5f67890a1b2c3d4e5f67890abcdef1234567890";
    let input = format!("provider rejected key {}", generic_key);
    let out = redact(&input);
    assert!(!out.contains(generic_key), "generic custom-provider key survived redaction: {out}");
}
```

```
PROBE OUTPUT: provider rejected key a1b2c3d4e5f67890a1b2c3d4e5f67890a1b2c3d4e5f67890abcdef1234567890
thread '...' panicked: generic custom-provider key survived redaction
```

The key comes through **completely unredacted**, and `contains_secret()`
(the function the UI is supposed to gate "send" on) would report this bundle
as **clean**, because it calls the same `redact()` — by the module's own
design ("a detector that saw something the redactor did not remove would
report a bundle as clean when it is not"), a blind spot in `redact` is
automatically a blind spot in the canary check too. This is not currently
leaking in production only because of Finding 1 (nothing calls this module
yet); the moment diagnostics gets wired to a real error path that can embed
a raw provider response or a raw key (e.g. `chimera-provider::probe`'s
"never logged" comment at `probe.rs:268` is a design intent, not something
enforced by a type — nothing stops a future error variant from carrying the
raw key text), this exact gap is live.

`scripts/verify-no-secrets.mjs` (V12, the static repo-wide scanner) has the
identical class of blind spot independently — `SECRET_PATTERNS` is the same
kind of fixed allowlist (OpenAI key, Bearer token, PEM header, URL creds, AWS
key, `.env` assignment) — so it provides no backstop for a leaked generic
custom-provider key either. Both layers of "does this look like a secret"
in this codebase share the same design limitation.

## Finding 4 — the atomic-store crate built for Step 9.2 is adopted nowhere, and the real settings path reproduces exactly the failure mode it was built to prevent

**Severity: high, not blocking on its own, but corroborates Finding 1.**
`chimera-update::atomic::AtomicStore<T>`'s own doc comment names the property
it exists to guarantee: *"never fall back to a default value, and never
panic"* on a corrupt primary — recover from the one `.bak` generation
instead. I verified this genuinely holds (see evidence table: breaking the
tmp+rename staging turns two tests red).

The app's actual settings persistence does not use this type at all. It is
hand-rolled in two places:

- `apps/chimera-desktop/src-tauri/src/runtime_cmds.rs:194-232`
  (`get_settings`/`save_settings`/`reset_settings`) — tmp+rename, **no
  `.bak`, no schema-version envelope**.
- `apps/chimera-desktop/src-tauri/src/state.rs:147-152`
  (`AppState::settings()`) —
  `std::fs::read_to_string(...).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()`.

A corrupt `settings.json` (bit rot, a hand-edit, a different process writing
to the same path) is **silently replaced by defaults** — precisely the
outcome `atomic.rs`'s own doc comment calls out as the one thing "the only
fail-closed outcome" must never allow. Spec §13 requires "atomic rename and
previous-generation backup" for exactly this document class; the crate that
implements it exists and works, but ships next to code that reimplements a
weaker version of it and is the one that actually runs.

For context only (out of my file scope, not scored against Task 9, and
included only because it is the concrete real-world shape of the same
pattern): `chimera-runtime::update::write_current_pointer` (lines 108-118)
is the identical tmp+rename-with-no-backup shape for `current.json`, and
`runtime_cmds.rs:27` (`state.runtime.read_current_pointer().ok().flatten()`)
swallows a `PointerCorrupt` error into `None`, so a corrupted install and a
never-installed one render identically to the user today. I mention this
only as evidence that the pattern in Finding 4 is not academic — it is the
same shape the workspace has already produced once, independently, in the
one place a Step-9.2-style store would have prevented it.

## Finding 5 — what does hold, verified by breaking

The following are real, load-bearing, and I confirmed each by deleting the
protection, watching the named test(s) go red, then restoring:

| Protection | File:line | Test(s) | Result of deletion |
|---|---|---|---|
| Development root refused when `TrustPolicy::RELEASE` is passed explicitly | `trust.rs:252-254` | `a_release_build_refuses_the_development_root`, `the_refusal_happens_before_any_signature_is_checked` | Both turn red; dev root is accepted and reported as a `Signature` error from a downstream check instead |
| `AtomicStore::write`'s tmp-then-rename staging | `atomic.rs:245-251` | `a_write_that_cannot_stage_leaves_the_previous_content_intact`, `a_write_that_cannot_stage_does_not_create_the_document_at_all` | Both turn red — a write that cannot stage silently destroys the previous document instead of being reported |
| Downgrade authorisation must name the *exact* installed version | `app_target.rs:189-193` | `a_downgrade_authorisation_for_a_different_version_does_not_apply` | Turns red — a downgrade authorised for version 1.0.0 is silently accepted on a 1.1.0 install |

Also read (not mutated, since already well-covered by adversarial tests
documented in the source itself): rollback/freeze/mix-and-match/wrong-role-key
checks in `trust.rs` (`check_rollback`, `check_expiry`, `check_pin`,
`check_signatures`) each have a dedicated attack-shaped test, and root
rotation (`accept_root_rotation`) requires consecutive versions signed by
*both* the outgoing and incoming root — correctly rejects a skipped version
or a rotation signed by only one side.

## Finding 6 — trust-domain isolation between the app chain and the Codex mirror (G8/G15)

**Holds, as coded.** `metadata::APP_TRUST_DOMAIN = "chimera-app-update.v1"`
is a hardcoded constant (not configurable — a config-file value could be
edited to match whatever the caller happens to be fetching) checked in every
`parse_*` function *before* a single signature is inspected
(`metadata.rs:230-257`), and `cache::APP_TRUST_CACHE_DIRNAME =
"chimera-app-trust"` is a fixed subdirectory name always joined onto
whatever base path a caller supplies, so even a wiring mistake that pointed
this crate at the mirror's own base directory would still land in a sibling
directory, never the same file. `signature.rs`'s own doc comment explains
why it is a deliberate duplicate of `mirror-contract::signature` rather than
a shared dependency — confirmed `services/mirror-contract/src/signature.rs`
is in fact a separately-implemented, non-identical module (different types:
`SignedManifest`/`TrustAnchor` vs. this crate's `SignedPayload`/`RootMetadata`).
`services/mirror-contract`'s own manifest shape (`MirrorManifest`:
`schema_version`/`channel`/... in `manifest.rs`) has no `domain` field at all
and no structural overlap with the four TUF role types this crate parses, so
a mirror document could not satisfy `parse_root`/`parse_targets`/etc. even
without the domain check — the domain check is real defense-in-depth, not
the only thing standing between the two domains.

Caveat restated from Finding 1: this isolation currently separates two
trust chains, neither of which has a live caller.

## Finding 7 — `UpdateCache`'s four-file write has no cross-process lock

**Severity: low.** `cache.rs`'s `write_root`/`write_timestamp`/
`write_snapshot`/`write_targets` each do an independent tmp+rename, but there
is no lock or transaction across the four files as a set, and no test in
`tests/cache.rs` exercises concurrent writers. Two concurrent update-check
runs (once this crate is ever wired to run more than once, e.g. a manual
check racing a scheduled one) could interleave which of the four cached
documents comes from which fetch cycle. I judge this low severity rather
than blocking because `trust::verify_chain`'s pin checks (`check_pin`
between snapshot↔targets and timestamp↔snapshot) would still refuse an
internally-inconsistent set fed back through — the likely outcome of the
race is a spurious refusal/retry on the next check, not a security bypass.
Still a genuine, untested concurrent-access boundary and worth a fixture
before this cache sees concurrent real use.

## Step-by-step

- **Step 9.1 (TUF chain, bundled root).** Logic is correct and
  well-tested for every case a test actually drives through
  `verify_chain_with_policy` with an explicit policy (Findings 5, 6). The
  automatic build-based policy selection a real caller would use
  (`for_this_build`, Finding 2) is not exercised by anything. Not reachable
  from any binary (Finding 1).
- **Step 9.2 (atomic settings/ownership/transaction).** `AtomicStore<T>`
  itself is correct (Finding 5). Adopted by zero real documents in the
  shipped app; the app's actual settings path reproduces the exact failure
  mode it exists to prevent (Finding 4).
- **Step 9.3 (redaction, diagnostics, secret canary).** Idempotency,
  structure-preservation, and the "canary reaches nothing" property all hold
  — for the four secret shapes the module recognises. Does not generalise to
  the product's own primary custom-key shape (Finding 3). The threat model's
  own open-items table already flags this area "In progress," which this
  audit substantiates with a specific, reproduced failure rather than
  contradicts.
- **Step 9.4 (threat model, least-privilege).** Read in full. Every claim I
  could check against code in `crates/chimera-update` matched. The two
  claims that reach outside this crate's files (`T3.3`/`T3.4`, citing
  `chimera-runtime::update`'s `OperationLock` and write-ahead journal) I
  independently re-verified against current source rather than trusting the
  document or any prior audit, and found accurate: `commit_version` (line
  243), `recover_if_interrupted` (line 321), and `rollback_to_last_known`
  (line 360) do each take `OperationLock::try_acquire` around their critical
  section, and a `TransactionPhase`-tagged journal (`Started` → `OldAsided`/
  `Installed` → `Committed`) does drive `recover_if_interrupted`'s
  asymmetric rollback-vs-complete logic. The document's Least-Privilege
  table's `chimera-update` row ("Cannot see the mirror's root or cache") is
  accurate per Finding 6. The document does **not** state anywhere that this
  entire crate has no caller (Finding 1) — that is the most material fact
  about Task 9's actual security posture missing from the threat model as
  written.

## The development trust root — explicit answer to the audit note

`bundled_root::development_root()` is unambiguously, repeatedly
self-labelled: key id
`"chimera-dev-insecure-DO-NOT-SHIP-root-1"`, a fixed public seed
(`DEV_INSECURE_ROOT_SEED = [0x44; 32]`) checked into source with a doc
comment stating plainly that anyone who can read the repository can forge a
root the same code would accept. `is_development_root` keys off the key id
specifically (not version/expiry, which a real root could coincidentally
match) and I confirmed `trust::verify_chain_with_policy` does refuse it when
given `TrustPolicy::RELEASE` (Finding 5's table, row 1).

**Answering the note directly: yes, it is refused under the release policy
when that policy is passed explicitly — but the only function a real caller
is expected to invoke (`verify_chain`, which selects the policy via
`for_this_build()`) has no test proving that selection actually resolves to
`RELEASE` under a real release build, and I demonstrated the selection logic
can be silently broken with zero test failures (Finding 2).** Regardless of
that gap, and regardless of Finding 1 (nothing calls any of this yet): this
key must never reach a release build under any circumstance, and **replacing
it with a root from a real offline key ceremony, generated and stored outside
source control, is a release blocker** — full stop, independent of whether
the wiring or test-coverage gaps above are also closed first. This matches
R4 in the TODO (still open) and the module's own stated intent; I am stating
it here as required by the audit brief, not as a novel finding.

## What I verified by breaking (summary table)

| # | File | Mechanism broken | Test(s) that went red | Restored? |
|---|---|---|---|---|
| 1 | `trust.rs:252` | Dev-root refusal (`!policy.allow_development_root && ...`) | `a_release_build_refuses_the_development_root`, `the_refusal_happens_before_any_signature_is_checked` | Yes — `git diff --stat` empty |
| 2 | `atomic.rs:245-251` | tmp+rename staging (replaced with direct `File::create(&self.path)`) | `a_write_that_cannot_stage_leaves_the_previous_content_intact`, `a_write_that_cannot_stage_does_not_create_the_document_at_all` | Yes — `git diff --stat` empty |
| 3 | `app_target.rs:189-193` | Exact-version downgrade authorisation | `a_downgrade_authorisation_for_a_different_version_does_not_apply` | Yes — `git diff --stat` empty |
| 4 | `trust.rs:120-124` | `for_this_build()`'s `cfg!(debug_assertions)` selection (hardcoded to always allow dev root) | **None — all 98 tests still passed** | Yes — `git diff --stat` empty |
| 5 | `redact.rs` (no code change — added-then-removed test only) | Generic/prefix-less secret detection | New probe test failed as expected (proving the gap, not a regression) | Yes — file identical to `git show HEAD` |

## What I could NOT verify, and why

- **Whether `chimera-update` will actually be wired to `apps/chimera-desktop`
  before release**, and whether the wiring (when it happens) will call
  `verify_chain` or `verify_chain_with_policy` — this is future work with no
  code yet to inspect.
- **Real network behaviour** — `fetch::MetadataFetcher` is a trait with no
  production HTTP implementation in this crate; every test is offline by
  design. I could not exercise a real mirror, a real TLS boundary, or a real
  redirect against the app's own update endpoint (there isn't one yet).
- **A real release build's `cfg!(debug_assertions)` value** — confirmed no
  `[profile.release]` override exists today (so the default holds), but I
  did not build an actual `--release` binary and call into it; that would
  require a build environment beyond what this audit's Rust unit tests
  exercise, and — per Finding 2 — no test in the suite does this either.
- **Whether a real credential ever actually reaches `last_error` /
  `recent_log_lines` in production** — `chimera-provider::probe.rs:268`
  documents that the key is "never logged" as an intent, but I did not audit
  every error-construction path in `chimera-provider` for a raw key
  reaching a `String` that could flow into `DiagnosticInput`; that crate is
  outside my assigned scope. Finding 3 stands regardless, as a defect in
  `redact()` itself, independent of whether anything currently feeds it a
  leaking string.

## Final check

```
$ git status --short -- crates/chimera-update/
(empty)
$ cargo test -p chimera-update --locked
98 passed; 0 failed
```

My own working tree is clean for everything in scope. Other in-flight,
unrelated changes from concurrent sessions (`apps/chimera-desktop/src/features/settings/index.tsx`,
`apps/chimera-desktop/src/i18n/{en,zh}.ts`, `crates/chimera-runtime/src/download.rs`,
untracked files under `crates/chimera-migration/tests/` and
`docs/superpowers/audits/`) are not mine and are left untouched.
