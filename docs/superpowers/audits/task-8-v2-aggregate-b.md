# Task 8 (T48) Audit B — Architecture / Failure Boundaries (v2)

**Date:** 2026-07-26
**Scope:** Steps 8.1–8.4. `crates/chimera-theme/` (all), `apps/chimera-desktop/src-tauri/src/skin_cmds.rs`.
**Commit audited:** `6d854d0` on branch `v2`.
**Auditor:** Independent B — architecture and failure boundaries only. I did not read Auditor A's report before forming these findings, and did not consult the Spec's V-clause checklist beyond the five stop conditions named in my brief.

## Verdict: PASS WITH GAPS

All five literal stop conditions named in the brief hold, and two of them (the CSS-escape bypass and the live/recorded divergence in `apply_and_commit`) hold **because** the fixes the note pointed me at are real — I deleted each one and watched the regression test that names it go red, then restored it. But there are two defects of the same class the developers already fixed once, left unfixed in adjacent code, plus one significant integration gap:

1. `SkinStateTransaction::restore_default` can record "Default" while the live session was never actually cleared — the identical divergence bug the developers fixed for `apply_and_commit`, reintroduced (never removed) in the sibling function, with zero test coverage of the failure branch that exposes it.
2. The skin engine's CDP session and the app's actual "Launch Codex" button drive two independent Codex processes with two independent profile directories and no shared state. As wired today, applying a skin cannot visibly affect the Codex window a user opens from the Home screen.
3. Step 8.4's fingerprint/kill-switch fuse is fully implemented and unit-tested (27 tests) but is never called from anywhere in `skin_cmds.rs` — it is inert in the shipped path. This is plausibly in-scope-as-deferred per the TODO's own wording ("candidate evidence only... published by Step 4.5's mirror gate"), so I present it as a scope-aware finding, not a stop-condition violation.

None of this is a FAIL against the five named stop conditions — each holds at the mechanism level I could exercise. It is not a clean PASS either: #1 is a genuine, reproducible defect in code that ships today, and #2 means the feature, as integrated, does not do the one thing it exists to do outside of the Appearance tab's own preview.

## What I ran

```
cargo test -p chimera-theme --locked        # 143 passed, 0 failed
node scripts/verify-v2-architecture.mjs     # PASS (layering, incl. chimera-theme = L2, deps only chimera-domain/chimera-platform)
```

I read every file in `crates/chimera-theme/src/` and `tests/`, `apps/chimera-desktop/src-tauri/src/skin_cmds.rs`, `state.rs`, `commands.rs`, and the frontend `features/appearance/index.tsx`, `features/home/index.tsx`, `features/codex/index.tsx`.

## Verified by breaking

### 1. CSS escape bypass (the fix the note asked me to check) — holds

`crates/chimera-theme/src/css_allowlist.rs:284-286` refuses any declaration value containing a backslash, specifically to close a bypass where `\75rl(` decodes to `url(` in a standards-compliant tokenizer and slips a remote URL past a scanner that only matched the literal bytes `url(`.

I deleted the guard (replaced the three lines with a no-op comment) and ran `cargo test -p chimera-theme --test step8_1_css_escape`:

```
test an_escaped_url_function_is_refused ... FAILED
test every_escape_spelling_is_refused_not_just_the_one_that_was_reported ... FAILED
test result: FAILED. 3 passed; 2 failed
```

Restored the guard, reran the full suite: 143/143 green, `git diff --stat` on the file empty. The fix holds and the regression test genuinely depends on it.

### 2. `apply_and_commit` live/recorded divergence (the second fix the note asked me to check) — holds

`crates/chimera-theme/src/apply.rs:219-266`. The original defect: CSS was pushed live before anything was written to disk, so a disk failure after a successful live push left the browser showing package B while `skin-state.json` still named package A. The fix reorders to stage → push live → publish, converging to Default if publish fails after a successful live push.

I reverted the ordering (pushed live before `package.write_to(&staging)`, mimicking the pre-fix code) and ran `cargo test -p chimera-theme --test step8_3_divergence`:

