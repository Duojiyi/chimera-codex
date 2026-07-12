# Task 14 Aggregate Audit B - T33

> Status: **PASS AFTER REMEDIATION**
> Date: 2026-07-12
> Independence: final-diff audit; did not read or reference aggregate audit A
> Scope: visual originality, self-contained SVG, PNG/ICO structure and hashes, source-to-export traceability, platform references, small-size legibility, and CI gate bypasses

## Decision

PASS. The current artwork is visually distinct, legible and correctly referenced, and the checked-in PNG/ICO files are valid. The three gate defects found by this audit were reproduced with negative fixtures and closed: SVG uses a strict safe vocabulary, derived assets are regenerated from the master by the package-lock-pinned Tauri CLI, and every ICO PNG frame is decoded and dimension-checked.

## Remediation Re-review B

- Re-review used the final diff and did not read or reference audit A.
- The original script/style/event/CSS URL probe, plus processing-instruction, external-namespace, `xlink:href`, `file:` URL, case-varied event and external DTD probes, are all rejected.
- The original four-frame signature-only ICO probe now returns four findings: every eight-byte pseudo-PNG fails real decoding.
- `npm ci` precedes both icon self-test and real icon verification in PR and Release gates. Release Windows/macOS builds and publish all depend on the successful gate job.
- Local `@tauri-apps/cli` resolves to package-lock entry `2.11.2` with registry integrity metadata. Its fresh ICO and 1024 PNG exports match the distributed hashes exactly.

## Findings

### B1 - CLOSED: SVG self-containment gate allowed active and external input

- `Get-SvgFindings` now uses `XmlReaderSettings` with DTD processing prohibited, a null resolver, entity/document limits, and rejects processing instructions.
- Elements are restricted to `svg/g/rect/path/title/desc` in the SVG namespace; attributes use a narrow allowlist. Foreign namespaces, namespaced attributes, event attributes, style/href and active/URI values fail.
- `-SelfTest` covers DTD/entity, script, style/import/url, event and data-image cases. Independent probes also rejected processing instructions, external namespace declarations, `xlink:href`, file URLs and case-varied events.
- PR and Release workflows run the self-test before the real gate.

### B2 - CLOSED: distributed PNG/ICO assets were not machine-bound to `brand/icon/logo.svg`

- The real gate requires `apps/codex-plus-manager/node_modules/.bin/tauri.cmd`; absence fails with an explicit instruction to run `npm ci`.
- It regenerates ICO and 1024 PNG from the current `brand/icon/logo.svg` into `target/verify-brand-icons`, then compares their SHA-256 values with the distributed primary assets. Byte-identical sibling checks bind the remaining four copies transitively.
- `package-lock.json` fixes CLI version `2.11.2`, resolved URL and integrity; both CI gate jobs run `npm ci` before verification.
- Independent hash comparison confirmed generated ICO `74723AA...1715` and PNG `F881F5...53E3` exactly match the release assets. Source-only drift or unrelated replacement can no longer pass both comparisons.

### B3 - CLOSED: ICO validation checked labels without decoding frame payloads

- `Get-IcoFindings` validates directory count, offset/length ranges and requires the locked exporter's PNG payload format.
- Each payload is copied into a bounded stream and decoded with `System.Drawing.Image::FromStream(..., validateImageData=true)`; decoded dimensions must match its directory entry.
- Self-test rejects zero-payload and signature-only truncated PNG fixtures. Re-running the original four-frame signature-only probe produced four decode findings instead of zero.
- The real ICO still independently decodes all 16/24/32/48/64/256 frames with matching dimensions.

## Verified Behavior

- Visual review using the `logo-designer` small-size criteria: the dark rounded-square C+ monogram does not resemble OpenAI/ChatGPT's interlocking knot or the prior Codex++ icon. It has a strong independent silhouette and no implied official affiliation.
- The 1024 PNG has transparent outer corners and good light/dark contrast. At 32px the C and full green plus remain distinct. At 16px the plus simplifies to a 2x2 green center accent, but the C silhouette remains readable; this tradeoff is accurately disclosed in `PROVENANCE.md`.
- `brand/icon/logo.svg` itself is clean: `viewBox="0 0 512 512"`, no fixed dimensions, external references, fonts, images, gradients, masks or filters.
- Three concepts and three refinements are retained, and provenance records the brief, tool/model, input boundary, selection, date and AGPL-3.0-only release declaration.
- PNG copies are byte-identical with SHA-256 `F881F5F1FE76449D7C11ACF61581DFA39DB0A0DD1940129ADF3860A9DBBE53E3`; ICO copies are byte-identical with SHA-256 `74723AA52B081D84970E3FE5358CA4F1B58F4C3BA1F94AB930BD54E96EC71715`; master SVG hash matches the evidence ledger.
- Tauri window/tray, launcher PE resource, NSIS installer/uninstaller, runtime Windows/macOS installers, README/docs, built-in assets and macOS `.icns` generation reference the new asset locations. PR and Release workflows invoke C11.

## Commands

```text
pwsh -NoProfile -File scripts/verify-brand-icons.ps1
PASS (positive fixture only)

pwsh -NoProfile -File scripts/generate-branding.ps1 -Check
PASS

cargo test -p codex-plus-core --test installers --locked pr_and_release_workflows_verify_original_brand_icons -- --exact
PASS - 1/1

cargo test -p codex-plus-core --test branding --locked
PASS - 3/3

cargo test -p codex-plus-manager --test windows_subsystem --locked icon
PASS - 2/2

pwsh -NoProfile -File scripts/verify-no-upstream-ads.ps1
PASS

git diff --check -- <Task 14 files>
PASS - no whitespace errors; only existing line-ending conversion warnings

SVG selector mutation probe
PASS - script/style/on*/style/href/url/DTD/processing-instruction/external namespace fixtures rejected

Malformed ICO directory probe
PASS - zero-payload, out-of-range and signature-only PNG fixtures rejected

pwsh -NoProfile -File scripts/verify-brand-icons.ps1 -SelfTest
PASS

Locked Tauri re-export
PASS - regenerated ICO/1024 PNG hashes equal distributed hashes

Independent real ICO decode
PASS - 16/24/32/48/64/256 entries are bounded and decode to matching dimensions
```

## Residual Risk

- Windows executable resource compilation and installed shortcut appearance remain Task 15/16 runner and real-install checks.
- macOS `.icns`, Finder and Dock rendering cannot be executed on this Windows host and remain required x64/arm64 release smoke.
- Visual originality is necessarily a human judgment; the present manual comparison passes, but hashes alone must never be treated as proof against third-party pixel reuse.

## Gate

**PASS.** B1-B3 are closed with negative fixtures and focused regression evidence. Task 14 may pass its aggregate gate if the other independent audit also passes.
