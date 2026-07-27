# Task 9 (T49) Audit A — Requirements Coverage

**Date:** 2026-07-26
**Scope:** Step 9.1–9.4 (`crates/chimera-update/` in full, `docs/architecture/threat-model.md`), branch `v2`, commit `5a04026` (chimera-update) + `433c8b0` (threat model)
**Auditor:** Independent A — requirements coverage only (Spec/Plan clause-by-clause vs. shipped code; no diff/boundary review)

## Method

Read every source and test file in `crates/chimera-update/` end to end (11 modules, 11 test files, 4,203 lines). Read the Plan's Task 9 section, Spec §8.4/§9, and the TODO's change record in full before judging anything — no revision applies to Task 9's Steps or stop conditions. Ran:

```
cargo test -p chimera-update --locked        # 98 passed, 0 failed
```

For every control cited as "MET" below, I deleted or inverted the guard in source, reran the targeted test, watched it go red, then restored the exact original text and reran the full suite to confirm 98/98 green again. Four such fault injections are recorded under "Verified by breaking." `git status --short` is clean at the end of this audit; `crates/chimera-update/src/*.rs` shows no diff. (Note: this is a live shared repo — a concurrent session, self-identified in a code comment as "AUDITOR-B", transiently modified `trust.rs` during this session and reverted it themselves; that edit is not mine and is not reflected in the final clean status.)

## Cross-cutting finding, load-bearing for every Step below

**`chimera-update` is not called from anywhere except its own tests.**

```
grep -rln "chimera_update" --include="*.rs" .   # only crates/chimera-update/ itself
grep -rn "chimera-update" --include=Cargo.toml . # only the workspace root and the crate's own manifest
```

`apps/chimera-desktop/src-tauri/Cargo.toml:19-29` does not depend on `chimera-update`. `AppState` (`apps/chimera-desktop/src-tauri/src/state.rs:88-102`) has no field for an `UpdateCache`, no `Paths` accessor for the app-trust directory, and `lib.rs`'s `generate_handler!` (`apps/chimera-desktop/src-tauri/src/lib.rs:56-95`) registers no self-update command at all — contrast with `runtime_cmds::apply_codex_update` (Codex payload update), which *is* registered and returns a hard-coded refusal (`runtime_cmds.rs:178-185`). There is no Chimera-app equivalent, not even a stub.

This matters because the Plan's Task 9 header names the target files as "Chimera self-update **crate/commands**" (plan line 161) — commands, plural, alongside the crate. Task 10 (T50)'s five steps (alpha → beta → gray rollout → audit → stable) presuppose a working end-to-end updater and contain no step to wire one up, so this is not deferred work — it is a gap in Task 9's own scope. Everything reported "MET" below is therefore true of the *library*, verified in isolation; none of it is exercised by the shipped application today.

---

## Step 9.1 — Independent TUF-style chain, bundled root, rotation/revocation, downgrade authorization

**Asked:** root/targets/snapshot/timestamp metadata; built-in initial root; threshold/expiry/monotonic version; consecutive root rotation; online key revocation; independent cache; `chimera-app-latest.json` as a pinned target only; downgrade requires explicit current-metadata authorization. Red must cover: offline, expired/freeze/rollback, unknown/skipped root, corrupt cache, normal **and forced** update, disaster downgrade, and app/payload domain cross-contamination.

**Verdict: PARTIAL**