```
test a_disk_failure_after_a_successful_live_push_is_impossible ... FAILED
  left: Some(".b{color:blue}")   (live)
 right: Some(".a{color:red}")   (recorded)
test result: FAILED. 3 passed; 1 failed
```

Restored the correct ordering, reran the full suite: 143/143 green, `git diff --stat` on the file empty. This fix also holds.

### 3. `restore_default`'s own divergence-on-`clear()`-failure — a live, unfixed instance of the same bug class

`crates/chimera-theme/src/apply.rs:273-283`:

```rust
pub fn restore_default(&mut self) -> Result<(), ApplyError> {
    let clear_result = self.applier.clear();
    self.current = SkinState::Default;
    self.persist_state()?;
    let _ = fs::remove_dir_all(self.state_dir.join(CURRENT_DIR));
    clear_result.map_err(ApplyError::from)
}
```

This unconditionally sets `self.current = SkinState::Default` and persists it **regardless of whether `self.applier.clear()` succeeded**. The module doc (lines 33-35) states this is deliberate: "does not consult or depend on whatever the previous operation's outcome was." That is a materially different, and materially weaker, guarantee than what `apply_and_commit`'s fix established two paragraphs above it in the same file — that function's own docs (lines 26-32) explain *why* a live push and the persisted record must never be allowed to disagree. `restore_default` reintroduces exactly that disagreement whenever the live clear fails, in the one function whose entire job is "make what's on screen match what's recorded."

Concretely, the failure is reachable, not academic. `restore_default` → `CdpSkinApplier::clear` (`skin_cmds.rs:52-56`) → `CdpSession::clear_css` (`session.rs:232-238`) → `ensure_alive()` passes (the Codex *process* is still running) → `WebSocketCdpClient::clear_css` (`cdp_transport.rs:392-404`) issues `CSS.setStyleSheetText` over the DevTools WebSocket and can fail via `self.call(...)` for any of: a dropped WebSocket that fails to reconnect (`socket_for`, `cdp_transport.rs:246-260`), a `send()` failure, or exceeding `CALL_TIMEOUT` (10s, `cdp_transport.rs:32,279-296`) if the renderer is momentarily unresponsive. None of these require the Codex process itself to have died — a wedged renderer or a transient socket reset is enough, and `ensure_alive()`'s `try_wait()` check cannot see that, because it only asks the OS about the process, not the DevTools socket.

When that happens: `restore_default()` returns `Err` (so the caller does see a failure surfaced), **but** `skin-state.json` and `SkinStateTransaction::current()` already say `Default`, while the live Codex window (if the process is still alive) is still rendering the old skin's CSS — because `WebSocketCdpClient::clear_css` never reached the point of clearing the stylesheet, and `CdpSession::clear_css` only sets `self.last_css = None` *after* a successful `client.clear_css` call (`session.rs:236`), so a subsequent `reinject_after_navigation` will faithfully re-push the "cleared" skin's CSS again on the next navigation. `skin_cmds.rs:207-256`'s `list_skins` — which is what the Appearance tab actually renders — reads `txn.current()` directly and would report "Default" applied, contradicting what is genuinely on screen.

I wrote a standalone reproduction (`FakeApplier` whose `clear()` fails once, matching the exact shape of the existing `FakeApplier`s in `step8_3_apply.rs`/`step8_3_divergence.rs`) and confirmed: `restore_default()` returns `Err`, yet `txn.current() == SkinState::Default` while the fake's `live_css()` is still `Some(".x{color:#fff}")` — the exact divergence `step8_3_divergence.rs`'s own `assert_consistent` helper exists to catch, just never invoked against a failing `clear()`. I ran this against the crate, watched it pass (i.e., the bug reproduces), then deleted the scratch test file; `git status --short` on the crate is clean.

