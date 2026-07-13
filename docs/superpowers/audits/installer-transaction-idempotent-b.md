# Installer transaction idempotency - Independent audit B

Date: 2026-07-13

Independence: reviewed only the diff, NSIS control flow, boundary conditions,
version/lockfile scope, allowlist precision, and regression surface without
consulting audit A.

- The empty-string exit precedes the legacy comparison and index increment.
- Legacy whole-key deletion retains its fatal error branch and success jump.
- Best-effort `/ifempty` cleanup is used only for URL protocol keys and clears
  its error flag; other deletion classes remain fail-closed.
- `Cargo.lock` changes only four workspace versions.
- The eight moved allowlist entries advance by exactly one line and all entries
  still match exact paths, patterns, line numbers, and source text.
- The no-diff `src-tauri/Cargo.toml` status is excluded from the staging scope.

PASS. No local-scope blocker remains; GitHub Actions and real-machine testing
are still required.