| Clause | Status | Evidence |
|---|---|---|
| Four-role metadata shapes | MET | `metadata.rs:76-176` (`RootMetadata`/`TimestampMetadata`/`SnapshotMetadata`/`TargetsMetadata`) |
| Built-in initial root | MET (dev placeholder, correctly labelled) | `bundled_root.rs:38-126`; refusal path below |
| Threshold / expiry / monotonic version | MET | `metadata.rs:115-134` (`validate_shape`, zero-threshold refusal); `trust.rs:131-151` (`check_expiry`, `check_rollback`); tests `trust.rs:227-311` |
| Consecutive root rotation | **Built but untested** | `trust.rs:327-350` `accept_root_rotation` requires `candidate.version == current.version + 1` and signatures from **both** roots. `grep -rn "accept_root_rotation" crates/chimera-update/` returns only the definition and one doc-comment mention — zero call sites in any of the 11 test files. |
| Online key revocation | Implicit only | No dedicated revoke function or test; revocation is achievable only as a side effect of a root rotation that drops a key from a role's list, and that path is the same one that is untested above. |
| Independent cache | MET | `cache.rs:32` `APP_TRUST_CACHE_DIRNAME = "chimera-app-trust"`, joined onto caller's base dir at `cache.rs:84-88`, never caller-configurable |
| `chimera-app-latest.json` as pinned target only | MET | `app_target.rs:144-174`: length → digest → parse, in that order, against the signed targets map — never a self-authenticating document |
| Downgrade requires explicit authorization | MET, verified by breaking | `app_target.rs:188-206` |
| Red: offline | MET (scope-appropriate) | `fetch.rs` trait + `tests/fetch.rs:31` — this crate does no real I/O by design, so a mocked `FetchError::Offline` is the correct level |
| Red: expired/freeze/rollback | MET | `tests/trust.rs:192-338` (9 tests) |
| Red: unknown/skipped root | **NOT MET** | See "consecutive root rotation" row — the one code path that would be exercised by this scenario has no test |
| Red: corrupt cache | MET | `tests/cache.rs:54-68` `a_corrupt_root_file_fails_closed_instead_of_being_treated_as_absent` |
| Red: normal **and forced** update | **NOT MET** | `grep -rin "force" crates/chimera-update/src crates/chimera-update/tests` matches nothing except unrelated prose ("force the client back to…"). There is no forced-update concept, field, or code path anywhere in the crate. Spec §8.4 is explicit that a forced/minimum-version update "must not become a backdoor" around signature/expiry/monotonicity checks — there is nothing here to backdoor because there is nothing here at all. |
| Red: disaster downgrade | MET | `tests/app_target.rs:204-231` (`downgrade_authorized_from`, plus the "for a different version does not apply" negative case) |
| Red: domain cross-contamination | MET, verified by breaking | `tests/trust.rs:509-533` |

### Verified by breaking (Step 9.1)

1. **Domain gate.** Changed `check_domain` (`metadata.rs:216-224`) to always return `Ok(())`. Reran `a_document_from_the_codex_mirror_domain_cannot_satisfy_the_app_chain` (`tests/trust.rs:510`) — it went from pass to a panic: `unwrap_err() on an Ok value`, with the accepted chain's root domain literally printed as `"codex-mirror.v1"`. Restored; 98/98 green again.
2. **Downgrade authorization.** Forced `authorised = true` unconditionally in `app_target.rs:188-206`. Two tests went red: `a_lower_version_with_no_authorisation_is_a_refused_downgrade` and `a_downgrade_authorisation_for_a_different_version_does_not_apply` (the latter is important — it proves the code checks the authorization is for *this exact* installed version, not merely present). Restored.
3. **Development-root release refusal.** Deleted the `if !policy.allow_development_root && is_development_root(...)` gate in `trust.rs:249-254`, replacing it with a no-op. `a_release_build_refuses_the_development_root` and `the_refusal_happens_before_any_signature_is_checked` both failed — the placeholder root, which is internally self-consistent and self-signed, sailed through full chain verification. Restored.
4. **Root-rotation logic (functional check, not a regression test)** — since the shipped suite has none, I wrote a throwaway test file (`tests/zz_audit_scratch_root_rotation.rs`, deleted after use, not part of the diff) exercising `accept_root_rotation` directly: a root that skips a version is rejected, a candidate missing the outgoing root's signature is rejected, and a valid consecutive rotation signed by both keys is accepted. All three passed — the *function itself* is correct. The finding above is specifically that this correctness is unverified by anything the crate ships.

### Note on the compiled-in trust root (per audit NOTE)