I checked whether this is simply untested-but-fine: it is not. Every `FakeApplier`/`FakeCdpClient` in `step8_3_apply.rs`, `step8_3_divergence.rs`, and `step8_2_session.rs` has a `clear()` that unconditionally succeeds (`step8_3_apply.rs:68-71`, `step8_3_divergence.rs:48-51`, `step8_2_session.rs:144-149`) — only `apply()` has a `fail_next_apply` knob anywhere in this crate's test suite. The failure half of `SkinApplier::clear` — the half `restore_default` exists to handle — has never once been exercised by any of the 143 tests. This is precisely the shape the brief asked me to look for: a stop condition ("the default appearance is always restorable") whose enforcement mechanism has an entire branch nobody has ever run a test through, and once run, disagrees with the module's own documented invariant.

Practical severity: this is not "the user can never get back to default" — a second call to `restore_default()` after the transport recovers will succeed and correct the live session (idempotent). The actual harm is a **false-positive recorded state**: the app tells the user (via `list_skins`/the Appearance UI) that Codex is back to its own default appearance while it may still be rendering the old skin, with no visible indication that anything is wrong beyond a possibly-missed error toast. A user who trusts the "Default — applied" badge has no way to know the CDP session silently failed to clear.

### 4. The skin session and the real "Launch Codex" flow are disjoint — no code path connects them

`apps/chimera-desktop/src/features/home/index.tsx:51-61` is the app's actual entry point for running Codex: `handleLaunch` calls `invoke("launch_codex")`, which is `commands::launch_codex` (`commands.rs:308-313`) → `chimera_runtime::process::launch_managed_codex` (`crates/chimera-runtime/src/process.rs:46-84`). That function spawns the managed exe directly — no `--remote-debugging-port`, no `--user-data-dir` override (Codex's own default profile), detached, and the `Child` handle is dropped immediately after reading the pid (`process.rs:80-83`: no session object retains it).

`skin_cmds::SkinRuntime::ensure` (`skin_cmds.rs:69-100`) is the *only* other place this codebase spawns Codex, and it is a second, fully independent path: `CodexLauncher::new(exe, profile)` where `profile = state.paths.data_root.join("skin-profile")` (`skin_cmds.rs:86`), launched with `--remote-debugging-port=<random>` and `--user-data-dir=<skin-profile>` (`cdp_transport.rs:131-140`). This is deliberate isolation — the doc comment at `cdp_transport.rs:135-138` explains it exists specifically so "enabling remote debugging never touches the user's real Codex profile" — but the consequence is that these are two different Codex windows, backed by two different profile directories, with no code anywhere that unifies them. I grepped both `commands.rs` and `state.rs` for any cross-reference (`SkinRuntime`, `is_alive`, `codex_running`) between the two and found none; `AppState.skins` (`state.rs:101`) and the runtime layout the Home button drives are two independent fields with no shared lock, no shared handle, and no code that routes a skin-tab action through the process the user already launched, or vice versa.

The practical consequence: a user who launches Codex from Home, then goes to Appearance and clicks Apply, gets a **second, separate Codex process** — running against an isolated `skin-profile` user-data-dir — which is the one that actually receives the CSS. The window they opened from Home is never touched. Nothing in the UI discloses this; the Appearance page's own "Safety" panel (`appearance/index.tsx:34-40`) tells the user CDP is loopback-only and JS is not allowed — true statements — but says nothing about which Codex window, if either, is actually being shown the skin.

I could not fully verify the end-to-end consequence (whether Codex/Electron enforces its own single-instance lock in a way that would make the second launch a no-op or forward to the first instance instead of opening a genuinely separate window) — that requires a real Codex build, which I do not have on this machine. Either outcome is a problem for the feature: if Codex allows two independent instances, the user now has two live processes and the themed one is not the one they use; if Codex's single-instance lock is keyed independently of `--user-data-dir` and forwards to the existing (Home-launched) window, then the CDP-managed process that the skin engine believes it launched may exit immediately after handing off, and `CdpSession::start`'s target discovery would be racing a process that is already gone — a scenario none of `step8_2_session.rs`'s tests can catch since they all use a fake `BrowserProcess`/`BrowserLauncher` that never model this.

