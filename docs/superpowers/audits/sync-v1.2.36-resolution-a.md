# Upstream v1.2.36 Resolution Audit A

Date: 2026-07-14
Perspective: requirements, tests, and observable customer behavior
Result: PASS

## Findings

The first review found one blocking documentation mismatch: both customer READMEs said the only Windows desktop shortcut directly launched Codex, while the installer contract targets the manager. This was closed with a Red/Green customer README contract and corrected Chinese and English copy.

The final independent review confirmed that Windows has one desktop `Chimera++` entry opening the manager, Codex launches from the manager, and macOS still documents two apps with `Chimera++.app` as the daily launch app. The new assertions lock both language variants without changing the macOS behavior.

No open requirement or customer-behavior finding remains. Cross-platform compilation/packaging and Task 16 real installation acceptance remain explicitly outside this local audit.
