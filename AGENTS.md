# AGENTS.md

## 1. Project Overview

Chimera++ 2.0 is a fully independent product. It is no longer a fork or derivative of CodexPlusPlus and does not track any CodexPlusPlus upstream.

Chimera++ v2 provides three core capabilities for Codex desktop users:

- Vendor switcher: lets users point Codex at any compatible API provider without patching the binary.
- Runtime manager: manages provider credentials, model selections, and connection lifecycle.
- Optional skin enhancer: ships opt-in UI theme overlays that do not modify Codex application files.

The 1.x line remains available on the `1.x-maintenance` branch for security fixes only. All new development targets the `v2` branch.

---

## 2. Repository Structure

```
chimera-refs/codex-app-mirror/
├── apps/
│   └── chimera-desktop/        # Electron shell and renderer (TypeScript + React)
├── crates/
│   ├── chimera-domain/         # Core domain types and business rules (no I/O)
│   ├── chimera-platform/       # OS-level integration (file paths, process launch)
│   ├── chimera-provider/       # Provider adapter implementations
│   ├── chimera-runtime/        # Credential lifecycle and active-session management
│   ├── chimera-theme/          # Skin/theme asset pipeline and injection engine
│   └── chimera-migration/      # Schema and config migration between versions
├── services/
│   └── mirror-contract/        # Shared interface contracts between desktop and Rust backend
└── scripts/
    └── verify-v2.mjs           # Primary verification entry point (see V1-V15 below)
```

Each crate owns its own `tests/` directory. Integration tests live under `tests/` at the workspace root.

---

## 3. Key Code Locations

These locations will be populated as Tasks 1-9 land. Do not guess paths; update this table when a task is merged.

| Area | Path | Task |
|---|---|---|
| Provider adapter trait | TBD | Task 2 |
| Credential store | TBD | Task 3 |
| Runtime session loop | TBD | Task 4 |
| Config operation lock | TBD | Task 5 |
| Theme injection entry | TBD | Task 6 |
| Migration runner | TBD | Task 7 |
| Mirror contract types | TBD | Task 8 |
| Desktop IPC bridge | TBD | Task 9 |

Until a path is confirmed by a merged PR, leave the cell as TBD. Never speculate.

---

## 4. Security Rules

These rules are non-negotiable. Violations block merge.

**No bulk delete without a named guard.**
Any code path that removes more than one record or file at a time must be gated behind an explicit operation lock acquired before the write begins. Batch deletes without a lock are rejected in review.

**No secrets in code, logs, or fixtures.**
API keys, tokens, passwords, and provider credentials must never appear in source files, test fixtures, log output, or commit history. Use environment variables or the credential store. If a secret is accidentally committed, treat it as compromised immediately.

**No plain-text keys in SQLite fields.**
Credentials stored in the local database must be encrypted at rest. Storing a raw key string in any SQLite column is a blocking defect regardless of whether the column name sounds sensitive.

**Require operation lock before writing config.**
Any write to the active configuration (provider selection, model override, theme state) must acquire the operation lock defined in `chimera-runtime`. Config writes that bypass the lock are rejected.

**No unsafe logging of user input.**
Log statements must not include user-supplied strings verbatim unless they have been explicitly sanitized. PII and credential fragments must be redacted before logging.

**Dependency additions require review.**
New third-party crates or npm packages must be justified in the PR description. Pinned exact versions are required. Open version ranges (`*`, `>=`, `^`) are not permitted.

---

## 5. Command Execution Rules

**Always confirm before running build or test commands in a user session.**
Before executing `cargo build`, `cargo test`, `npm run`, `pnpm`, or any script from `package.json` or `Makefile`, state the command and wait for explicit confirmation unless the user has already approved it in the current turn.

**Do not run scripts that are not listed in this file or in `package.json`/`Cargo.toml`.**
Unknown scripts (especially downloaded or generated ones) must not be executed without user review.

**The verification suite is the canonical check.**
Use `node scripts/verify-v2.mjs` as the primary gate. Do not substitute ad-hoc one-liner checks as a replacement for the full suite.

**No side-effecting commands during read-only investigation.**
When investigating a bug or reading the codebase, do not run commands that modify state (database writes, file creation outside temp directories, network calls to live providers).

---

## 6. Coding Standards

### Rust crates