### 5. Step 8.4's fingerprint fuse and kill switch: implemented, tested, and entirely unwired

`crates/chimera-theme/src/fingerprint.rs` is well-built in isolation: `SkinFuse` has no field, method, or variant that names a process or Codex's lifecycle (verified by inspection and by the crate's own `a_tripped_fuse_disables_only_the_skin_never_codex_launch` test), `ExpectedFingerprint::matches` fails closed on any disagreement, and `KillSwitchTrustAnchor::verify` leaves the fuse untouched on any signature/key failure (also tested). This architecturally satisfies "a fingerprint mismatch never prevents stock Codex from launching," and I have no reason to doubt it — the type simply cannot reach a process handle.

But `grep -rn "SkinFuse|compute_fingerprint|ExpectedFingerprint|KillSwitch" --include=*.rs --include=*.ts --include=*.tsx .` outside `crates/chimera-theme/src/fingerprint.rs` and its own test files returns nothing in the shipped app. `skin_cmds.rs`'s `try_skin`, `apply_skin`, and `restore_default_skin` never call `compute_fingerprint`, never construct an `ExpectedFingerprint`, and never consult a `SkinFuse` before pushing CSS live. Today, in this build, a skin will be applied to whatever Codex build is running with no compatibility check performed at all — the entire mechanism exists only as a library API exercised by its own 27-test suite, never by a real `try_skin`/`apply_skin` call.

I read the TODO's own scoping for this (`T48`'s note: "只产出候选测试证据，由 Step 4.5 的镜像 gate 生成并发布 stable capability manifest" — Step 8.4 produces candidate evidence only; the real signed manifest is Task 4.5's job) and take this as intentional under-construction rather than a defect against the stop condition itself — the stop condition is "mismatch never blocks Codex," which holds trivially precisely because the check never runs. I flag it because the inverse claim in the Spec ("选择器或运行时指纹不匹配时，自动停用皮肤并恢复默认" — a mismatch automatically disables the skin) is not true of the shipped code today, and a reviewer skimming the 27 green tests in `step8_4_fingerprint.rs` could reasonably, and wrongly, conclude the protection is live.

## Stop conditions — individually

| Stop condition | Verdict | Basis |
|---|---|---|
| No official Codex file is ever modified | **PASS** | `apply.rs`'s API has no parameter, field, or path that can name the Codex install (verified by reading every function signature); `SKINS_DIR`/`SKIN_STATE_DIR`/`skin-profile` are all joined onto `state.paths.data_root`, never `state.paths.codex_home` or the runtime root (`skin_cmds.rs:29-31,86,92-94`; `state.rs:16-21,60-63`). `step8_3_apply.rs`'s snapshot test is tautological (the API never receives the path, so "never touching it" is guaranteed by the type signature, not by extra logic) but does not contradict the claim. |
| No arbitrary JavaScript, no remote resource | **PASS** | Verified the CSS-escape fix by deletion (above). `package.rs` bans `.js`/executables/anything off its extension allowlist, verifies magic bytes so a renamed payload doesn't ride through, and scans SVGs for script vectors — all independently tested (30+18+5 tests across `step8_1_import.rs`/`step8_1_css_allowlist.rs`/`step8_1_css_escape.rs`). `cdp_transport.rs`/`session.rs` use only `CSS.createStyleSheet`/`CSS.setStyleSheetText`; `step8_2_no_javascript.rs` source-scans for `Runtime.evaluate` and friends and proves its own matcher works. Confirmed the legacy `codex-plus-core` crate (which *does* use `Runtime.evaluate` extensively, v1.x code) is not a dependency of `chimera-desktop` at all (`apps/chimera-desktop/src-tauri/Cargo.toml`) — the safe path is the only one reachable from the shipped app. |
| CDP binds only a random loopback port | **PASS**, with a caveat | `OsPortAllocator`'s tests (`step8_2_session.rs:175-221`) are the one place this crate legitimately opens real sockets rather than fakes, and they prove non-fixed allocation, immediate release, and no collision across concurrent allocations. `target_socket_url` (`cdp_transport.rs:81-83`) is built from the port Chimera itself chose, never from the browser's self-reported `webSocketDebuggerUrl` — closing the obvious spoof vector. **Caveat:** I cannot verify, without a real Codex/Chromium build, that the child process actually honors `--remote-debugging-address=127.0.0.1` rather than binding wider — the module's own docs concede this ("not something a unit test can honestly assert"), and I have no such build on this machine. |
| A fingerprint mismatch never prevents stock Codex from launching | **PASS** (vacuously, see finding 5) | `SkinFuse` cannot reach a process by construction, and is untested-in-anger only because it is never called at all in the live path yet. |
| The default appearance is always restorable | **PASS WITH A GAP** — see finding 3 | `restore_default` is always callable and idempotent, and correctly handles "no session ever opened" (`skin_cmds.rs:307-310`) and "restore right after a failed apply" (tested, `step8_3_apply.rs:166-195`). The gap is specifically the divergence between recorded and live state when the live `clear()` call itself fails — restoring is always *retriable*, but the app can misreport success in the interim. |

