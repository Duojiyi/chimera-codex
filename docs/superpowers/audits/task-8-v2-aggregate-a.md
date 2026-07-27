# Task 8 (T48) Audit A — Requirements Coverage (v2)

**Date:** 2026-07-26
**Scope:** Steps 8.1–8.4 (`crates/chimera-theme/` all files, `apps/chimera-desktop/src-tauri/src/skin_cmds.rs`), branch `v2`
**Auditor:** Independent A — requirements coverage (clause-by-clause against Plan/Spec, not architecture/failure-boundary review)

## Method

Read Plan `Task 8 (T48)` (lines 149–157) and Spec §11 "皮肤与非侵入增强" (lines 339–352) plus the TODO's T48 entry (line 55) and change record (lines 106–149) — the change record contains no revision touching Task 8/skins, so no clause here is VOID; every clause in Steps 8.1–8.4 is judged as written.

Read every source file in `crates/chimera-theme/src/` (`apply.rs`, `cdp_transport.rs`, `css_allowlist.rs`, `fingerprint.rs`, `package.rs`, `schema.rs`, `session.rs`, `lib.rs`), every test file in `crates/chimera-theme/tests/` (11 files), `apps/chimera-desktop/src-tauri/src/skin_cmds.rs`, and the surrounding wiring (`state.rs`, `commands.rs`, `process.rs`, `lib.rs`'s `generate_handler!`, and the frontend `features/appearance/index.tsx`) to check whether what is claimed in `skin_cmds.rs`'s module doc is actually true end to end.

Ran:
```
cargo test -p chimera-theme --locked   # 144 passed, 0 failed
cargo test -p chimera-desktop --locked # 22 passed, 0 failed
```

Broke and restored three protections to confirm they are real (each reverted; `git status --short` for the audited scope is clean — see end of report):
1. Removed the `value.contains('\\')` escape-refusal in `css_allowlist.rs` → `step8_1_css_escape.rs` went red (2 failures, `\75rl(...)` bypass succeeds again). Restored; green.
2. Reordered `apply_and_commit` back to "live push before staging" (the documented original defect) → `step8_3_divergence.rs::a_disk_failure_after_a_successful_live_push_is_impossible` went red (asserted `.b{color:blue}` live while `.a` was expected). Restored; green.
3. Added a dummy `"Runtime.evaluate"` string constant to `cdp_transport.rs` → `step8_2_no_javascript.rs::no_source_file_names_a_script_executing_cdp_method` went red. Reverted via `git checkout`.

All three fixes hold under removal — they are real protections, not tests that would pass regardless.

## Step-by-step findings

### Step 8.1 — `.codexskin` schema, path traversal, decompression bomb, MIME, CSS allowlist, no JS/executable/remote URL

**Status: MET**

- Schema (`schema.rs:24-114`): `SkinManifest::parse` is the sole constructor, schema version pinned (`SUPPORTED_SCHEMA_VERSION = 1`), `entry_css` validated against scheme/absolute/traversal/backslash/colon. 15 tests in `step8_1_schema.rs`, all pass.
- Path traversal (`package.rs:369-419` `validate_entry_name`, `306-335` `safe_join`): rejects `..`, absolute paths, backslash, colon (ADS/drive-letter), trailing dot/space (Windows path-collision trick), reserved device names (`CON`, `NUL`, ...), case-insensitive duplicate collisions. Covered by 9 dedicated tests in `step8_1_import.rs:63-180` plus 3 `safe_join` tests (`:417-436`).
- Decompression bomb (`package.rs:33-36,343-357,506-539`): three independent caps — ratio (200x), per-file (16 MiB), total (64 MiB), plus a hard streaming cap via `Read::take` that is enforced independent of declared size. `an_entry_over_the_per_file_cap_is_refused_even_at_a_normal_ratio` and `a_highly_compressible_oversized_entry_is_refused` both pass (`step8_1_import.rs:197-254`).
- MIME (`package.rs:444-469` `verify_magic_bytes`): magic-byte check per `AssetKind`, independent of extension. `a_png_extension_with_the_wrong_magic_bytes_is_refused` passes.
- CSS allowlist (`css_allowlist.rs`, all): deny-by-default property list explicitly excludes `position`/`top`/`z-index` (phishing-overlay vector), at-rules refused outright (blocks `@import`/`@font-face`/`@media`), `url()` accepted only for a bundled-asset-set membership match after normalisation. 16 tests in `step8_1_css_allowlist.rs` cover https/protocol-relative/absolute/`javascript:`/`data:`/traversal-via-`..`, all refused.
- No JS/executable/remote URL: `classify_extension` (`package.rs:424-440`) allowlists exactly 8 image/font extensions; `.js`/`.exe`/no-extension all refused (`step8_1_import.rs:320-354`). SVG script vectors (`<script>`, `on*=`, `javascript:`, `<foreignObject>`, `<iframe>`) scanned and refused (`package.rs:478-504`).
- **Defect verified fixed**: the CSS-escape bypass (`\75rl(...)` decoding to `url(` in a real tokenizer) is closed by refusing any backslash in a declaration value outright (`css_allowlist.rs:284-286`) rather than chasing escape spellings. Confirmed by breaking it (see Method).

### Step 8.2 — Random loopback CDP, owned child, target discovery, reload/reinject, exit cleanup

**Status: MET**

- Random loopback: `OsPortAllocator::allocate` (`session.rs:70-83`) binds `127.0.0.1:0`, reads back the OS-assigned port, releases it. Tests prove non-fixed (`os_port_allocator_hands_out_a_free_port_that_is_not_always_the_same`), loopback-only (`os_port_allocator_only_ever_binds_the_loopback_interface`), and no collision across two real allocations (`two_concurrent_port_allocations_never_collide`). `cdp_transport.rs:38-44,131-140` hard-codes `127.0.0.1` (not `localhost`) for both the target-list HTTP endpoint and the launch flag `--remote-debugging-address=127.0.0.1`.
- Owned child: `CdpSession`'s `Drop` (`session.rs:259-270`) kills the managed process unconditionally, verified to survive a panic mid-scope (`a_panic_while_a_session_is_in_scope_still_kills_the_process`). At the call site, `skin_cmds.rs:110-129` (`resolve_owned_codex`) re-verifies `is_process_owned_by_runtime` against the canonicalised runtime root before ever constructing a `CodexLauncher` — `CodexLauncher` itself documents that it trusts its caller for this (`cdp_transport.rs:173-184`), so the check is not skippable by construction of the call graph as wired.
- Target discovery: `discover_target` (`session.rs:199-208`) finds the first `"page"`-kind target, refuses with `NoTargetFound` otherwise (test `discover_target_refuses_when_no_page_target_exists`).
- Reload/reinject: `poll_navigated`/`is_top_level_navigation` (`cdp_transport.rs:108-124,406-430`) distinguish top-level navigation (no `parentId`) from subframe navigation, and `reinject_after_navigation` (`session.rs:244-256`) re-pushes the last-applied CSS only on a real top-level navigation (`a_navigation_causes_the_last_applied_css_to_be_reinjected`, `no_navigation_means_no_reinjection`).
- Exit cleanup: covered by the `Drop` tests above.
- **Removed JS-based CDP client, verified**: `step8_2_no_javascript.rs` source-scans every `.rs` file in `src/` for five script-executing CDP methods (`Runtime.evaluate`, `Runtime.callFunctionOn`, `Runtime.compileScript`, `Page.addScriptToEvaluateOnNewDocument`, `Page.addScriptToEvaluateOnLoad`) and asserts none appear in code (comments are stripped first). Confirmed this scanner actually catches a reintroduction (see Method, break #3). CSS is installed via `CSS.createStyleSheet`/`CSS.setStyleSheetText` (`cdp_transport.rs:298-332,379-390`), never `Runtime.evaluate`.

### Step 8.3 — try/apply/restore-default, local skin-state transaction, no writes to the official app directory

**Status: PARTIAL**

What's solid:
- No official directory is ever named: `apply.rs` takes only a caller-supplied `state_dir: &CanonicalPath`, never Codex's install or config path. `skin_cmds.rs` only ever passes `state.paths.data_root.join(SKIN_STATE_DIR)` — never `state.paths.codex_home` or `state.paths.codex_config()` (`skin_cmds.rs:92-94` vs. `state.rs:19-20,66-68`). Confirmed by the crate's own test `skin_state_is_never_written_under_a_directory_named_like_an_official_install` and `a_full_apply_try_cancel_restore_cycle_never_touches_an_unrelated_official_dir` (both pass).
- **Divergence defect verified fixed**: `apply_and_commit`'s three-phase ordering (stage → live push → publish, `apply.rs:219-266`) genuinely prevents the live session and `skin-state.json` from disagreeing after a partial failure. Confirmed by breaking it (see Method, break #2) — reverting to "live-push-first" reproduces the exact bug the regression test names.
- `try_skin`/`cancel_try`/`restore_default` are logically correct as pure `SkinStateTransaction` methods and are well tested (11 tests in `step8_3_apply.rs`, 4 in `step8_3_divergence.rs`).

Two concrete gaps, neither covered by any test:

1. **`restore_default_skin` silently no-ops across an app restart, `skin_cmds.rs:301-311`:**
   ```rust
   pub fn restore_default_skin(state: State<'_, AppState>) -> Result<(), String> {
       ...
       match runtime.txn.as_mut() {
           Some(txn) => txn.restore_default().map_err(|e| e.to_string()),
           None => Ok(()),
       }
   }
   ```
   `SkinRuntime::txn` starts `None` on every process start (`state.rs:101`, `Default` derive). If a skin was committed in a previous run (`skin-state.json` says `Applied`), and the very first skin action in the new run is "Restore Default" — a plausible first click — this hits the `None` arm and returns `Ok(())` **without ever calling `SkinStateTransaction::open` or `restore_default`**. The on-disk `skin-state.json` still says `Applied`, and the code path that would learn otherwise (`SkinRuntime::ensure`, `skin_cmds.rs:70-100`) is never invoked. A later `try_skin`/`cancel_try_skin` in the same run then opens the transaction, reads the stale `Applied` record, and `cancel_try` will re-push that skin's CSS — resurrecting a skin the user believed was already restored to default. The comment justifying this ("with no live Codex there is nothing showing a skin, so the answer is already 'default'", `skin_cmds.rs:296-300`) conflates "no live browser this run" with "nothing committed on disk," which is exactly the distinction `SkinStateTransaction` exists to preserve (its own docs, `apply.rs:161-166`: "never reflects an in-progress `try_skin`... The last *committed* state").
2. **`list_skins` reports "Default" as applied on every fresh start regardless of what is actually persisted, `skin_cmds.rs:208-217`:**
   ```rust
   let applied_name = state.skins.lock().ok()
       .and_then(|r| r.txn.as_ref().map(|t| t.current().clone()))
       ...
   ```
   Since `txn` is `None` until some action calls `ensure()`, `applied_name` is `None` on first load, so the Default entry is marked `applied: true` (`is_none()`) irrespective of the real `skin-state.json` content. The frontend's `useEffect(() => { reload(); }, [reload])` (`appearance/index.tsx:78`) calls exactly this command on mount with no other state-refresh path. A user who committed a custom skin in a prior session and reopens the app sees "Default" marked as active until they perform a Try/Apply/Cancel action in the new session — a false read, not merely a stale cache, because it is asserted as fact by the query rather than flagged as unknown.

No test in `skin_cmds.rs`'s own `#[cfg(test)] mod tests` (`skin_cmds.rs:313-348`, 3 tests, only `sanitise_id`) or anywhere else exercises `restore_default_skin`/`list_skins` against a `txn: None` + pre-existing `skin-state.json` combination — this is exactly the "a clause with no code behind it is a gap even when the crate's tests are green" pattern called out in the task brief.

A third, more severe finding for this Step: **applying/trying a skin never affects the Codex window the user actually launches.** `commands::launch_codex` (`commands.rs:311-316`) calls `launch_managed_codex` (`process.rs:46-84`), which spawns Codex with **no** `--remote-debugging-port` flag and the user's normal profile. Skin application (`skin_cmds.rs::SkinRuntime::ensure`, lines 68-100) spawns a **second, independent** Codex process via `CodexLauncher`, with CDP enabled, in a distinct profile directory (`state.paths.data_root.join("skin-profile")`, line 86) — an empty/throwaway profile, not the user's real session. Nothing in the codebase (`grep` for `skins`/`SkinRuntime` across `src-tauri/src` — see evidence below) ever connects the two: `commands::launch_codex` never checks `state.skins`, and the skin session never reuses the user's ordinary profile. The Appearance screen's own "preview" pane (`appearance/index.tsx:229-287`) is `aria-hidden="true"` static markup with hardcoded strings ("ChimeraHub", "26.721") and skeleton bars — it is not a live view of the CDP-themed window at all. Net effect: clicking "Apply" launches or reuses a hidden, separate Codex instance the user never interacts with; the Codex window they actually use via the main "Launch Codex" button is never themed. This is a functional gap in what Step 8.3 is for, even though it does not violate the literal "don't write to the official app directory" clause.

Evidence for the disconnection claim:
```
grep -rn "skin-profile|SkinRuntime|state\.skins" apps/chimera-desktop/src-tauri/src
  skin_cmds.rs:64   pub struct SkinRuntime
  skin_cmds.rs:86   let profile = state.paths.data_root.join("skin-profile");
  state.rs:101      pub skins: Mutex<crate::skin_cmds::SkinRuntime>,
  (commands.rs: no match)
```

### Step 8.4 — Versioned probe/fingerprint algorithm, negative fixtures, client-side fail-closed interpreter, fingerprint-mismatch fuse, signed skin-only kill switch

**Status: PARTIAL**

What's solid, as a library:
- `compute_fingerprint` (`fingerprint.rs:87-115`) is deterministic and order-independent (`selector_order_does_not_change_the_fingerprint`), and length-prefixes every field rather than delimiting — the **collision defect is verified fixed**: `step8_4_collision.rs` proves `["a\nb"]` vs. `["a","b"]` and boundary-adjacent selector sets no longer collide (both pass; this is a real, non-trivial property, not merely asserted).
- `ExpectedFingerprint::parse`/`validate` (`fingerprint.rs:172-199`) fail closed on malformed JSON, non-UTF-8, wrong schema version, empty version, and malformed digest shape (9 negative-fixture tests, `step8_4_fingerprint.rs:90-159`).
- `SkinFuse` (`fingerprint.rs:227-286`) has no field, method, or variant that names a process or Codex's lifecycle — `skin_enabled()` is the only externally observable effect, proven by `a_tripped_fuse_disables_only_the_skin_never_codex_launch` (a stub launch function that takes no `SkinFuse` parameter at all — the absence of a gating parameter, not a runtime check, is the proof).
- The signed kill switch (`fingerprint.rs:326-386`) verifies against a pinned trust anchor, resolves key-id to one specific key rather than trying all trusted keys (blocking cross-key replay), and is proven to leave the fuse untouched on bad signature/tampered payload/unknown key/malformed signature/malformed payload (7 tests, all pass).
- **This crate genuinely never signs anything**: no `sign_*` function, no signing-key type beyond `ed25519_dalek::SigningKey` used only in test fixtures — correctly enforced by omission per the module's own claim.

The gap: **nothing in the shipped application ever calls this machinery.**
```
grep -rn "ProbeInput|observed_selectors|compute_fingerprint" crates/ apps/
  crates/chimera-theme/src/fingerprint.rs        (definition)
  crates/chimera-theme/tests/step8_4_collision.rs
  crates/chimera-theme/tests/step8_4_fingerprint.rs
```
No production call site anywhere — not `cdp_transport.rs`, not `session.rs`, not `apply.rs`, not `skin_cmds.rs` — ever constructs a `ProbeInput` from a live CDP session (no code queries the DOM for the selectors a skin targets), computes a `CandidateFingerprint`, loads an `ExpectedFingerprint`, calls `.matches()`, constructs a `SkinFuse`, or checks `skin_enabled()` before `try_skin`/`apply_skin` push CSS. Spec §11 states plainly: "选择器或运行时指纹不匹配时，自动停用皮肤并恢复默认，不阻止 Codex 启动" ("when selectors or the runtime fingerprint mismatch, automatically disable the skin and restore default, without blocking Codex launch") — the "automatically disable and restore default" half of this sentence cannot happen today: there is no code path that ever computes a mismatch to react to. The Plan's own Step 8.4 text scopes the implementation to "in `chimera-theme`" and treats the trusted manifest as Step 4.5's output, so the algorithm/type-level work is legitimately in scope and done; but "客户端 fail-closed 解释器" (client-side fail-closed interpreter) is also explicitly named in the same Step, and an interpreter that is never invoked at runtime is not something a live skin session can fail closed against — it is inert code, exercised only by its own unit tests.

Separately (Step 8.1 interaction worth flagging under 8.4's "capability surface" framing): **bundled fonts can never take visual effect.** `package.rs` accepts and magic-byte-verifies `.woff/.woff2/.ttf/.otf` assets, and Spec §11 lists "字体" (fonts) alongside images and CSS as one of a skin's three content types. But `css_allowlist.rs` refuses every at-rule (including `@font-face`, the only CSS mechanism that binds a font file to a `font-family` name) and does not include `src` in `ALLOWED_PROPERTIES`. Verified directly:
```rust
validate_css("@font-face { font-family: \"Custom\"; src: url(font.woff2); } .x { font-family: \"Custom\"; }", &assets)
  => Err(AtRuleRefused("@font-face { font-family"))
validate_css(".x { src: url(font.woff2); }", &assets)
  => Err(DisallowedProperty("src"))
```
A skin author cannot ship a font that renders — any attempt refuses the *entire* stylesheet (all-or-nothing per `css_allowlist.rs`'s own design), not just the font declaration. This is a real, reproducible content-type gap, not a preference: one of the three content types the Spec names for a skin is completely non-functional as shipped.

## Stop conditions (verified)

| Stop condition | Verdict | Evidence |
|---|---|---|
| No official Codex file is modified | **HOLDS** | `chimera-theme` has no parameter/field naming an official install path anywhere (grep confirms); `skin_cmds.rs` only ever touches `state.paths.data_root`-relative dirs. `SkinFuse`/kill switch also cannot touch Codex (see below). |
| No arbitrary JS, no remote resource permitted | **HOLDS** | `css_allowlist.rs` blocks `@import`/at-rules/non-bundled `url()`/escape sequences; `package.rs` allowlists 8 asset extensions only, magic-byte verified, SVG script-scanned; `step8_2_no_javascript.rs` source-scans for CDP script-execution methods. All confirmed by breaking (Method #1, #3). |
| CDP binds only a random loopback port | **HOLDS** | `OsPortAllocator` (session.rs:70-83) + `--remote-debugging-address=127.0.0.1` (cdp_transport.rs:134) + hard-coded `127.0.0.1` endpoints, all with passing tests including two real concurrent allocations never colliding. |
| A fingerprint mismatch never prevents stock Codex from launching | **HOLDS, but vacuously** | `SkinFuse` cannot name a process by construction (fingerprint.rs:227-235), proven by `a_tripped_fuse_disables_only_the_skin_never_codex_launch`. However, since the fuse is never invoked in the shipped app (see Step 8.4 finding), this guarantee currently protects against a mechanism that never runs — true but not yet meaningful in production. |
| The default appearance is always restorable | **PARTIAL** | `SkinStateTransaction::restore_default` (apply.rs:273-283) is unconditional and correct when reached. But the Tauri command wrapping it, `restore_default_skin` (skin_cmds.rs:301-311), no-ops without persisting anything when no live session has been opened yet this run — see Step 8.3 finding #1. This is a genuine counter-example to "always," not a hypothetical. |

## What I could NOT verify

- **No real Codex build or real machine.** Everything above the pure-function/fake-transport layer (`cdp_transport.rs`'s actual WebSocket/HTTP calls against a real Chromium-based Codex process, `CodexLauncher::launch` actually producing a debuggable window, the real DOM selectors a real Codex build exposes) is untested against a real binary in this environment — the crate's own module docs concede this ("What remains — that a real Codex build answers these exact messages — is not something a unit test can honestly assert"). I could not confirm CDP injection actually paints the page in a live Windows session with a real Codex executable.
- **The two UI-level Step 8.3 gaps (restore-default no-op, list_skins stale read) are demonstrated by code reading and direct logical trace, not by driving the real Tauri app.** I did not have a built `chimera-desktop` binary + a managed Codex install to click through Import → Apply → restart → Restore Default end to end. The reasoning is unambiguous from the source (`skin_cmds.rs:301-311` has exactly one `match` with a `None => Ok(())` arm and no other code path touches disk in that command), but I flag this as "read, not clicked" per the task's own distinction.
- **Step 8.5 (real-machine multi-viewport verification) is explicitly out of this audit's scope** (Steps 8.1–8.4 only) and was not assessed.

## Verdict

**PASS WITH GAPS.**

Steps 8.1 and 8.2 are solidly met: every named clause (schema, path traversal, decompression bomb, MIME, CSS allowlist, no-JS, loopback-only random-port CDP, owned child, target discovery, reload/reinject, exit cleanup) has real code behind it, tested, and I confirmed three specific protections (CSS-escape refusal, live/disk-ordering, JS-execution source scan) are load-bearing by breaking each and watching the relevant test go red.

Steps 8.3 and 8.4 are PARTIAL, for reasons distinct from (and in addition to) the two defects the task description told me were already found and fixed:

- Step 8.3: the transaction primitive itself is correct (divergence bug genuinely fixed), but the Tauri command layer wrapping it has a real, unguarded gap (`restore_default_skin`'s `None`-arm no-op) that breaks the "default is always restorable" guarantee across an app restart, and a related stale-read bug in `list_skins`. Separately, and more fundamentally: applying a skin never affects the Codex window the user actually launches and uses — it themes a second, hidden, throwaway-profile process instead.
- Step 8.4: the fingerprint/fuse/kill-switch algorithm and its negative fixtures are well-built and well-tested as a library, but nothing in the shipped app ever invokes it — no probe is ever taken from a live session, so the fuse can never trip in production regardless of how well it would behave if it did. Additionally, fonts — one of the three content types Spec §11 names for a skin — cannot render under the current CSS allowlist (`@font-face` and `src` are both refused), a concrete content-type gap, not a preference.

None of these gaps are the two defects named in the task prompt (CSS escape, apply/restore divergence) or the JS-based-client removal — all three of those are verified fixed. These are additional findings from walking the Plan/Spec clauses against the actual wiring.

## Repo state

`git status --short` restricted to the audited scope (`crates/chimera-theme/`, `apps/chimera-desktop/src-tauri/src/skin_cmds.rs`) is clean — all three temporary breaks made during verification were reverted. Unrelated files elsewhere in the tree (`apps/chimera-desktop/src/features/settings/index.tsx`, i18n files, `crates/chimera-migration/...`) show as modified/untracked in the full `git status`; these were not touched by this audit and appear to belong to a concurrent process, per the task's own warning that this is a live shared repo.