`bundled_root.rs:12-36` labels itself unambiguously: `DEV_INSECURE_ROOT_SEED` is a fixed, source-visible byte pattern (`bundled_root.rs:51`), the key id is `"chimera-dev-insecure-DO-NOT-SHIP-root-1"` (line 56), and the module doc states outright it "would be a total compromise of every install if it ever shipped." I confirmed by fault injection (#3 above) that `TrustPolicy::RELEASE` (`trust.rs:114-116`, `allow_development_root: false`) does in fact refuse this root when the gate is present, and that removing the gate silently accepts it. **This refusal is real and correctly wired inside the crate.** But per the cross-cutting finding, `TrustPolicy::for_this_build()`/`verify_chain` is never called from `apps/chimera-desktop` at all — there is no startup path today that could invoke this refusal in a real build, because there is no startup path that invokes the chain at all. **Replacing this development root with a real offline-ceremony root, and wiring the release-policy refusal into `apps/chimera-desktop`'s actual startup, is a release blocker for 2.0.0 — stated plainly, as instructed.** This is already self-acknowledged in the module doc (`bundled_root.rs:34-36`) and in the TODO (`T49. 〔代码完成，缺正式信任根与安全审计〕`), so this audit is confirming a known gap, not discovering a hidden one.

---

## Step 9.2 — Atomic writes, schema migration, `.bak` recovery, SQLite consistency/backup for settings/ownership/transaction

**Asked, verbatim:** "实现 settings/ownership/transaction 的原子写、schema migration、`.bak` 恢复和 SQLite consistency/backup" — atomic writes, schema migration, `.bak` recovery **for settings/ownership/transaction**, plus SQLite consistency/backup.

**Verdict: PARTIAL, and the gap is specifically in the three named subjects**

`atomic.rs` itself is genuinely well-built: `AtomicStore<T>` (`atomic.rs:173-253`) does tmp-file-then-fsync-then-rename (atomic), copies the existing primary to a single `.bak` generation before every write (`atomic.rs:239-243`, recoverable), and every document carries a `schema_version` in an `Envelope` that is upgraded via the `Migratable` trait or refused outright if it is newer than this binary understands (`atomic.rs:87-154`, migratable). All three properties are individually tested (`tests/atomic.rs`: corrupt-primary-falls-back-to-backup at line 91, schema-upgrade-on-read at line 173, future-schema-refused at line 195, single-backup-generation-kept at line 117, staging-failure-leaves-original-intact at line 254).

**None of this reaches the actual settings, ownership, or transaction files the app writes:**

- **Settings.** `save_settings` (`apps/chimera-desktop/src-tauri/src/runtime_cmds.rs:207-221`) does its own tmp+rename (no `AtomicStore`), writes a bare `SettingsDto` with no `schema_version` envelope, and creates no `.bak`. `get_settings` (`runtime_cmds.rs:194-203`) treats an unparseable file as "use built-in defaults" (`serde_json::from_str(&text).unwrap_or_default()` at dto.rs level) — a corrupt settings file is silently *discarded*, not recovered from a backup, because no backup exists.
- **Ownership.** `write_ownership_manifest` (`crates/chimera-runtime/src/detection.rs:146-178`) builds its own `serde_json::json!{...}` (lines 165-173) with no `schema_version` key at all, and does a bare tmp+rename (lines 175-177) — no `.bak`. `InstallOwnership` itself (`crates/chimera-domain/src/ownership.rs:39-49`) has no schema-version field to migrate from.
- **Transaction.** `chimera-provider/src/transaction.rs`'s journal writer (`write_journal`, lines 185-193) is the same pattern: tmp+rename, no backup, no schema tag.

`grep -rn "\.bak\b" --include=*.rs crates/ apps/ services/` returns hits only in the legacy 1.x product (`codex-plus-core`, `codex-plus-data`) — none in any v2 crate outside `chimera-update` itself.

**SQLite:** `ProviderDb::open` (`crates/chimera-provider/src/db.rs:31-33`) sets `PRAGMA journal_mode=WAL`, a real consistency mechanism (atomic commits, crash-safe by design) — this much is genuinely present. There is no backup mechanism (no `VACUUM INTO`, no periodic copy), no `PRAGMA integrity_check` anywhere in the crate, and no test in `crates/chimera-provider/tests/` exercises recovery from a truncated or corrupt database file.

**Conclusion for this Step:** the Step's title ("implement atomic writes/schema migration/`.bak` recovery/SQLite backup **for settings/ownership/transaction**") describes a generic library module that exists and works, sitting unused, next to three real production writers that only get the "atomic" third of the requirement. This is a specific, unmet acceptance criterion, not a style preference.

---

## Step 9.3 — Error classification, log rotation, diagnostics preview, double redaction, real secret canary

**Verdict: PARTIAL**

| Clause | Status | Evidence |
|---|---|---|
| Structured error classification | MET | `diagnostics.rs:22-49`; test `classification_never_reads_the_secret_it_is_classifying` |
| Log rotation | MET | `diagnostics.rs:151-197`; 4 tests, including the "never remove the only log" edge case |
| Diagnostics preview | MET (library level) | `DiagnosticBundle::render()` (`diagnostics.rs:85-98`) |
| Double redaction | MET, verified by breaking | `build_bundle` redacts every field on ingest (`diagnostics.rs:114-132`); `render()` redacts the assembled whole again (line 97); test `redaction_is_applied_twice_and_the_second_pass_changes_nothing` |
| Real secret-canary fail-closed test | MET at the unit level, **not real in the sense of protecting anything shipped** | See below |

### Verified by breaking (Step 9.3)

Changed `looks_like_prefixed_token` (`redact.rs:58-60`) to always return `false`. `a_canary_planted_in_every_field_reaches_none_of_the_output` (`tests/diagnostics.rs:20-42`) went red, printing the literal canary string `sk-CANARYzzzz…` back out in five of six fields — a real, working, non-vacuous fail-closed test (it is itself guarded by `the_canary_test_would_fail_if_redaction_did_nothing`, `tests/diagnostics.rs:44`, so it cannot pass by having nothing to redact). Restored; confirmed green.

### Two structural gaps found beyond the fault-injection check

1. **The redactor is a fixed allowlist of known credential shapes** (`redact.rs:39` `TOKEN_PREFIXES = ["sk-", "ghp_", "gho_", "github_pat_"]`, plus JWT shape at line 47). I wrote and ran a throwaway test (deleted after use) confirming that an opaque custom-provider API key (`"4f9a2c7e8b1d3f6a0e5c9b2d7a1f4e8c"`) and an AWS-style key id (`"AKIAIOSFODNN7EXAMPLE"`) both pass through `redact()` byte-for-byte unchanged. Spec §3/D3/D4 explicitly makes an unconstrained-format API key the default custom-provider case ("自定义供应商默认只填 URL 和 Key"), so this is not a hypothetical shape — it is the documented common case, and it structurally cannot be caught by a prefix/JWT allowlist. This is a real gap against the stop condition "no diagnostic output can contain a secret": that guarantee only holds for secrets shaped like the four known token families.
2. **The canary protects a pipeline nothing calls.** `runtime_cmds::run_diagnostics` (`runtime_cmds.rs:137-140`) never constructs a `chimera_update::diagnostics::DiagnosticInput`/`DiagnosticBundle` — it returns three hard-coded `pass`/`warn`/`fail` rows (`diagnostics_for`, lines 100-116) with no free-text field at all. So today there is no live risk (there is no text to leak), but there is also no redaction actually protecting the diagnostics feature a user can click in the running app. `docs/architecture/threat-model.md:273` lists "Secret canary in diagnostics | Step 9.3 | In progress" — read charitably, that line is more accurate about the *shipped* diagnostics feature than the crate's own confident, passing unit test is.

---

## Step 9.4 — Threat model and least-privilege audit for provider/mirror/runtime/skin/update

**Verdict: PARTIAL — good document, one required omission**

`docs/architecture/threat-model.md` (274 lines) covers all five named domains via four trust boundaries (B1 webview↔Rust, B2 network↔client, B3 Chimera↔Codex process, B4 skin↔CDP), an assets table, a least-privilege table, and an explicit "what is not defended, and why" section. It is honest about several open residuals rather than claiming completeness: T2.7 SSRF is flagged "Control (partial)" with no private-range block (line 150-155); T3.2 TOCTOU is flagged as an accepted residual (lines 186-189); T4.4/T4.6 CDP hardening and kill-switch are marked "in progress" (lines 226-227, 234-238); the open-items table (lines 265-274) lists four unresolved items by name. I found no claim in this document that overstates what the code does — unlike prior findings elsewhere in this codebase, this doc appears to accurately describe its own gaps.

**What it omits, and which the audit brief explicitly asks me to confirm:** the document never mentions the compiled-in development trust root at all. `grep -n "development\|placeholder\|DEV_INSECURE\|bootstrap" docs/architecture/threat-model.md` matches nothing relevant. Given the root's own module doc calls it a "total compromise of every install if it ever shipped" (`bundled_root.rs:18-21`), and this is precisely the kind of asset a "最小权限审计" (least-privilege audit) exists to name, its complete absence from the one document whose job is to enumerate exactly this class of risk is a specific, checkable gap in Step 9.4 — not merely an omission from Step 9.1's code.

SSRF, path, TOCTOU, signature, process, and WebView boundaries are each covered (T2.7, T4.1, T3.2, B2 generally, B3 generally, B1/T4.4 respectively), satisfying the letter of "覆盖 SSRF、路径、TOCTOU、签名、进程和 WebView 边界."

---

## Stop conditions — explicit check

| Stop condition | Result |
|---|---|
| App state and Codex payload state never share a key, path, cache, or trust root | **Holds within the library** (domain tag `metadata.rs:33`, cache dirname `cache.rs:32`, duplicated signature code `signature.rs:1-14` vs. `services/mirror-contract/src/signature.rs`, verified by breaking #1 above) — **but moot in production**, because neither `chimera-update`'s cache nor its trust chain is ever instantiated by `AppState` (`state.rs:88-102`) today. There is no live path where the two could collide because one side of the collision does not run. |
| An update cannot downgrade without explicit authorization | **Holds within the library**, verified by breaking #2. Moot in production for the same reason — `apply_codex_update` is hard-refused (`runtime_cmds.rs:178-185`) and there is no Chimera-app self-update call site at all. |
| No diagnostic output can contain a secret | **Partially holds.** The canary test is real (verified by breaking). The redaction function is an allowlist that structurally misses opaque/custom-provider-shaped keys (reproduced above). The command a user actually reaches (`run_diagnostics`) does not route through this code at all. |
| No security check is bypassable from the UI | **Vacuously true, not meaningfully verified.** Nothing in `chimera-update` is reachable from any Tauri command or frontend feature, so there is no UI-exposed surface to bypass yet — this is an absence of attack surface, not a demonstrated resistance to one. |

## What I could not verify

- Behavior on a real disconnected network, a real disk-full condition, or a real power-loss mid-write — this crate is offline-by-design (`fetch.rs` doc comment, `lib.rs:4-8`) and I have no real machine, real mirror server, or real Codex build available in this session; everything above is verified against the crate's own mocked seams, which is the most this scope permits.
- Whether a genuine offline root-key ceremony process exists anywhere outside this repository — out of scope for a code audit and explicitly named as pending in the TODO.
- Any CI/security-scanner enforcement of `verify-no-secrets.mjs` (V12) against this crate specifically — not re-run in this session; scope was the crate and the threat model document, not the full V13 gate.

## Conclusion

| Step | Verdict | Primary reason |
|---|---|---|
| 9.1 | PARTIAL | Core chain verification is correct and well-tested (confirmed by breaking four separate controls); root rotation is implemented but has zero test coverage anywhere in the shipped suite; "forced update" — an explicitly named Red-list scenario — does not exist in any form |
| 9.2 | PARTIAL | Generic `AtomicStore` module delivers all four properties in isolation; the three subjects the Step names (settings, ownership, transaction) each reimplement only the atomic-rename third of it, with no schema versioning and no `.bak` anywhere in production code; SQLite gets WAL but no backup/integrity check |
| 9.3 | PARTIAL | Redaction, double-pass, and the canary test are real and verified; the canary protects a `DiagnosticBundle` pipeline nothing in the shipped app calls; the redaction ruleset is a fixed allowlist that misses the Spec's own default custom-provider key shape |
| 9.4 | PARTIAL | Threat model is honest and structurally complete against its five named domains, but omits the one asset — the development trust root — this audit was specifically asked to confirm is addressed |

**Overall verdict: FAIL.**

Every Step has at least one specific, named acceptance criterion that is unmet, not merely under-polished: no forced-update path exists at all (9.1); no test exists for root rotation despite the Plan explicitly requiring Red coverage for it (9.1); the actual settings/ownership/transaction writers get none of schema migration or `.bak` recovery (9.2); the redaction allowlist misses the Spec's own default custom-key shape (9.3); the threat model never names the development trust root (9.4). Layered on top of all four: the entire crate — a genuinely well-designed, individually well-tested cryptographic and hygiene library — has zero call sites anywhere in the shipped application, so none of the above protections currently defend anything a user's build actually runs. The TODO's own status line for T49 ("代码完成，缺正式信任根与安全审计") undersells this: the gap is not only the trust root and the audit, it is that Task 9's own stated target — "crate/commands" — never produced the commands half.

I do not recommend checking off T49 on the strength of this audit. The library-level work is a solid foundation and none of its individually-tested guarantees were found to be fake — every "MET" above survived an actual deletion-and-restore test. What is missing is integration, not correctness.