## What I could not verify

- **A real Codex build.** Everything about whether the launched child actually binds loopback-only, whether it honors `--user-data-dir` the way Chromium/Electron conventionally does, and whether it enforces (or doesn't) a single-instance lock independent of profile directory, requires a real Codex executable. This directly bears on finding 4's practical severity — I can prove the two launch paths are architecturally disjoint from the Chimera side; I cannot prove what actually happens on a real machine when both fire.
- **A real Chromium DevTools session under load**, to directly reproduce (rather than reason from the code and a fake) the "CDP call times out while the process is alive" precondition behind finding 3. The reasoning is sound from the `CALL_TIMEOUT`/`ensure_alive` code, but I did not have a live Codex instance to actually stall a `CSS.setStyleSheetText` call against.
- **The real-Codex pixel gate (Step 8.5)**, which the TODO already marks outside what's complete for Task 8 and outside this audit's stated Step range (8.1–8.4) regardless.

## Housekeeping

- `apps/chimera-desktop/src-tauri/src/commands.rs:338-340` still carries the header comment `// chimera-theme is a stub until Task 8. These commands return the honest default-only state...` with no code following it — stale from before `skin_cmds.rs` became the real implementation. Harmless (nothing is defined there to conflict with `skin_cmds.rs`'s registrations in `lib.rs:78-83`), but exactly the kind of doc/reality mismatch the brief warned about; worth deleting so a future edit doesn't duplicate a command here.
- `apps/chimera-desktop/src/features/appearance/index.tsx`'s `handleApply`/`handleTry`/`handleRestore` (`index.tsx:102-135`) update local `skins` state optimistically on success and do nothing on failure — they never call `reload()` in the `catch` branch. After the finding-3 scenario (restore reports an error but the backend has already committed to `Default`), the frontend's skin list can keep showing whichever skin was "applied" before the failed restore, compounding the confusion rather than refetching the truth. Minor, UI-only, but the same "trust a stale local copy after an error" pattern as the backend-level finding.
- I noticed a separate, concurrent process editing this same shared repository during this audit (temporary edits/untracked files under `crates/chimera-migration/tests/` and `docs/superpowers/audits/task-8-v2-aggregate-a.md` appeared and disappeared while I was working, consistent with another auditor session running in parallel). I did not touch, read for content-borrowing, or revert anything not created by me. `git status --short` at the end of this session shows no diff in any file I edited (`crates/chimera-theme/src/css_allowlist.rs`, `crates/chimera-theme/src/apply.rs`) and no leftover scratch file of mine.

## Confirmation

```
git status --short
```
shows no changes in `crates/chimera-theme/` attributable to this audit; my two temporary edits (the CSS-escape guard, the `apply_and_commit` phase order) were each reverted immediately after producing the failing test run, and the scratch reproduction test for finding 3 was deleted after use.
