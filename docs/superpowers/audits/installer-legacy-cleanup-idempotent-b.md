# Windows legacy cleanup idempotency - Independent audit B

Date: 2026-07-13

Independence: reviewed only the diff, NSIS control flow, boundary conditions,
version/lockfile scope, and regression surface without consulting audit A.

## Findings

- The NSIS index starts at zero, increments after a miss, and loops correctly.
- End-of-enumeration is the absent-key success path; only an exact `Codex++`
  subkey match reaches `DeleteRegKey`.
- A real deletion error remains fatal; successful deletion cannot fall through
  to a failure label.
- The Rust contract locks initialization, scan, absent-key completion, match,
  increment, loop, delete failure, and delete success ordering.
- `Cargo.lock` changes only four workspace package versions; no third-party
  dependency graph drift remains.
- All release version surfaces agree on `1.2.35-chimera.2`, and the macOS build
  number advances from `3` to `4`.
- `apps/codex-plus-manager/src-tauri/Cargo.toml` is a no-diff phantom status and
  must remain outside the explicit staging allowlist.

## Conclusion

PASS. No unrelated semantic change or local regression blocker remains. Cloud
builds and Task 16 real-machine installation remain required.