- Follow a strict three-layer architecture in each crate: **domain**, **service**, **adapter**.
- The domain layer (`chimera-domain`) must have zero I/O dependencies. No `tokio`, no `reqwest`, no `std::fs` in domain types.
- Service layers coordinate domain logic with adapters but do not call external services directly.
- Adapter layers own all I/O: file system, HTTP, SQLite, IPC.
- Avoid `unwrap()` and `expect()` in non-test code. Use `?` propagation and typed errors.
- All public items in a crate must have doc comments in English or Chinese. Both are acceptable; mixing within a single item is not.
- Comments explaining intent or non-obvious decisions may be written in Chinese.

### TypeScript / React (apps/chimera-desktop)

- Use a **feature-based** directory layout. Each feature owns its components, hooks, and local state.
- Page components must not read files or call IPC directly. They receive data through hooks or context.
- All IPC calls go through the bridge defined in `services/mirror-contract`. No direct `ipcRenderer.invoke` calls scattered through components.
- Prefer `unknown` over `any`. Uses of `any` require a comment explaining why it cannot be avoided.
- No barrel `index.ts` files that re-export everything; they hide import paths and slow TypeScript.

---

## 7. TDD and Dual-Blind Audit Rules

### Red-Green-Refactor

Every feature change follows the strict cycle:

1. **Red**: write a failing test that describes the expected behavior. Commit the failing test alone.
2. **Green**: write the minimum code to make the test pass. No gold-plating.
3. **Refactor**: clean up without changing behavior. All tests must remain green after refactor.

Skipping the Red step (writing code first, then writing a test that passes immediately) is not acceptable. If a reviewer cannot identify the commit that contained the failing test, the PR is returned.

### Dual-Blind Audit

Each Step (as defined in the project plan) requires two independent audit passes before its checkbox can be marked complete:

- **Audit A**: the author reviews their own diff against the acceptance criteria and signs off.
- **Audit B**: a second reviewer (human or designated agent) reviews the same diff independently, without seeing Audit A's notes until their own review is written.

Both audits must be recorded in the PR description or a linked review comment. A Step with only one audit pass is not done.

---

## 8. v2 Branch Rules

**`v2` is the long-term development branch.**
All feature work, refactors, and new tasks target `v2`. Direct commits to `main` are not permitted except for release tagging.

**No auto-sync to CodexPlusPlus upstream.**
Chimera++ v2 is an independent product. There is no upstream remote pointing to CodexPlusPlus. Do not add one. Do not cherry-pick from CodexPlusPlus without explicit written approval from the project lead, and only after a security review of the incoming changes.

**`1.x-maintenance` is for security fixes only.**
The `1.x-maintenance` branch receives backported security patches only. No new features, no refactors, no dependency upgrades unless they are directly required by a security fix.

**Never push Chimera customizations to upstream.**
Chimera-specific logic, configuration formats, credential handling, and theme APIs must not be contributed back to Codex or any other upstream project. If a general improvement is identified that has no Chimera-specific content, it may be contributed separately after review, but this requires explicit approval.

**Branch naming for tasks.**
Feature branches follow the pattern `v2/task-N-short-description`. Hotfix branches on `1.x-maintenance` follow `1x/fix-short-description`.

---

## 9. Verification Commands

The primary verification entry point is:

```
node scripts/verify-v2.mjs
```

Individual checks (V1-V15) can be run by passing the check ID as an argument:

```
node scripts/verify-v2.mjs --check V3
```

| ID | What it checks |
|---|---|
| V1 | Workspace compiles without errors (`cargo check --workspace`) |
| V2 | All Rust unit tests pass (`cargo test --workspace`) |
| V3 | No `unwrap()` or `expect()` outside `#[cfg(test)]` blocks |
| V4 | No plain-text credential patterns in source or fixtures |
| V5 | SQLite schema has no unencrypted key columns (static analysis) |
| V6 | Operation lock is acquired before every config write path |
| V7 | TypeScript compiles without errors (`tsc --noEmit`) |
| V8 | Frontend unit tests pass |
| V9 | No direct `ipcRenderer.invoke` calls outside the bridge module |
| V10 | No open dependency version ranges in `Cargo.toml` or `package.json` |
| V11 | All public Rust items have doc comments |
| V12 | Migration runner can apply and roll back every known migration |
| V13 | No CodexPlusPlus remote configured in git remotes |
| V14 | Dual-blind audit records present for all completed Steps |
| V15 | Full integration smoke test (desktop launches, provider round-trip, theme toggle) |

Run the full suite before opening any PR against `v2` or `1.x-maintenance`. A PR with a failing verification check is not reviewed until the check passes.
