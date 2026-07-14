# Upstream v1.2.36 Resolution Audit B

Date: 2026-07-14
Perspective: complete diff, boundaries, and regression surface
Result: PASS

## Findings

The independent diff review found no remaining unmerged index entries or conflict markers. Chimera relay latency, application types/calls, branding, customer routing, updater policy, and promotion tombstones remain intact. Sponsor/community assets and GitHub customer CTA additions did not enter the candidate tree.

Upstream portable companion arguments, macOS bundle-id launch with safe fallback, repeated manager activation, Windows window scoring, V2 pet settings/assets/CDP gating, and launcher watchdog cleanup are internally consistent with their tests. The translocation fixtures use exact Chimera bundle names and reject legacy names.

README behavior now matches the NSIS contract: the only desktop shortcut targets the manager, the silent launcher is not a second desktop target, and both Start Menu entries remain available. Version values are aligned at `1.2.36-chimera.1` with macOS build `7`; the line-ending-only `src-tauri/Cargo.toml` working-tree entry is excluded from the index.

No open diff or regression finding remains. Cloud required checks and Task 16 real-platform acceptance are the declared residual gates.
